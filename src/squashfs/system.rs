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

/// Build the complete system into the staging directory.
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("Building complete system for squashfs...");

    // === PHASE 1: Filesystem structure ===
    build::filesystem::create_fhs_structure(&ctx.staging)?;
    build::filesystem::create_symlinks(&ctx.staging)?;

    // === PHASE 2: Binaries (complete set) ===
    // 87+ BIN + 38+ SBIN + systemd + sudo + login
    build::binaries::copy_shell(ctx)?;
    build::binaries::copy_coreutils(ctx)?;
    build::binaries::copy_sbin_utils(ctx)?;
    build::binaries::copy_systemd_binaries(ctx)?;
    build::binaries::copy_login_binaries(ctx)?;
    build::binaries::copy_sudo_libs(ctx)?;

    // === PHASE 3: Systemd units (full targets) ===
    build::systemd::copy_systemd_units(ctx)?;
    build::systemd::copy_dbus_symlinks(ctx)?;
    build::systemd::setup_getty(ctx)?;
    setup_autologin(ctx)?; // Like archiso - live ISO boots to root shell
    build::systemd::setup_serial_console(ctx)?;
    build::systemd::set_default_target(ctx)?; // multi-user.target
    build::systemd::setup_dbus(ctx)?;
    build::systemd::copy_udev_rules(ctx)?;
    build::systemd::copy_tmpfiles(ctx)?;
    build::systemd::copy_sysctl(ctx)?;

    // === PHASE 4: Networking ===
    // NetworkManager + wpa_supplicant + helpers + WiFi firmware
    build::network::setup_network(ctx)?;

    // === PHASE 5: D-Bus ===
    build::dbus::setup_dbus(ctx)?;

    // === PHASE 6: Chrony NTP ===
    build::users::ensure_chrony_user(ctx)?;
    build::chrony::setup_chrony(ctx)?;

    // === PHASE 7: Kernel modules ===
    // Copy all modules - daily driver needs hardware support
    let config = crate::config::Config::load(&ctx.base_dir);
    let module_list = config.all_modules();
    build::modules::setup_modules(ctx, &module_list)?;

    // === PHASE 8: PAM (full authentication for installed system) ===
    // Note: Live ISO uses autologin (like archiso), installed system uses PAM
    build::pam::setup_pam(ctx)?;
    build::pam::copy_pam_modules(ctx)?;
    build::pam::create_security_config(ctx)?;

    // === PHASE 9: /etc configuration ===
    build::etc::create_etc_files(ctx)?;
    build::etc::copy_timezone_data(ctx)?;
    build::etc::copy_locales(ctx)?;

    // Empty machine-id - systemd generates on first boot
    let machine_id = ctx.staging.join("etc/machine-id");
    fs::write(&machine_id, "")?;

    // Disable SELinux - we don't ship policies
    disable_selinux(ctx)?;

    // === PHASE 10: Recipe package manager ===
    build::recipe::copy_recipe(ctx)?;
    build::recipe::setup_recipe_config(ctx)?;

    // === PHASE 11: ALL firmware (daily driver needs everything) ===
    copy_all_firmware(ctx)?;

    // === PHASE 11b: Keymaps (for loadkeys command) ===
    copy_keymaps(ctx)?;

    // === PHASE 12: Kernel ===
    // Copy kernel from output/staging/boot/vmlinuz if built, otherwise skip
    // (ISO will use kernel separately)
    copy_kernel(ctx)?;

    // === PHASE 13: Create /sbin/init symlink ===
    let init_link = ctx.staging.join("usr/sbin/init");
    if !init_link.exists() && !init_link.is_symlink() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }

    // Also ensure /sbin/init exists (via merged /usr)
    let sbin_init = ctx.staging.join("sbin/init");
    if !sbin_init.exists() && !sbin_init.is_symlink() {
        // /sbin -> /usr/sbin, so /sbin/init -> /usr/sbin/init -> /usr/lib/systemd/systemd
        // The symlink chain is already set up by create_symlinks
        ()
    }

    // === PHASE 14: Create root user (for live boot) ===
    // Note: etc files already set up root user in /etc/passwd
    // For live boot, we want passwordless root access
    setup_live_root_access(ctx)?;

    // === PHASE 15: Create init script for live boot ===
    // Note: For squashfs, we use switch_root instead of rdinit
    // The tiny initramfs handles mounting squashfs, then switch_root to /sbin/init

    // === PHASE 16: Create welcome message ===
    create_welcome_message(ctx)?;

    // === PHASE 17: Copy recstrap installer ===
    copy_recstrap(ctx)?;

    println!("System build complete.");
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

/// Copy kernel if built.
fn copy_kernel(ctx: &BuildContext) -> Result<()> {
    let kernel_src = ctx.base_dir.join("output/staging/boot/vmlinuz");
    let kernel_dst = ctx.staging.join("boot/vmlinuz");

    if kernel_src.exists() {
        fs::create_dir_all(ctx.staging.join("boot"))?;
        fs::copy(&kernel_src, &kernel_dst)?;
        println!("Copied kernel to squashfs");
    } else {
        println!("Note: Kernel not found at output/staging/boot/vmlinuz");
        println!("  ISO will use kernel from separate location");
    }

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

 To install to disk:
   recstrap /dev/vda

 For networking:
   nmcli device status              # Show network devices
   nmcli device wifi list           # List WiFi networks
   nmcli device wifi connect SSID password PASSWORD

 Other commands:
   lsblk                            # List block devices
   recstrap --help                  # Installation options

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

/// Set up autologin for live ISO (like archiso).
///
/// archiso boots directly to a root shell without login prompt.
/// This is the expected UX for a live installation environment.
fn setup_autologin(ctx: &BuildContext) -> Result<()> {
    println!("Setting up autologin (like archiso)...");

    // Create a simple console service that runs bash directly (like serial-console.service)
    // This is simpler and more reliable than the agetty/login approach
    let console_service = ctx.staging.join("etc/systemd/system/console-autologin.service");
    fs::write(
        &console_service,
        r#"[Unit]
Description=Console Autologin
After=systemd-user-sessions.service getty-pre.target
Before=getty.target

[Service]
Environment=HOME=/root
Environment=TERM=linux
WorkingDirectory=/root
ExecStart=/bin/bash --login
StandardInput=tty
StandardOutput=tty
StandardError=tty
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=yes
Type=idle
Restart=always
RestartSec=0

[Install]
WantedBy=getty.target
"#,
    )?;

    // Disable the default getty@tty1 and enable our autologin service
    let wants_dir = ctx.staging.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&wants_dir)?;

    // Remove default getty@tty1 symlink if it exists
    let getty_link = wants_dir.join("getty@tty1.service");
    if getty_link.exists() || getty_link.is_symlink() {
        fs::remove_file(&getty_link)?;
    }

    // Enable our autologin service
    std::os::unix::fs::symlink(
        "/etc/systemd/system/console-autologin.service",
        wants_dir.join("console-autologin.service"),
    )?;

    println!("  Configured console autologin on tty1");
    Ok(())
}

/// Copy directory recursively and return total size in bytes.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<u64> {
    let mut total_size: u64 = 0;

    if !src.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().unwrap();
        let dest_path = dst.join(filename);

        if path.is_dir() {
            total_size += copy_dir_recursive(&path, &dest_path)?;
        } else if path.is_symlink() {
            // Preserve symlinks
            let target = fs::read_link(&path)?;
            if !dest_path.exists() && !dest_path.is_symlink() {
                std::os::unix::fs::symlink(&target, &dest_path)?;
            }
        } else {
            fs::copy(&path, &dest_path)?;
            if let Ok(meta) = fs::metadata(&dest_path) {
                total_size += meta.len();
            }
        }
    }

    Ok(total_size)
}
