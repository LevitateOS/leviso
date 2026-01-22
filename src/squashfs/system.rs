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

use anyhow::Result;
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
    build::systemd::setup_autologin(&ctx.staging)?; // Like archiso - live ISO boots to root shell
    build::systemd::setup_serial_console(ctx)?;
    build::systemd::set_default_target(ctx)?; // multi-user.target
    build::systemd::copy_dbus_configs(ctx)?;
    build::systemd::copy_udev_rules(ctx)?;
    build::systemd::copy_tmpfiles(ctx)?;
    build::systemd::copy_sysctl(ctx)?;

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

    // ALL firmware (daily driver needs everything)
    copy_all_firmware(ctx)?;

    // Keymaps (for loadkeys command)
    copy_keymaps(ctx)?;

    // Kernel
    copy_kernel(ctx)?;

    Ok(())
}

/// Phase 13-17: Final setup (init symlink, root access, welcome, recstrap).
fn build_final_setup(ctx: &BuildContext) -> Result<()> {
    // Create /sbin/init symlink
    let init_link = ctx.staging.join("usr/sbin/init");
    if !init_link.exists() && !init_link.is_symlink() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }
    // Note: /sbin/init exists via merged /usr (/sbin -> /usr/sbin)
    // The symlink chain: /sbin/init -> /usr/sbin/init -> /usr/lib/systemd/systemd

    // Create root user (for live boot)
    // Note: etc files already set up root user in /etc/passwd
    // For live boot, we want passwordless root access
    setup_live_root_access(ctx)?;

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
        println!("  Warning: Keymaps not found at {}", keymaps_src.display());
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
        println!("  Warning: Dracut modules not found at {}", dracut_src.display());
    }

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
        println!("  Warning: No firmware directory found");
        return Ok(());
    };

    fs::create_dir_all(&firmware_dst)?;

    let size = copy_dir_recursive(actual_src, &firmware_dst)?;
    println!(
        "  Copied all firmware ({:.1} MB)",
        size as f64 / 1_000_000.0
    );

    Ok(())
}

/// Copy kernel to squashfs (required for installation).
fn copy_kernel(ctx: &BuildContext) -> Result<()> {
    let kernel_dst = ctx.staging.join("boot/vmlinuz");

    // Check sources in order of preference
    let levitate_kernel = ctx.base_dir.join("output/staging/boot/vmlinuz");
    let rocky_kernel = ctx.base_dir.join("downloads/iso-contents/images/pxeboot/vmlinuz");

    let kernel_src = if levitate_kernel.exists() {
        println!("Using LevitateOS kernel");
        levitate_kernel
    } else if rocky_kernel.exists() {
        println!("Using Rocky kernel (fallback)");
        rocky_kernel
    } else {
        println!("Warning: No kernel found for squashfs");
        println!("  Installation to disk will not have a kernel");
        return Ok(());
    };

    fs::create_dir_all(ctx.staging.join("boot"))?;
    fs::copy(&kernel_src, &kernel_dst)?;
    println!("  Copied kernel to /boot/vmlinuz");

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

/// Set up passwordless root access for live boot.
fn setup_live_root_access(ctx: &BuildContext) -> Result<()> {
    println!("Setting up live root access...");

    // Update shadow to allow passwordless root login
    // The '!' in shadow means locked, empty means no password
    let shadow_path = ctx.staging.join("etc/shadow");
    if shadow_path.exists() {
        let content = fs::read_to_string(&shadow_path)?;
        // Replace "root:!:" with "root::" (empty password)
        let content = content.replace("root:!:", "root::");
        fs::write(&shadow_path, content)?;
    }

    println!("  Root has empty password for live boot");
    println!("  (Installation will prompt for password)");

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
   genfstab -U /mnt >> /mnt/etc/fstab

   # 6. Chroot and configure
   arch-chroot /mnt
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
fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    // recstrap is built in the sibling recstrap/ directory
    let recstrap_src = ctx.base_dir.join("../recstrap/target/release/recstrap");
    let recstrap_dst = ctx.staging.join("usr/bin/recstrap");

    if recstrap_src.exists() {
        fs::copy(&recstrap_src, &recstrap_dst)?;
        // Make executable
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&recstrap_dst, fs::Permissions::from_mode(0o755))?;
        println!("  Copied recstrap installer to /usr/bin/recstrap");
    } else {
        println!("  Warning: recstrap not found at {}", recstrap_src.display());
        println!("    Build it with: cd ../recstrap && cargo build --release");
    }

    Ok(())
}

