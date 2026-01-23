//! Live ISO overlay operations.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::build::context::BuildContext;

/// Create live overlay directory with autologin, serial console, empty root password.
///
/// This is called by iso.rs during ISO creation. The overlay is applied ONLY
/// during live boot, not extracted to installed systems.
pub fn create_live_overlay_at(output_dir: &Path) -> Result<()> {
    println!("Creating live overlay directory...");

    let overlay_dir = output_dir.join("live-overlay");
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    let systemd_dir = overlay_dir.join("etc/systemd/system");
    let getty_wants = systemd_dir.join("getty.target.wants");
    let multi_user_wants = systemd_dir.join("multi-user.target.wants");

    fs::create_dir_all(&getty_wants)?;
    fs::create_dir_all(&multi_user_wants)?;
    fs::create_dir_all(overlay_dir.join("etc"))?;

    // Console autologin service
    fs::write(
        systemd_dir.join("console-autologin.service"),
        "[Unit]\n\
         Description=Console Autologin\n\
         After=systemd-user-sessions.service getty-pre.target\n\
         Before=getty.target\n\n\
         [Service]\n\
         Environment=HOME=/root\nEnvironment=TERM=linux\n\
         WorkingDirectory=/root\nExecStart=/bin/bash --login\n\
         StandardInput=tty\nStandardOutput=tty\nStandardError=tty\n\
         TTYPath=/dev/tty1\nTTYReset=yes\nTTYVHangup=yes\nTTYVTDisallocate=yes\n\
         Type=idle\nRestart=always\nRestartSec=0\n\n\
         [Install]\nWantedBy=getty.target\n",
    )?;

    std::os::unix::fs::symlink(
        "../console-autologin.service",
        getty_wants.join("console-autologin.service"),
    )?;

    // Disable getty@tty1 during live boot
    let getty_override = systemd_dir.join("getty@tty1.service.d");
    fs::create_dir_all(&getty_override)?;
    fs::write(
        getty_override.join("live-disable.conf"),
        "[Unit]\nConditionPathExists=!/live-boot-marker\n",
    )?;

    // Serial console service
    fs::write(
        systemd_dir.join("serial-console.service"),
        "[Unit]\n\
         Description=Serial Console Shell\n\
         After=basic.target\nConflicts=rescue.service emergency.service\n\n\
         [Service]\n\
         Environment=HOME=/root\nEnvironment=TERM=vt100\n\
         WorkingDirectory=/root\nExecStart=/bin/bash --login\n\
         StandardInput=tty\nStandardOutput=tty\nStandardError=tty\n\
         TTYPath=/dev/ttyS0\nTTYReset=yes\nTTYVHangup=yes\nTTYVTDisallocate=no\n\
         Type=idle\nRestart=always\nRestartSec=0\n\n\
         [Install]\nWantedBy=multi-user.target\n",
    )?;

    std::os::unix::fs::symlink(
        "../serial-console.service",
        multi_user_wants.join("serial-console.service"),
    )?;

    // Shadow file with empty root password
    fs::write(
        overlay_dir.join("etc/shadow"),
        "root::19000:0:99999:7:::\n\
         bin:*:19000:0:99999:7:::\ndaemon:*:19000:0:99999:7:::\nnobody:*:19000:0:99999:7:::\n\
         systemd-network:!*:19000::::::\nsystemd-resolve:!*:19000::::::\n\
         systemd-timesync:!*:19000::::::\nsystemd-coredump:!*:19000::::::\n\
         dbus:!*:19000::::::\nchrony:!*:19000::::::\n",
    )?;

    fs::set_permissions(
        overlay_dir.join("etc/shadow"),
        fs::Permissions::from_mode(0o600),
    )?;

    println!("  Created live overlay");
    Ok(())
}

/// Create live overlay (wrapper for BuildContext).
pub fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    create_live_overlay_at(&ctx.output)
}

/// Create welcome message (MOTD) for live environment.
pub fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
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

    let issue = ctx.staging.join("etc/issue");
    fs::write(&issue, "\nLevitateOS Live - \\l\n\n")?;

    Ok(())
}

/// Copy installation tools (recstrap, recfstab, recchroot) to staging.
pub fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    use leviso_deps::DependencyResolver;

    let resolver = DependencyResolver::new(&ctx.base_dir)?;

    let (recstrap, recfstab, recchroot) = resolver
        .all_tools()
        .context("Installation tools are REQUIRED - the ISO cannot install itself without them")?;

    for tool in [&recstrap, &recfstab, &recchroot] {
        let dst = ctx.staging.join("usr/bin").join(tool.tool.name());
        fs::copy(&tool.path, &dst)?;
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        println!(
            "  Copied {} to /usr/bin/{} (from {:?})",
            tool.tool.name(),
            tool.tool.name(),
            tool.source
        );
    }

    Ok(())
}

/// Set up live systemd configurations (volatile journal, no suspend).
pub fn setup_live_systemd_configs(ctx: &BuildContext) -> Result<()> {
    println!("Setting up live systemd configs...");

    let journald_dir = ctx.staging.join("etc/systemd/journald.conf.d");
    fs::create_dir_all(&journald_dir)?;
    fs::write(
        journald_dir.join("volatile.conf"),
        "[Journal]\nStorage=volatile\nRuntimeMaxUse=64M\n",
    )?;

    let logind_dir = ctx.staging.join("etc/systemd/logind.conf.d");
    fs::create_dir_all(&logind_dir)?;
    fs::write(
        logind_dir.join("do-not-suspend.conf"),
        "[Login]\nHandleSuspendKey=ignore\nHandleHibernateKey=ignore\n\
         HandleLidSwitch=ignore\nHandleLidSwitchExternalPower=ignore\nIdleAction=ignore\n",
    )?;

    println!("  Created live systemd configs");
    Ok(())
}
