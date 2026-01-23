//! Systemd setup.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::context::BuildContext;
use super::libdeps::{copy_dir_tree, copy_systemd_units as copy_units};

/// Essential systemd unit files.
const ESSENTIAL_UNITS: &[&str] = &[
    // Targets
    "basic.target", "sysinit.target", "multi-user.target", "default.target",
    "getty.target", "local-fs.target", "local-fs-pre.target",
    "remote-fs.target", "remote-fs-pre.target",
    "network.target", "network-pre.target", "network-online.target",
    "paths.target", "slices.target", "sockets.target", "timers.target",
    "swap.target", "shutdown.target", "rescue.target", "emergency.target",
    "reboot.target", "poweroff.target", "halt.target",
    "suspend.target", "sleep.target", "umount.target", "final.target",
    "graphical.target",
    // Services - core
    "systemd-journald.service", "systemd-journald@.service",
    "systemd-udevd.service", "systemd-udev-trigger.service",
    "systemd-modules-load.service", "systemd-sysctl.service",
    "systemd-tmpfiles-setup.service", "systemd-tmpfiles-setup-dev.service",
    "systemd-tmpfiles-clean.service",
    "systemd-random-seed.service", "systemd-vconsole-setup.service",
    // Services - disk
    "systemd-fsck-root.service", "systemd-fsck@.service",
    "systemd-remount-fs.service", "systemd-fstab-generator",
    // Services - auth
    "systemd-logind.service",
    // Services - getty
    "getty@.service", "serial-getty@.service",
    "console-getty.service", "container-getty@.service",
    // Services - time/network
    "systemd-timedated.service", "systemd-hostnamed.service",
    "systemd-localed.service", "systemd-networkd.service",
    "systemd-resolved.service", "systemd-networkd-wait-online.service",
    // Services - misc
    "dbus.service", "dbus-broker.service", "chronyd.service",
    // Services - SSH (remote installation/rescue)
    "sshd.service", "sshd@.service", "sshd.socket",
    "sshd-keygen.target", "sshd-keygen@.service",
    // Sockets
    "systemd-journald.socket", "systemd-journald-dev-log.socket",
    "systemd-journald-audit.socket",
    "systemd-udevd-control.socket", "systemd-udevd-kernel.socket",
    "dbus.socket",
    // Paths
    "systemd-ask-password-console.path", "systemd-ask-password-wall.path",
    // Slices
    "-.slice", "system.slice", "user.slice", "machine.slice",
];

/// D-Bus activation symlinks.
const DBUS_SYMLINKS: &[&str] = &[
    "dbus-org.freedesktop.timedate1.service",
    "dbus-org.freedesktop.hostname1.service",
    "dbus-org.freedesktop.locale1.service",
    "dbus-org.freedesktop.login1.service",
    "dbus-org.freedesktop.network1.service",
    "dbus-org.freedesktop.resolve1.service",
];

/// Udev helper binaries.
const UDEV_HELPERS: &[&str] = &[
    "ata_id", "scsi_id", "cdrom_id", "v4l_id", "dmi_memory_id", "mtd_probe",
];

/// Copy systemd unit files.
pub fn copy_systemd_units(ctx: &BuildContext) -> Result<()> {
    println!("Copying systemd units...");
    let copied = copy_units(ctx, ESSENTIAL_UNITS)?;
    println!("  Copied {}/{} unit files", copied, ESSENTIAL_UNITS.len());
    Ok(())
}

/// Copy D-Bus activation symlinks.
pub fn copy_dbus_symlinks(ctx: &BuildContext) -> Result<()> {
    println!("Copying D-Bus symlinks...");

    let unit_src = ctx.source.join("usr/lib/systemd/system");
    let unit_dst = ctx.staging.join("usr/lib/systemd/system");

    for symlink in DBUS_SYMLINKS {
        let src = unit_src.join(symlink);
        let dst = unit_dst.join(symlink);
        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            if !dst.exists() {
                std::os::unix::fs::symlink(&target, &dst)?;
            }
        }
    }
    Ok(())
}

/// Set up getty (standard, no autologin).
pub fn setup_getty(ctx: &BuildContext) -> Result<()> {
    println!("Setting up getty...");

    let getty_wants = ctx.staging.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants)?;

    let getty_link = getty_wants.join("getty@tty1.service");
    if !getty_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &getty_link)?;
    }

    let multi_user_wants = ctx.staging.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let getty_target_link = multi_user_wants.join("getty.target");
    if !getty_target_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty.target", &getty_target_link)?;
    }

    println!("  Enabled getty@tty1.service");
    Ok(())
}

/// Set default.target to multi-user.target.
pub fn set_default_target(ctx: &BuildContext) -> Result<()> {
    println!("Setting default target...");

    let default_link = ctx.staging.join("etc/systemd/system/default.target");
    if default_link.exists() || default_link.is_symlink() {
        fs::remove_file(&default_link).ok();
    }
    std::os::unix::fs::symlink("/usr/lib/systemd/system/multi-user.target", &default_link)?;

    println!("  Set default.target -> multi-user.target");
    Ok(())
}

/// Copy D-Bus configuration files and enable the D-Bus socket.
pub fn copy_dbus_configs(ctx: &BuildContext) -> Result<()> {
    println!("Copying D-Bus configs...");

    copy_dir_tree(ctx, "usr/share/dbus-1/system.d")?;
    copy_dir_tree(ctx, "usr/share/dbus-1/system-services")?;

    // Enable D-Bus socket
    let sockets_wants = ctx.staging.join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants)?;

    let dbus_socket_link = sockets_wants.join("dbus.socket");
    if !dbus_socket_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/dbus.socket", &dbus_socket_link)?;
    }

    println!("  Set up D-Bus");
    Ok(())
}

/// Copy udev rules and helpers.
pub fn copy_udev_rules(ctx: &BuildContext) -> Result<()> {
    println!("Copying udev rules...");

    copy_dir_tree(ctx, "usr/lib/udev/rules.d")?;
    copy_dir_tree(ctx, "usr/lib/udev/hwdb.d")?;

    // Copy helpers
    let udev_src = ctx.source.join("usr/lib/udev");
    let udev_dst = ctx.staging.join("usr/lib/udev");
    fs::create_dir_all(&udev_dst)?;

    for helper in UDEV_HELPERS {
        let src = udev_src.join(helper);
        let dst = udev_dst.join(helper);
        if src.exists() && !dst.exists() {
            fs::copy(&src, &dst)?;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        }
    }

    // Enable udev services
    let sysinit_wants = ctx.staging.join("etc/systemd/system/sysinit.target.wants");
    fs::create_dir_all(&sysinit_wants)?;

    for socket in ["systemd-udevd-control.socket", "systemd-udevd-kernel.socket"] {
        let link = sysinit_wants.join(socket);
        if !link.exists() {
            std::os::unix::fs::symlink(format!("/usr/lib/systemd/system/{}", socket), &link)?;
        }
    }

    let trigger_link = sysinit_wants.join("systemd-udev-trigger.service");
    if !trigger_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/systemd-udev-trigger.service",
            &trigger_link,
        )?;
    }

    println!("  Copied udev rules and enabled services");
    Ok(())
}

/// Copy tmpfiles.d configuration.
pub fn copy_tmpfiles(ctx: &BuildContext) -> Result<()> {
    println!("Copying tmpfiles.d...");
    let count = copy_dir_tree(ctx, "usr/lib/tmpfiles.d")?;
    if count > 0 {
        println!("  Copied {} tmpfiles.d entries", count);
    }
    Ok(())
}

/// Copy sysctl.d configuration.
pub fn copy_sysctl(ctx: &BuildContext) -> Result<()> {
    println!("Copying sysctl.d...");
    let count = copy_dir_tree(ctx, "usr/lib/sysctl.d")?;
    if count > 0 {
        println!("  Copied {} sysctl.d entries", count);
    }
    Ok(())
}

/// Configure systemd for live environment (archiso parity).
pub fn setup_live_systemd_configs(ctx: &BuildContext) -> Result<()> {
    println!("Setting up live systemd configs...");

    // Volatile journal storage
    let journald_dir = ctx.staging.join("etc/systemd/journald.conf.d");
    fs::create_dir_all(&journald_dir)?;
    fs::write(
        journald_dir.join("volatile.conf"),
        "[Journal]\nStorage=volatile\nRuntimeMaxUse=64M\n",
    )?;

    // Do-not-suspend config
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

/// Create live overlay directory with live-specific configs.
pub fn create_live_overlay(output_dir: &Path) -> Result<()> {
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
