//! D-Bus message bus setup.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::binary::{copy_library, get_all_dependencies};
use super::context::BuildContext;
use super::users;

/// D-Bus binaries to copy.
const DBUS_BINARIES: &[&str] = &[
    "dbus-broker",
    "dbus-broker-launch",
    "dbus-send",
    "dbus-daemon",
    "busctl",
];

/// D-Bus policy files needed.
const NEEDED_POLICIES: &[&str] = &[
    "org.freedesktop.systemd1.conf",
    "org.freedesktop.hostname1.conf",
    "org.freedesktop.locale1.conf",
    "org.freedesktop.timedate1.conf",
    "org.freedesktop.login1.conf",
];

/// D-Bus service activation files needed.
const NEEDED_SERVICES: &[&str] = &[
    "org.freedesktop.systemd1.service",
    "org.freedesktop.hostname1.service",
    "org.freedesktop.locale1.service",
    "org.freedesktop.timedate1.service",
    "org.freedesktop.login1.service",
];

/// D-Bus systemd units.
const DBUS_UNITS: &[&str] = &["dbus.socket", "dbus-daemon.service"];

/// Set up D-Bus message bus.
pub fn setup_dbus(ctx: &BuildContext) -> Result<()> {
    println!("Setting up D-Bus...");

    // Create D-Bus directories
    fs::create_dir_all(ctx.staging.join("usr/share/dbus-1"))?;
    fs::create_dir_all(ctx.staging.join("etc/dbus-1"))?;
    fs::create_dir_all(ctx.staging.join("run/dbus"))?;
    fs::create_dir_all(ctx.staging.join("usr/bin"))?;

    // Copy D-Bus binaries
    copy_dbus_binaries(ctx)?;

    // Copy D-Bus configs
    copy_dbus_configs(ctx)?;

    // Copy D-Bus systemd units
    copy_dbus_units(ctx)?;

    // Enable D-Bus socket
    enable_dbus_socket(ctx)?;

    // Enable journald sockets
    enable_journald_sockets(ctx)?;

    // Ensure dbus user exists
    ensure_dbus_user(ctx)?;

    println!("  D-Bus configured");

    Ok(())
}

/// Copy D-Bus binaries and their libraries.
fn copy_dbus_binaries(ctx: &BuildContext) -> Result<()> {
    for binary in DBUS_BINARIES {
        let src = ctx.source.join("usr/bin").join(binary);
        if src.exists() {
            let dst = ctx.staging.join("usr/bin").join(binary);
            fs::copy(&src, &dst)?;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
            println!("  Copied {}", binary);

            // Get library dependencies using readelf (cross-compilation safe)
            let libs = get_all_dependencies(&ctx.source, &src, &[])?;
            for lib_name in &libs {
                if let Err(e) = copy_library(ctx, lib_name) {
                    println!("  Warning: Failed to copy library {}: {}", lib_name, e);
                }
            }
        }
    }

    Ok(())
}

/// Copy D-Bus configuration files.
fn copy_dbus_configs(ctx: &BuildContext) -> Result<()> {
    let dbus_conf_src = ctx.source.join("usr/share/dbus-1");
    let dbus_conf_dst = ctx.staging.join("usr/share/dbus-1");

    // Copy system.conf
    if dbus_conf_src.join("system.conf").exists() {
        fs::copy(
            dbus_conf_src.join("system.conf"),
            dbus_conf_dst.join("system.conf"),
        )?;
    }

    // Copy policy files
    let system_d_src = dbus_conf_src.join("system.d");
    let system_d_dst = dbus_conf_dst.join("system.d");
    fs::create_dir_all(&system_d_dst)?;

    for policy in NEEDED_POLICIES {
        let src = system_d_src.join(policy);
        let dst = system_d_dst.join(policy);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied D-Bus policy: {}", policy);
        }
    }

    // Copy service activation files
    let services_src = dbus_conf_src.join("system-services");
    let services_dst = dbus_conf_dst.join("system-services");
    fs::create_dir_all(&services_dst)?;

    for service in NEEDED_SERVICES {
        let src = services_src.join(service);
        let dst = services_dst.join(service);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied D-Bus service: {}", service);
        }
    }

    Ok(())
}

/// Copy D-Bus systemd unit files.
fn copy_dbus_units(ctx: &BuildContext) -> Result<()> {
    let unit_src = ctx.source.join("usr/lib/systemd/system");
    let unit_dst = ctx.staging.join("usr/lib/systemd/system");

    for unit in DBUS_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied {}", unit);
        }
    }

    // Create dbus.service symlink to dbus-daemon.service
    let dbus_service_link = unit_dst.join("dbus.service");
    if !dbus_service_link.exists() {
        std::os::unix::fs::symlink("dbus-daemon.service", &dbus_service_link)?;
    }

    Ok(())
}

/// Enable D-Bus socket in sockets.target.wants.
fn enable_dbus_socket(ctx: &BuildContext) -> Result<()> {
    let sockets_wants = ctx
        .staging
        .join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants)?;

    let dbus_socket_link = sockets_wants.join("dbus.socket");
    if !dbus_socket_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/dbus.socket", &dbus_socket_link)?;
    }

    Ok(())
}

/// Enable journald sockets (fixes "Failed to connect stdout to the journal socket").
fn enable_journald_sockets(ctx: &BuildContext) -> Result<()> {
    let sockets_wants = ctx
        .staging
        .join("etc/systemd/system/sockets.target.wants");

    let journald_socket_link = sockets_wants.join("systemd-journald.socket");
    if !journald_socket_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/systemd-journald.socket",
            &journald_socket_link,
        )?;
    }

    let journald_dev_log_link = sockets_wants.join("systemd-journald-dev-log.socket");
    if !journald_dev_log_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/systemd-journald-dev-log.socket",
            &journald_dev_log_link,
        )?;
    }

    Ok(())
}

/// Ensure dbus user and group exist.
fn ensure_dbus_user(ctx: &BuildContext) -> Result<()> {
    users::ensure_user(
        &ctx.source,
        &ctx.staging,
        "dbus",
        81,
        81,
        "/",
        "/sbin/nologin",
    )?;
    users::ensure_group(&ctx.source, &ctx.staging, "dbus", 81)?;
    Ok(())
}
