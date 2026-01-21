//! Systemd init system setup.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::context::BuildContext;

/// Essential systemd binaries (required by systemd 255+).
const SYSTEMD_BINARIES: &[&str] = &[
    "systemd-executor", // CRITICAL: required since systemd 255
    "systemd-shutdown",
    "systemd-sulogin-shell",
    "systemd-cgroups-agent",
    "systemd-journald",
    "systemd-modules-load",
    "systemd-sysctl",
    "systemd-tmpfiles-setup",
    // D-Bus activated services (for timedatectl, hostnamectl, etc.)
    "systemd-timedated",
    "systemd-hostnamed",
    "systemd-localed",
];

/// Essential systemd unit files.
const ESSENTIAL_UNITS: &[&str] = &[
    // Targets
    "basic.target",
    "sysinit.target",
    "multi-user.target",
    "default.target",
    "getty.target",
    "local-fs.target",
    "local-fs-pre.target",
    "network.target",
    "network-pre.target",
    "paths.target",
    "slices.target",
    "sockets.target",
    "timers.target",
    "swap.target",
    "shutdown.target",
    "rescue.target",
    "emergency.target",
    // Services
    "getty@.service",
    "serial-getty@.service",
    "systemd-tmpfiles-setup.service",
    "systemd-journald.service",
    "systemd-udevd.service",
    // D-Bus activated services
    "systemd-timedated.service",
    "systemd-hostnamed.service",
    "systemd-localed.service",
    "systemd-logind.service",
    "chronyd.service",
    // Sockets
    "systemd-journald.socket",
    "systemd-journald-dev-log.socket",
];

/// D-Bus activation symlinks.
const DBUS_SYMLINKS: &[&str] = &[
    "dbus-org.freedesktop.timedate1.service",
    "dbus-org.freedesktop.hostname1.service",
    "dbus-org.freedesktop.locale1.service",
    "dbus-org.freedesktop.login1.service",
];

/// Libraries required by systemd.
const SYSTEMD_LIBS: &[&str] = &[
    "libacl.so.1",
    "libattr.so.1",
    "libaudit.so.1",
    "libblkid.so.1",
    "libcap-ng.so.0",
    "libcap.so.2",
    "libcrypto.so.3",
    "libcrypt.so.2",
    "libc.so.6",
    "libeconf.so.0",
    "libgcc_s.so.1",
    "libmount.so.1",
    "libm.so.6",
    "libpam.so.0",
    "libpcre2-8.so.0",
    "libseccomp.so.2",
    "libselinux.so.1",
    "libz.so.1",
    "ld-linux-x86-64.so.2",
];

/// Set up systemd as init.
pub fn setup_systemd(ctx: &BuildContext) -> Result<()> {
    println!("Setting up systemd...");

    // Copy main systemd binary
    copy_systemd_binary(ctx)?;

    // Copy systemd helper binaries
    copy_systemd_binaries(ctx)?;

    // Copy systemd private libraries
    copy_systemd_private_libs(ctx)?;

    // Copy systemd shared libraries
    copy_systemd_libs(ctx)?;

    // Create /sbin/init symlink
    create_init_symlink(ctx)?;

    // Copy unit files
    copy_systemd_units(ctx)?;

    // Copy D-Bus activation symlinks
    copy_dbus_symlinks(ctx)?;

    // Set up autologin getty
    setup_getty_autologin(ctx)?;

    // Set up serial console
    setup_serial_console(ctx)?;

    // Enable getty target
    enable_getty_target(ctx)?;

    // Create machine-id and os-release
    create_system_files(ctx)?;

    println!("  Configured autologin on tty1");

    Ok(())
}

/// Copy main systemd binary.
fn copy_systemd_binary(ctx: &BuildContext) -> Result<()> {
    let systemd_src = ctx.rootfs.join("usr/lib/systemd/systemd");
    let systemd_dst = ctx.initramfs.join("usr/lib/systemd/systemd");

    fs::create_dir_all(
        systemd_dst
            .parent()
            .context("systemd destination has no parent")?,
    )?;
    fs::copy(&systemd_src, &systemd_dst)
        .with_context(|| format!("Failed to copy systemd from {}", systemd_src.display()))?;

    let mut perms = fs::metadata(&systemd_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&systemd_dst, perms)?;
    println!("  Copied systemd");

    Ok(())
}

/// Copy essential systemd helper binaries.
fn copy_systemd_binaries(ctx: &BuildContext) -> Result<()> {
    let systemd_lib_dir = ctx.initramfs.join("usr/lib/systemd");

    for binary in SYSTEMD_BINARIES {
        let src = ctx.rootfs.join("usr/lib/systemd").join(binary);
        if src.exists() {
            let dst = systemd_lib_dir.join(binary);
            fs::copy(&src, &dst)?;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
            println!("  Copied {}", binary);
        } else {
            println!("  Warning: {} not found", binary);
        }
    }

    Ok(())
}

/// Copy systemd private libraries.
fn copy_systemd_private_libs(ctx: &BuildContext) -> Result<()> {
    let systemd_lib_src = ctx.rootfs.join("usr/lib64/systemd");
    if systemd_lib_src.exists() {
        fs::create_dir_all(ctx.initramfs.join("usr/lib64/systemd"))?;
        for entry in fs::read_dir(&systemd_lib_src)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                let dst = ctx.initramfs.join("usr/lib64/systemd").join(&name);
                fs::copy(entry.path(), &dst)?;
                println!("  Copied {}", name_str);
            }
        }
    }

    Ok(())
}

/// Copy systemd shared libraries.
fn copy_systemd_libs(ctx: &BuildContext) -> Result<()> {
    for lib in SYSTEMD_LIBS {
        let src_candidates = [
            ctx.rootfs.join("usr/lib64").join(lib),
            ctx.rootfs.join("lib64").join(lib),
        ];
        let dst = ctx.initramfs.join("lib64").join(lib);
        if !dst.exists() {
            for src in &src_candidates {
                if src.exists() {
                    fs::copy(src, &dst)?;
                    println!("  Copied {}", lib);
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Create /sbin/init symlink to systemd.
fn create_init_symlink(ctx: &BuildContext) -> Result<()> {
    let init_link = ctx.initramfs.join("sbin/init");
    if !init_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }
    Ok(())
}

/// Copy systemd unit files.
fn copy_systemd_units(ctx: &BuildContext) -> Result<()> {
    let unit_src = ctx.rootfs.join("usr/lib/systemd/system");
    let unit_dst = ctx.initramfs.join("usr/lib/systemd/system");

    for unit in ESSENTIAL_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
        }
    }

    println!("  Copied essential unit files");

    Ok(())
}

/// Copy D-Bus activation symlinks.
fn copy_dbus_symlinks(ctx: &BuildContext) -> Result<()> {
    let unit_src = ctx.rootfs.join("usr/lib/systemd/system");
    let unit_dst = ctx.initramfs.join("usr/lib/systemd/system");

    for symlink in DBUS_SYMLINKS {
        let src = unit_src.join(symlink);
        let dst = unit_dst.join(symlink);
        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            if !dst.exists() {
                std::os::unix::fs::symlink(&target, &dst)?;
                println!("  Created symlink: {} -> {}", symlink, target.display());
            }
        }
    }

    Ok(())
}

/// Set up autologin getty for tty1.
fn setup_getty_autologin(ctx: &BuildContext) -> Result<()> {
    let getty_override_dir = ctx
        .initramfs
        .join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&getty_override_dir)?;

    fs::write(
        getty_override_dir.join("autologin.conf"),
        r#"[Service]
ExecStart=
ExecStart=-/bin/agetty --autologin root --noclear --login-program /bin/bash --login-options '-l' %I linux
Type=idle
"#,
    )?;

    Ok(())
}

/// Set up serial console service.
fn setup_serial_console(ctx: &BuildContext) -> Result<()> {
    let serial_console = ctx
        .initramfs
        .join("etc/systemd/system/serial-console.service");
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

    // Enable serial-console
    let multi_user_wants = ctx
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let serial_link = multi_user_wants.join("serial-console.service");
    if !serial_link.exists() {
        std::os::unix::fs::symlink("/etc/systemd/system/serial-console.service", &serial_link)?;
    }

    Ok(())
}

/// Enable getty.target from multi-user.target.
fn enable_getty_target(ctx: &BuildContext) -> Result<()> {
    // Enable getty on tty1
    let getty_wants = ctx
        .initramfs
        .join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants)?;

    let getty_link = getty_wants.join("getty@tty1.service");
    if !getty_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &getty_link)?;
    }

    // Enable getty.target from multi-user.target
    let multi_user_wants = ctx
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let getty_target_link = multi_user_wants.join("getty.target");
    if !getty_target_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty.target", &getty_target_link)?;
    }

    Ok(())
}

/// Create machine-id and os-release files.
fn create_system_files(ctx: &BuildContext) -> Result<()> {
    // Empty machine-id (systemd will populate on first boot)
    fs::write(ctx.initramfs.join("etc/machine-id"), "")?;

    // os-release
    fs::write(
        ctx.initramfs.join("etc/os-release"),
        r#"NAME="LevitateOS"
ID=levitateos
VERSION="1.0"
PRETTY_NAME="LevitateOS Live"
"#,
    )?;

    Ok(())
}
