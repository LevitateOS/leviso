//! Filesystem structure creation.
//!
//! Creates the full FHS directory structure and symlinks for merged /usr.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

// Re-export copy_dir_recursive
pub use crate::common::binary::copy_dir_recursive;

/// Create full FHS directory structure.
pub fn create_fhs_structure(staging: &Path) -> Result<()> {
    println!("Creating FHS directory structure...");

    let dirs = [
        // Essential directories
        "bin",
        "sbin",
        "lib",
        "lib64",
        // /usr hierarchy (merged)
        "usr/bin",
        "usr/sbin",
        "usr/lib",
        "usr/lib64",
        "usr/share",
        "usr/share/man",
        "usr/share/doc",
        "usr/share/licenses",
        "usr/share/zoneinfo",
        "usr/local/bin",
        "usr/local/sbin",
        "usr/local/lib",
        "usr/local/share",
        // /etc configuration
        "etc",
        "etc/systemd/system",
        "etc/pam.d",
        "etc/security",
        "etc/profile.d",
        // XDG Base Directory spec
        "etc/xdg",
        "etc/xdg/autostart",
        // User skeleton with XDG structure
        "etc/skel",
        "etc/skel/.config",
        "etc/skel/.local",
        "etc/skel/.local/share",
        "etc/skel/.local/state",
        "etc/skel/.cache",
        // Volatile directories
        "proc",
        "sys",
        "dev",
        "dev/pts",
        "dev/shm",
        "run",
        "run/lock",
        "tmp",
        // Persistent data
        "var",
        "var/log",
        "var/log/journal",
        "var/tmp",
        "var/cache",
        "var/lib",
        "var/spool",
        // Mount points
        "mnt",
        "media",
        "boot",
        // User directories
        "root",
        "home",
        // Optional
        "opt",
        "srv",
        // Systemd
        "usr/lib/systemd/system",
        "usr/lib/systemd/system-generators",
        "usr/lib64/systemd",
        // Modules
        "usr/lib/modules",
        // PAM
        "usr/lib64/security",
        // D-Bus
        "usr/share/dbus-1/system.d",
        "usr/share/dbus-1/system-services",
        // Locale
        "usr/lib/locale",
    ];

    for dir in dirs {
        fs::create_dir_all(staging.join(dir))
            .with_context(|| format!("Failed to create directory: {}", dir))?;
    }

    println!("  Created {} directories", dirs.len());
    Ok(())
}

/// Create essential symlinks for merged /usr.
pub fn create_symlinks(staging: &Path) -> Result<()> {
    println!("Creating symlinks...");

    // /var/run -> /run
    let var_run = staging.join("var/run");
    if !var_run.exists() && !var_run.is_symlink() {
        std::os::unix::fs::symlink("/run", &var_run)
            .context("Failed to create /var/run symlink")?;
    }

    // /var/lock -> /run/lock
    let var_lock = staging.join("var/lock");
    if !var_lock.exists() && !var_lock.is_symlink() {
        std::os::unix::fs::symlink("/run/lock", &var_lock)
            .context("Failed to create /var/lock symlink")?;
    }

    // /bin -> /usr/bin (merged usr)
    let bin_link = staging.join("bin");
    if bin_link.exists() && !bin_link.is_symlink() {
        fs::remove_dir_all(&bin_link)?;
    }
    if !bin_link.exists() {
        std::os::unix::fs::symlink("usr/bin", &bin_link)
            .context("Failed to create /bin symlink")?;
    }

    // /sbin -> /usr/sbin (merged usr)
    let sbin_link = staging.join("sbin");
    if sbin_link.exists() && !sbin_link.is_symlink() {
        fs::remove_dir_all(&sbin_link)?;
    }
    if !sbin_link.exists() {
        std::os::unix::fs::symlink("usr/sbin", &sbin_link)
            .context("Failed to create /sbin symlink")?;
    }

    // /lib -> /usr/lib (merged usr)
    let lib_link = staging.join("lib");
    if lib_link.exists() && !lib_link.is_symlink() {
        fs::remove_dir_all(&lib_link)?;
    }
    if !lib_link.exists() {
        std::os::unix::fs::symlink("usr/lib", &lib_link)
            .context("Failed to create /lib symlink")?;
    }

    // /lib64 -> /usr/lib64 (merged usr)
    let lib64_link = staging.join("lib64");
    if lib64_link.exists() && !lib64_link.is_symlink() {
        fs::remove_dir_all(&lib64_link)?;
    }
    if !lib64_link.exists() {
        std::os::unix::fs::symlink("usr/lib64", &lib64_link)
            .context("Failed to create /lib64 symlink")?;
    }

    // /usr/bin/sh -> bash
    let sh_link = staging.join("usr/bin/sh");
    if !sh_link.exists() && !sh_link.is_symlink() {
        std::os::unix::fs::symlink("bash", &sh_link)
            .context("Failed to create /usr/bin/sh symlink")?;
    }

    println!("  Created essential symlinks");
    Ok(())
}
