//! Systemd setup for installed system.
//!
//! This is different from the live initramfs - an installed system needs:
//! - Disk mount services (fsck, remount-fs)
//! - Real getty (not autologin by default)
//! - Networking services (networkd, resolved)
//! - Full service management

use anyhow::Result;
use std::fs;

use crate::rootfs::context::BuildContext;

/// Essential systemd unit files for an installed system.
const ESSENTIAL_UNITS: &[&str] = &[
    // Targets
    "basic.target",
    "sysinit.target",
    "multi-user.target",
    "default.target",
    "getty.target",
    "local-fs.target",
    "local-fs-pre.target",
    "remote-fs.target",
    "remote-fs-pre.target",
    "network.target",
    "network-pre.target",
    "network-online.target",
    "paths.target",
    "slices.target",
    "sockets.target",
    "timers.target",
    "swap.target",
    "shutdown.target",
    "rescue.target",
    "emergency.target",
    "reboot.target",
    "poweroff.target",
    "halt.target",
    "suspend.target",
    "sleep.target",
    "umount.target",
    "final.target",
    "graphical.target",
    // Services - core systemd
    "systemd-journald.service",
    "systemd-journald@.service",
    "systemd-udevd.service",
    "systemd-modules-load.service",
    "systemd-sysctl.service",
    "systemd-tmpfiles-setup.service",
    "systemd-tmpfiles-setup-dev.service",
    "systemd-tmpfiles-clean.service",
    "systemd-random-seed.service",
    "systemd-vconsole-setup.service",
    // Services - boot critical for disk systems
    "systemd-fsck-root.service",
    "systemd-fsck@.service",
    "systemd-remount-fs.service",
    "systemd-fstab-generator",
    // Services - authentication
    "systemd-logind.service",
    // Services - getty
    "getty@.service",
    "serial-getty@.service",
    "console-getty.service",
    "container-getty@.service",
    // Services - time/network
    "systemd-timedated.service",
    "systemd-hostnamed.service",
    "systemd-localed.service",
    "systemd-networkd.service",
    "systemd-resolved.service",
    "systemd-networkd-wait-online.service",
    // Services - misc
    "dbus.service",
    "dbus-broker.service",
    "chronyd.service",
    // Sockets
    "systemd-journald.socket",
    "systemd-journald-dev-log.socket",
    "systemd-journald-audit.socket",
    "systemd-udevd-control.socket",
    "systemd-udevd-kernel.socket",
    "dbus.socket",
    // Paths
    "systemd-ask-password-console.path",
    "systemd-ask-password-wall.path",
    // Slices
    "-.slice",
    "system.slice",
    "user.slice",
    "machine.slice",
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

/// Set up getty for installed system (no autologin).
pub fn setup_getty(ctx: &BuildContext) -> Result<()> {
    println!("Setting up getty...");

    // Enable getty on tty1
    let getty_wants = ctx
        .staging
        .join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants)?;

    let getty_link = getty_wants.join("getty@tty1.service");
    if !getty_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &getty_link)?;
    }

    // Enable getty.target from multi-user.target
    let multi_user_wants = ctx
        .staging
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let getty_target_link = multi_user_wants.join("getty.target");
    if !getty_target_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty.target", &getty_target_link)?;
    }

    println!("  Enabled getty@tty1.service");
    Ok(())
}

/// Set up serial console for installed system.
pub fn setup_serial_console(ctx: &BuildContext) -> Result<()> {
    println!("Setting up serial console...");

    // Enable serial-getty on ttyS0
    let getty_wants = ctx
        .staging
        .join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants)?;

    let serial_link = getty_wants.join("serial-getty@ttyS0.service");
    if !serial_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/serial-getty@.service",
            &serial_link,
        )?;
    }

    println!("  Enabled serial-getty@ttyS0.service");
    Ok(())
}

/// Set up systemd-networkd for networking.
pub fn setup_networkd(ctx: &BuildContext) -> Result<()> {
    println!("Setting up systemd-networkd...");

    // Create network configuration directory
    let network_dir = ctx.staging.join("etc/systemd/network");
    fs::create_dir_all(&network_dir)?;

    // Create default network configuration (DHCP on all interfaces)
    fs::write(
        network_dir.join("80-dhcp.network"),
        r#"[Match]
Name=en*
Name=eth*

[Network]
DHCP=yes
IPv6AcceptRA=yes

[DHCPv4]
UseDNS=yes
UseNTP=yes
UseHostname=yes
"#,
    )?;

    // Enable networkd
    let wants_dir = ctx
        .staging
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&wants_dir)?;

    let networkd_link = wants_dir.join("systemd-networkd.service");
    if !networkd_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/systemd-networkd.service",
            &networkd_link,
        )?;
    }

    // Enable resolved
    let resolved_link = wants_dir.join("systemd-resolved.service");
    if !resolved_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/systemd-resolved.service",
            &resolved_link,
        )?;
    }

    println!("  Enabled systemd-networkd and resolved");
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

/// Copy D-Bus configuration.
pub fn setup_dbus(ctx: &BuildContext) -> Result<()> {
    println!("Setting up D-Bus...");

    // Copy D-Bus system configuration
    let dbus_src = ctx.source.join("usr/share/dbus-1/system.d");
    let dbus_dst = ctx.staging.join("usr/share/dbus-1/system.d");

    if dbus_src.exists() {
        fs::create_dir_all(&dbus_dst)?;
        for entry in fs::read_dir(&dbus_src)? {
            let entry = entry?;
            let dst = dbus_dst.join(entry.file_name());
            fs::copy(entry.path(), &dst)?;
        }
    }

    // Copy D-Bus system services
    let services_src = ctx.source.join("usr/share/dbus-1/system-services");
    let services_dst = ctx.staging.join("usr/share/dbus-1/system-services");

    if services_src.exists() {
        fs::create_dir_all(&services_dst)?;
        for entry in fs::read_dir(&services_src)? {
            let entry = entry?;
            let dst = services_dst.join(entry.file_name());
            if entry.path().is_file() {
                fs::copy(entry.path(), &dst)?;
            }
        }
    }

    // Enable D-Bus socket
    let sockets_wants = ctx
        .staging
        .join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants)?;

    let dbus_socket_link = sockets_wants.join("dbus.socket");
    if !dbus_socket_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/dbus.socket", &dbus_socket_link)?;
    }

    println!("  Set up D-Bus");
    Ok(())
}

/// Copy udev rules.
pub fn copy_udev_rules(ctx: &BuildContext) -> Result<()> {
    println!("Copying udev rules...");

    let rules_src = ctx.source.join("usr/lib/udev/rules.d");
    let rules_dst = ctx.staging.join("usr/lib/udev/rules.d");

    if rules_src.exists() {
        fs::create_dir_all(&rules_dst)?;
        for entry in fs::read_dir(&rules_src)? {
            let entry = entry?;
            let dst = rules_dst.join(entry.file_name());
            fs::copy(entry.path(), &dst)?;
        }
        println!("  Copied udev rules");
    }

    Ok(())
}

/// Copy tmpfiles.d configuration.
pub fn copy_tmpfiles(ctx: &BuildContext) -> Result<()> {
    println!("Copying tmpfiles.d...");

    let tmpfiles_src = ctx.source.join("usr/lib/tmpfiles.d");
    let tmpfiles_dst = ctx.staging.join("usr/lib/tmpfiles.d");

    if tmpfiles_src.exists() {
        fs::create_dir_all(&tmpfiles_dst)?;
        for entry in fs::read_dir(&tmpfiles_src)? {
            let entry = entry?;
            let dst = tmpfiles_dst.join(entry.file_name());
            if entry.path().is_file() {
                fs::copy(entry.path(), &dst)?;
            }
        }
        println!("  Copied tmpfiles.d");
    }

    Ok(())
}

/// Copy sysctl.d configuration.
pub fn copy_sysctl(ctx: &BuildContext) -> Result<()> {
    println!("Copying sysctl.d...");

    let sysctl_src = ctx.source.join("usr/lib/sysctl.d");
    let sysctl_dst = ctx.staging.join("usr/lib/sysctl.d");

    if sysctl_src.exists() {
        fs::create_dir_all(&sysctl_dst)?;
        for entry in fs::read_dir(&sysctl_src)? {
            let entry = entry?;
            let dst = sysctl_dst.join(entry.file_name());
            if entry.path().is_file() {
                fs::copy(entry.path(), &dst)?;
            }
        }
        println!("  Copied sysctl.d");
    }

    Ok(())
}
