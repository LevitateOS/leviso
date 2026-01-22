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
///
/// Note: For full D-Bus setup (binaries, units, users), use `dbus::setup_dbus`.
/// This function handles the systemd-related D-Bus configuration only.
pub fn copy_dbus_configs(ctx: &BuildContext) -> Result<()> {
    println!("Copying D-Bus configs...");

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

/// Create live overlay directory with live-specific configs.
///
/// These configs are ONLY applied during live boot (via init_tiny overlay).
/// They are NOT extracted to installed systems by recstrap.
///
/// Contents:
/// - console-autologin.service: Root autologin on tty1 (like archiso)
/// - serial-console.service: Serial console for QEMU testing
/// - etc/shadow: Root with empty password for live boot
pub fn create_live_overlay(output_dir: &Path) -> Result<()> {
    println!("Creating live overlay directory...");

    let overlay_dir = output_dir.join("live-overlay");

    // Clean up previous overlay if it exists
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    let systemd_dir = overlay_dir.join("etc/systemd/system");
    let getty_wants = systemd_dir.join("getty.target.wants");
    let multi_user_wants = systemd_dir.join("multi-user.target.wants");

    // Create directory structure
    fs::create_dir_all(&getty_wants)?;
    fs::create_dir_all(&multi_user_wants)?;
    fs::create_dir_all(overlay_dir.join("etc"))?;

    // === Console Autologin Service ===
    // Like archiso - live ISO boots directly to root shell on tty1
    let console_service = systemd_dir.join("console-autologin.service");
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

    // Enable autologin (symlink in getty.target.wants)
    std::os::unix::fs::symlink(
        "../console-autologin.service",
        getty_wants.join("console-autologin.service"),
    )?;

    // Create a drop-in to disable getty@tty1 during live boot
    // This prevents the normal login prompt from competing with autologin
    let getty_override_dir = systemd_dir.join("getty@tty1.service.d");
    fs::create_dir_all(&getty_override_dir)?;
    fs::write(
        getty_override_dir.join("live-disable.conf"),
        r#"# Disable getty@tty1 during live boot - console-autologin.service handles tty1
# The ! means "condition fails if path exists" - so getty won't start during live boot
[Unit]
ConditionPathExists=!/live-boot-marker
"#,
    )?;

    // Create a marker file that only exists during live boot
    // (init_tiny will create /live-boot-marker before switch_root)
    // This is how we conditionally disable getty@tty1

    // === Serial Console Service ===
    // For QEMU testing with -serial
    let serial_service = systemd_dir.join("serial-console.service");
    fs::write(
        &serial_service,
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
    std::os::unix::fs::symlink(
        "../serial-console.service",
        multi_user_wants.join("serial-console.service"),
    )?;

    // === Shadow file with empty root password ===
    // During live boot, this overlays the base system's /etc/shadow (which has root:!)
    // Result: root has empty password, can login without password
    fs::write(
        overlay_dir.join("etc/shadow"),
        r#"root::19000:0:99999:7:::
bin:*:19000:0:99999:7:::
daemon:*:19000:0:99999:7:::
nobody:*:19000:0:99999:7:::
systemd-network:!*:19000::::::
systemd-resolve:!*:19000::::::
systemd-timesync:!*:19000::::::
systemd-coredump:!*:19000::::::
dbus:!*:19000::::::
chrony:!*:19000::::::
"#,
    )?;

    // Set proper permissions on shadow
    let mut perms = fs::metadata(overlay_dir.join("etc/shadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(overlay_dir.join("etc/shadow"), perms)?;

    println!("  Created live overlay with:");
    println!("    - console-autologin.service (root autologin on tty1)");
    println!("    - serial-console.service (serial shell for QEMU)");
    println!("    - /etc/shadow (root with empty password)");

    Ok(())
}
