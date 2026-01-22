//! D-Bus message bus setup.

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use crate::common::binary::create_symlink_if_missing;

use super::context::BuildContext;
use super::libdeps::{copy_binary_with_libs, copy_dir_tree, copy_systemd_units};
use super::users;

/// D-Bus binaries to copy.
const BINARIES: &[&str] = &["dbus-broker", "dbus-broker-launch", "dbus-send", "dbus-daemon", "busctl"];

/// D-Bus systemd units.
const UNITS: &[&str] = &["dbus.socket", "dbus-daemon.service"];

/// Set up D-Bus message bus.
pub fn setup_dbus(ctx: &BuildContext) -> Result<()> {
    println!("Setting up D-Bus...");

    // Create directories
    fs::create_dir_all(ctx.staging.join("run/dbus"))?;

    // Copy binaries (all required - D-Bus is critical)
    for bin in BINARIES {
        if !copy_binary_with_libs(ctx, bin, "usr/bin")? {
            bail!("{} not found - D-Bus is required for systemd", bin);
        }
    }

    // Copy configs
    copy_dir_tree(ctx, "usr/share/dbus-1")?;
    copy_dir_tree(ctx, "etc/dbus-1")?;

    // Copy and enable systemd units
    copy_systemd_units(ctx, UNITS)?;

    let unit_dst = ctx.staging.join("usr/lib/systemd/system");
    let dbus_service_link = unit_dst.join("dbus.service");
    if !dbus_service_link.exists() {
        std::os::unix::fs::symlink("dbus-daemon.service", &dbus_service_link)?;
    }

    // Enable D-Bus socket
    let sockets_wants = ctx.staging.join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants)?;
    create_symlink_if_missing(
        Path::new("/usr/lib/systemd/system/dbus.socket"),
        &sockets_wants.join("dbus.socket"),
    )?;

    // Enable journald sockets
    create_symlink_if_missing(
        Path::new("/usr/lib/systemd/system/systemd-journald.socket"),
        &sockets_wants.join("systemd-journald.socket"),
    )?;
    create_symlink_if_missing(
        Path::new("/usr/lib/systemd/system/systemd-journald-dev-log.socket"),
        &sockets_wants.join("systemd-journald-dev-log.socket"),
    )?;

    // Ensure dbus user exists
    users::ensure_user(&ctx.source, &ctx.staging, "dbus", 81, 81, "/", "/sbin/nologin")?;
    users::ensure_group(&ctx.source, &ctx.staging, "dbus", 81)?;

    println!("  D-Bus configured");
    Ok(())
}
