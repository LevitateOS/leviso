//! Complete system builder for squashfs.
//!
//! Builds the complete system by merging:
//! - Binaries, PAM, systemd, sudo, recipe (complete user-facing tools)
//! - Networking (NetworkManager, wpa_supplicant, WiFi firmware)
//! - D-Bus (required for systemctl, timedatectl, etc.)
//! - Chrony (NTP time synchronization)
//! - Kernel modules (for hardware support)
//!
//! The result is a single image that serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! DESIGN: Live = Installed (same content, zero duplication)

use anyhow::{bail, Result};
use std::fs;

use crate::build::{self, BuildContext};
use crate::common::binary::copy_dir_recursive;

/// Build the complete system into the staging directory.
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("Building complete system for squashfs...");

    build_filesystem_and_binaries(ctx)?;
    build_systemd_and_services(ctx)?;
    build_network_and_auth(ctx)?;
    build_packages_and_firmware(ctx)?;
    build_final_setup(ctx)?;

    println!("System build complete.");
    Ok(())
}

/// Phase 1-2: Create filesystem structure and copy all binaries.
fn build_filesystem_and_binaries(ctx: &BuildContext) -> Result<()> {
    // Filesystem structure
    build::filesystem::create_fhs_structure(&ctx.staging)?;
    build::filesystem::create_symlinks(&ctx.staging)?;

    // Binaries (87+ BIN + 38+ SBIN + systemd + sudo + login)
    build::binary_lists::copy_shell(ctx)?;
    build::binary_lists::copy_coreutils(ctx)?;
    build::binary_lists::copy_sbin_utils(ctx)?;
    build::binary_lists::copy_systemd_binaries(ctx)?;
    build::binary_lists::copy_login_binaries(ctx)?;
    build::binary_lists::copy_sudo_libs(ctx)?;

    Ok(())
}

/// Phase 3: Set up systemd units, getty, and udev.
fn build_systemd_and_services(ctx: &BuildContext) -> Result<()> {
    build::systemd::copy_systemd_units(ctx)?;
    build::systemd::copy_dbus_symlinks(ctx)?;
    build::systemd::setup_getty(ctx)?;
    // NOTE: Autologin and serial-console are NOT in squashfs
    // They're in the live overlay (/live/overlay on ISO) and applied only during live boot
    // This ensures installed systems have proper security (login required)
    build::systemd::set_default_target(ctx)?; // multi-user.target
    build::systemd::copy_dbus_configs(ctx)?;
    build::systemd::copy_udev_rules(ctx)?;
    build::systemd::copy_tmpfiles(ctx)?;
    build::systemd::copy_sysctl(ctx)?;
    build::systemd::setup_live_systemd_configs(ctx)?;

    Ok(())
}

/// Phase 4-8: Set up networking, D-Bus, chrony, kernel modules, and PAM.
fn build_network_and_auth(ctx: &BuildContext) -> Result<()> {
    // Networking (NetworkManager + wpa_supplicant + helpers + WiFi firmware)
    build::network::setup_network(ctx)?;

    // D-Bus
    build::dbus::setup_dbus(ctx)?;

    // Chrony NTP
    build::users::ensure_chrony_user(ctx)?;
    build::chrony::setup_chrony(ctx)?;

    // OpenSSH server (remote installation/rescue)
    build::openssh::setup_openssh(ctx)?;

    // Kernel modules (daily driver needs hardware support)
    let config = crate::config::Config::load(&ctx.base_dir);
    let module_list = config.all_modules();
    build::modules::setup_modules(ctx, &module_list)?;

    // PAM (full authentication for installed system)
    // Note: Live ISO uses autologin (like archiso), installed system uses PAM
    build::pam::setup_pam(ctx)?;
    build::pam::copy_pam_modules(ctx)?;
    build::pam::create_security_config(ctx)?;

    Ok(())
}

/// Phase 9-12: Set up /etc, recipe, dracut, firmware, keymaps, and kernel.
fn build_packages_and_firmware(ctx: &BuildContext) -> Result<()> {
    // /etc configuration
    build::etc::create_etc_files(ctx)?;
    build::etc::copy_timezone_data(ctx)?;
    build::etc::copy_locales(ctx)?;

    // Empty machine-id - systemd generates on first boot
    let machine_id = ctx.staging.join("etc/machine-id");
    fs::write(&machine_id, "")?;

    // Disable SELinux - we don't ship policies
    disable_selinux(ctx)?;

    // Recipe package manager
    build::recipe::copy_recipe(ctx)?;
    build::recipe::setup_recipe_config(ctx)?;

    // Dracut (initramfs generator for installation)
    copy_dracut_modules(ctx)?;
    build::etc::create_dracut_config(ctx)?;

    // systemd-boot EFI files (bootloader for installation)
    copy_systemd_boot_efi(ctx)?;

    // ALL firmware (daily driver needs everything)
    copy_all_firmware(ctx)?;

    // Keymaps (for loadkeys command)
    copy_keymaps(ctx)?;

    // NOTE: Kernel is NOT included in squashfs
    // - Live boot: kernel is on ISO at /boot/vmlinuz, loaded by GRUB
    // - Installed system: kernel is copied from ISO to ESP during installation
    // This avoids duplication and FAT32 ownership errors when extracting squashfs

    Ok(())
}

/// Phase 13-17: Final setup (init symlink, welcome, recstrap).
fn build_final_setup(ctx: &BuildContext) -> Result<()> {
    // Create /sbin/init symlink
    let init_link = ctx.staging.join("usr/sbin/init");
    if !init_link.exists() && !init_link.is_symlink() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }
    // Note: /sbin/init exists via merged /usr (/sbin -> /usr/sbin)
    // The symlink chain: /sbin/init -> /usr/sbin/init -> /usr/lib/systemd/systemd

    // NOTE: Root password is NOT set to empty here
    // - Base system (squashfs): root:! (locked, no login possible)
    // - Live overlay: root:: (empty password, autologin works)
    // - Installed system: root password set by user during installation (chpasswd)
    // The live overlay is applied by init_tiny, NOT baked into squashfs

    // Note: For squashfs, we use switch_root instead of rdinit
    // The tiny initramfs handles mounting squashfs, then switch_root to /sbin/init

    // Create welcome message
    create_welcome_message(ctx)?;

    // Copy recstrap installer
    copy_recstrap(ctx)?;

    Ok(())
}

/// Copy keymaps for loadkeys command.
fn copy_keymaps(ctx: &BuildContext) -> Result<()> {
    println!("Copying keymaps...");

    let keymaps_src = ctx.source.join("usr/lib/kbd/keymaps");
    let keymaps_dst = ctx.staging.join("usr/lib/kbd/keymaps");

    if keymaps_src.exists() {
        fs::create_dir_all(keymaps_dst.parent().unwrap())?;
        copy_dir_recursive(&keymaps_src, &keymaps_dst)?;
        println!("  Copied keymaps for keyboard layout support");
    } else {
        // FAIL FAST - keymaps are REQUIRED for loadkeys (keyboard layout)
        // Without keymaps, users cannot change keyboard layout.
        // DO NOT change this to a warning.
        bail!(
            "Keymaps not found at {}.\n\
             \n\
             Keymaps are REQUIRED for keyboard layout support (loadkeys).\n\
             Users cannot change keyboard layout without them.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            keymaps_src.display()
        );
    }

    Ok(())
}

/// Copy dracut modules (required for initramfs generation during installation).
fn copy_dracut_modules(ctx: &BuildContext) -> Result<()> {
    println!("Copying dracut modules...");

    let dracut_src = ctx.source.join("usr/lib/dracut");
    let dracut_dst = ctx.staging.join("usr/lib/dracut");

    if dracut_src.exists() {
        fs::create_dir_all(dracut_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&dracut_src, &dracut_dst)?;
        println!(
            "  Copied dracut modules ({:.1} MB)",
            size as f64 / 1_000_000.0
        );
    } else {
        // FAIL FAST - dracut is REQUIRED for initramfs generation during installation
        // Without dracut, users cannot install LevitateOS to disk.
        // DO NOT change this to a warning.
        bail!(
            "Dracut modules not found at {}.\n\
             \n\
             Dracut is REQUIRED - it generates the initramfs during installation.\n\
             Without it, users cannot install LevitateOS to disk.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            dracut_src.display()
        );
    }

    Ok(())
}

/// Copy systemd-boot EFI files (required for bootloader installation).
///
/// These files are in the systemd-boot-unsigned RPM on the Rocky ISO.
/// bootctl install requires these to create a bootable system.
fn copy_systemd_boot_efi(ctx: &BuildContext) -> Result<()> {
    use std::process::Command;

    println!("Copying systemd-boot EFI files...");

    let efi_dst = ctx.staging.join("usr/lib/systemd/boot/efi");

    // Find the systemd-boot-unsigned RPM
    let rpm_dir = ctx.base_dir.join("downloads/iso-contents/AppStream/Packages/s");
    let rpm_pattern = "systemd-boot-unsigned";

    let rpm_path = std::fs::read_dir(&rpm_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains(rpm_pattern))
                .unwrap_or(false)
        });

    let Some(rpm_path) = rpm_path else {
        // FAIL FAST - systemd-boot EFI files are REQUIRED for installation
        bail!(
            "systemd-boot-unsigned RPM not found in {}.\n\
             \n\
             The EFI files from this package are REQUIRED for bootctl install.\n\
             Without them, users cannot install a bootable system.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            rpm_dir.display()
        );
    };

    // Extract to temp directory
    let temp_dir = ctx.base_dir.join("output/.systemd-boot-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    // Extract using rpm2cpio and cpio
    let rpm2cpio = Command::new("rpm2cpio")
        .arg(&rpm_path)
        .current_dir(&temp_dir)
        .output()?;

    if !rpm2cpio.status.success() {
        bail!(
            "Failed to extract RPM: {}",
            String::from_utf8_lossy(&rpm2cpio.stderr)
        );
    }

    let cpio = Command::new("cpio")
        .args(["-idm"])
        .current_dir(&temp_dir)
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    let mut cpio_stdin = cpio.stdin.unwrap();
    std::io::Write::write_all(&mut cpio_stdin, &rpm2cpio.stdout)?;
    drop(cpio_stdin);

    // Copy EFI files to staging
    let efi_src = temp_dir.join("usr/lib/systemd/boot/efi");
    if efi_src.exists() {
        fs::create_dir_all(efi_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&efi_src, &efi_dst)?;
        println!(
            "  Copied systemd-boot EFI files ({:.1} KB)",
            size as f64 / 1_000.0
        );
    } else {
        bail!(
            "EFI files not found in extracted RPM at {}.\n\
             Expected /usr/lib/systemd/boot/efi directory.",
            temp_dir.display()
        );
    }

    // Cleanup temp directory
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(())
}

/// Copy ALL firmware for daily driver support.
fn copy_all_firmware(ctx: &BuildContext) -> Result<()> {
    println!("Copying ALL firmware...");

    let firmware_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("usr/lib/firmware");

    // Also check /lib/firmware
    let alt_src = ctx.source.join("lib/firmware");
    let actual_src = if firmware_src.exists() {
        &firmware_src
    } else if alt_src.exists() {
        &alt_src
    } else {
        // FAIL FAST - firmware is REQUIRED for hardware support
        // Without firmware, WiFi, Bluetooth, GPU, and other hardware won't work.
        // LevitateOS is a DAILY DRIVER - it MUST support real hardware.
        // DO NOT change this to a warning.
        bail!(
            "No firmware directory found.\n\
             \n\
             Checked:\n\
             - {}\n\
             - {}\n\
             \n\
             Firmware is REQUIRED - LevitateOS is a daily driver for real hardware.\n\
             Without firmware, WiFi, Bluetooth, GPU, and other devices won't work.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            firmware_src.display(),
            alt_src.display()
        );
    };

    fs::create_dir_all(&firmware_dst)?;

    let size = copy_dir_recursive(actual_src, &firmware_dst)?;
    println!(
        "  Copied all firmware ({:.1} MB)",
        size as f64 / 1_000_000.0
    );

    // Rocky/RHEL puts Intel microcode in /usr/share/microcode_ctl/, not /lib/firmware/
    // Copy it to the standard location so early microcode loading works
    let intel_ucode_dst = firmware_dst.join("intel-ucode");
    let microcode_ctl_src = ctx.source.join("usr/share/microcode_ctl/ucode_with_caveats/intel/intel-ucode");
    if microcode_ctl_src.exists() && microcode_ctl_src.is_dir() {
        fs::create_dir_all(&intel_ucode_dst)?;
        let intel_size = copy_dir_recursive(&microcode_ctl_src, &intel_ucode_dst)?;
        println!(
            "  Copied Intel microcode from microcode_ctl ({:.1} KB)",
            intel_size as f64 / 1_000.0
        );
    }

    // Validate microcode directories exist (P0 critical for CPU security)
    // Per CLAUDE.md Rule 7: FAIL FAST - NO WARNINGS FOR REQUIRED COMPONENTS
    // LevitateOS ISO must work on ANY x86-64 hardware - require BOTH
    let amd_ucode = firmware_dst.join("amd-ucode");
    let intel_ucode = firmware_dst.join("intel-ucode");

    // AMD microcode (required)
    if !amd_ucode.exists() {
        bail!(
            "AMD microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.\n\
             AMD microcode comes from linux-firmware package.",
            amd_ucode.display()
        );
    }
    let amd_count = fs::read_dir(&amd_ucode)?.filter(|e| e.is_ok()).count();
    if amd_count == 0 {
        bail!(
            "AMD microcode directory is empty at {}.\n\
             The directory should contain microcode files from linux-firmware.",
            amd_ucode.display()
        );
    }
    println!("  AMD microcode: {} files", amd_count);

    // Intel microcode (required)
    if !intel_ucode.exists() {
        bail!(
            "Intel microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.\n\
             Intel microcode comes from microcode_ctl RPM.",
            intel_ucode.display()
        );
    }
    let intel_count = fs::read_dir(&intel_ucode)?.filter(|e| e.is_ok()).count();
    if intel_count == 0 {
        bail!(
            "Intel microcode directory is empty at {}.\n\
             The directory should contain microcode files from microcode_ctl.",
            intel_ucode.display()
        );
    }
    println!("  Intel microcode: {} files", intel_count);

    Ok(())
}

/// Disable SELinux - we don't ship policies and it causes boot warnings.
fn disable_selinux(ctx: &BuildContext) -> Result<()> {
    let selinux_dir = ctx.staging.join("etc/selinux");
    fs::create_dir_all(&selinux_dir)?;

    fs::write(
        selinux_dir.join("config"),
        "# SELinux disabled - LevitateOS doesn't ship SELinux policies\n\
         SELINUX=disabled\n\
         SELINUXTYPE=targeted\n",
    )?;

    println!("  Disabled SELinux");
    Ok(())
}

/// Create welcome message for live system.
fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    // /etc/motd - shown after login
    let motd = ctx.staging.join("etc/motd");
    fs::write(
        &motd,
        r#"
  _                _ _        _        ___  ____
 | |    _____   __(_) |_ __ _| |_ ___ / _ \/ ___|
 | |   / _ \ \ / /| | __/ _` | __/ _ \ | | \___ \
 | |__|  __/\ V / | | || (_| | ||  __/ |_| |___) |
 |_____\___| \_/  |_|\__\__,_|\__\___|\___/|____/

 Welcome to LevitateOS Live!

 Installation (manual, like Arch):

   # 1. Partition disk
   fdisk /dev/vda                   # Create GPT, EFI + root partitions

   # 2. Format partitions
   mkfs.fat -F32 /dev/vda1          # EFI partition
   mkfs.ext4 /dev/vda2              # Root partition

   # 3. Mount
   mount /dev/vda2 /mnt
   mkdir -p /mnt/boot
   mount /dev/vda1 /mnt/boot

   # 4. Extract system
   recstrap /mnt

   # 5. Generate fstab
   recfstab -U /mnt >> /mnt/etc/fstab

   # 6. Chroot and configure
   recchroot /mnt
   passwd                           # Set root password
   bootctl install                  # Install bootloader
   exit

   # 7. Reboot
   reboot

 For networking:
   nmcli device wifi list           # List WiFi networks
   nmcli device wifi connect SSID password PASSWORD

"#,
    )?;

    // /etc/issue - shown before login prompt
    let issue = ctx.staging.join("etc/issue");
    fs::write(
        &issue,
        r#"
LevitateOS Live - \l

"#,
    )?;

    Ok(())
}

/// Copy recstrap installer to the system.
/// Copy recstrap to the system.
///
/// # FAIL FAST - NO WARNINGS FOR REQUIRED COMPONENTS
///
/// recstrap is REQUIRED. Without it, the ISO cannot install itself.
/// A warning that scrolls by and gets ignored is WORTHLESS.
/// If recstrap is missing, the build MUST FAIL immediately.
///
/// DO NOT change this to a warning. DO NOT make it optional.
/// If you think it should be optional, you are WRONG.
fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let recstrap_src = ctx.base_dir.join("../recstrap/target/release/recstrap");
    let recstrap_dst = ctx.staging.join("usr/bin/recstrap");

    // FAIL FAST. No warning. No "optional". FAIL.
    if !recstrap_src.exists() {
        bail!(
            "recstrap not found at {}\n\
             \n\
             recstrap is REQUIRED - the ISO cannot install itself without it.\n\
             \n\
             Build it first:\n\
             \n\
             cd ../recstrap && cargo build --release\n\
             \n\
             DO NOT make this a warning. DO NOT skip this. FAIL FAST.",
            recstrap_src.display()
        );
    }

    fs::copy(&recstrap_src, &recstrap_dst)?;
    fs::set_permissions(&recstrap_dst, fs::Permissions::from_mode(0o755))?;
    println!("  Copied recstrap to /usr/bin/recstrap");

    Ok(())
}

