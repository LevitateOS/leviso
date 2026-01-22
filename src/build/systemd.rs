//! Systemd setup.
//!
//! Handles unit files, services, getty, udev, and D-Bus configuration.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::context::BuildContext;

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

    let unit_src = ctx.source.join("usr/lib/systemd/system");
    let unit_dst = ctx.staging.join("usr/lib/systemd/system");
    fs::create_dir_all(&unit_dst)?;

    let mut copied = 0;
    for unit in ESSENTIAL_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
            copied += 1;
        }
    }

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

/// Set up serial console.
pub fn setup_serial_console(ctx: &BuildContext) -> Result<()> {
    println!("Setting up serial console...");

    // Create custom serial console service
    let serial_console = ctx.staging.join("etc/systemd/system/serial-console.service");
    fs::write(
        &serial_console,
        r#"[Unit]
Description=Serial Console Shell
After=basic.target
Conflicts=rescue.service emergency.service

[Service]
Environment=HOME=/root
Environment=TERM=vt100
WorkingDirectory=/root
ExecStart=/bin/bash --login
StandardInput=tty
StandardOutput=tty
StandardError=tty
TTYPath=/dev/ttyS0
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=no
Type=idle
Restart=always
RestartSec=0

[Install]
WantedBy=multi-user.target
"#,
    )?;

    // Enable serial console
    let multi_user_wants = ctx.staging.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let serial_link = multi_user_wants.join("serial-console.service");
    if !serial_link.exists() {
        std::os::unix::fs::symlink("/etc/systemd/system/serial-console.service", &serial_link)?;
    }

    println!("  Enabled serial-console.service");
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

/// Set up D-Bus.
pub fn setup_dbus(ctx: &BuildContext) -> Result<()> {
    println!("Setting up D-Bus...");

    // Copy D-Bus system configuration
    let dbus_src = ctx.source.join("usr/share/dbus-1/system.d");
    let dbus_dst = ctx.staging.join("usr/share/dbus-1/system.d");

    if dbus_src.exists() {
        fs::create_dir_all(&dbus_dst)?;
        for entry in fs::read_dir(&dbus_src)? {
            let entry = entry?;
            fs::copy(entry.path(), dbus_dst.join(entry.file_name()))?;
        }
    }

    // Copy D-Bus system services
    let services_src = ctx.source.join("usr/share/dbus-1/system-services");
    let services_dst = ctx.staging.join("usr/share/dbus-1/system-services");

    if services_src.exists() {
        fs::create_dir_all(&services_dst)?;
        for entry in fs::read_dir(&services_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), services_dst.join(entry.file_name()))?;
            }
        }
    }

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

    // Copy rules
    let rules_src = ctx.source.join("usr/lib/udev/rules.d");
    let rules_dst = ctx.staging.join("usr/lib/udev/rules.d");

    if rules_src.exists() {
        fs::create_dir_all(&rules_dst)?;
        for entry in fs::read_dir(&rules_src)? {
            let entry = entry?;
            fs::copy(entry.path(), rules_dst.join(entry.file_name()))?;
        }
    }

    // Copy helpers
    let udev_src = ctx.source.join("usr/lib/udev");
    let udev_dst = ctx.staging.join("usr/lib/udev");
    fs::create_dir_all(&udev_dst)?;

    for helper in UDEV_HELPERS {
        let src = udev_src.join(helper);
        let dst = udev_dst.join(helper);
        if src.exists() && !dst.exists() {
            fs::copy(&src, &dst)?;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
        }
    }

    // Copy hwdb.d
    let hwdb_src = udev_src.join("hwdb.d");
    let hwdb_dst = udev_dst.join("hwdb.d");
    if hwdb_src.is_dir() {
        fs::create_dir_all(&hwdb_dst)?;
        for entry in fs::read_dir(&hwdb_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), hwdb_dst.join(entry.file_name()))?;
            }
        }
    }

    // Enable udev services
    let sysinit_wants = ctx.staging.join("etc/systemd/system/sysinit.target.wants");
    fs::create_dir_all(&sysinit_wants)?;

    for socket in ["systemd-udevd-control.socket", "systemd-udevd-kernel.socket"] {
        let link = sysinit_wants.join(socket);
        if !link.exists() {
            std::os::unix::fs::symlink(
                format!("/usr/lib/systemd/system/{}", socket),
                &link,
            )?;
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

    let src = ctx.source.join("usr/lib/tmpfiles.d");
    let dst = ctx.staging.join("usr/lib/tmpfiles.d");

    if src.exists() {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(&src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
        }
        println!("  Copied tmpfiles.d");
    }
    Ok(())
}

/// Copy sysctl.d configuration.
pub fn copy_sysctl(ctx: &BuildContext) -> Result<()> {
    println!("Copying sysctl.d...");

    let src = ctx.source.join("usr/lib/sysctl.d");
    let dst = ctx.staging.join("usr/lib/sysctl.d");

    if src.exists() {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(&src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
        }
        println!("  Copied sysctl.d");
    }
    Ok(())
}

/// Set up autologin for live environment (like archiso).
pub fn setup_autologin(staging: &Path) -> Result<()> {
    println!("Setting up autologin (like archiso)...");

    let console_service = staging.join("etc/systemd/system/console-autologin.service");
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

    let wants_dir = staging.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&wants_dir)?;

    // Disable default getty
    let getty_link = wants_dir.join("getty@tty1.service");
    if getty_link.exists() || getty_link.is_symlink() {
        fs::remove_file(&getty_link)?;
    }

    // Enable autologin
    std::os::unix::fs::symlink(
        "/etc/systemd/system/console-autologin.service",
        wants_dir.join("console-autologin.service"),
    )?;

    println!("  Configured console autologin on tty1");
    Ok(())
}
