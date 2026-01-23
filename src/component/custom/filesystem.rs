//! Filesystem operations - FHS symlinks, merged /usr.

use anyhow::{Context, Result};
use std::fs;

use crate::build::context::BuildContext;

/// Create essential FHS symlinks for merged /usr layout.
pub fn create_fhs_symlinks(ctx: &BuildContext) -> Result<()> {
    println!("Creating symlinks...");

    // /var/run -> /run
    let var_run = ctx.staging.join("var/run");
    if !var_run.exists() && !var_run.is_symlink() {
        std::os::unix::fs::symlink("/run", &var_run)
            .context("Failed to create /var/run symlink")?;
    }

    // /var/lock -> /run/lock
    let var_lock = ctx.staging.join("var/lock");
    if !var_lock.exists() && !var_lock.is_symlink() {
        std::os::unix::fs::symlink("/run/lock", &var_lock)
            .context("Failed to create /var/lock symlink")?;
    }

    // /bin -> /usr/bin (merged usr)
    let bin_link = ctx.staging.join("bin");
    if bin_link.exists() && !bin_link.is_symlink() {
        fs::remove_dir_all(&bin_link)?;
    }
    if !bin_link.exists() {
        std::os::unix::fs::symlink("usr/bin", &bin_link)
            .context("Failed to create /bin symlink")?;
    }

    // /sbin -> /usr/sbin (merged usr)
    let sbin_link = ctx.staging.join("sbin");
    if sbin_link.exists() && !sbin_link.is_symlink() {
        fs::remove_dir_all(&sbin_link)?;
    }
    if !sbin_link.exists() {
        std::os::unix::fs::symlink("usr/sbin", &sbin_link)
            .context("Failed to create /sbin symlink")?;
    }

    // /lib -> /usr/lib (merged usr)
    let lib_link = ctx.staging.join("lib");
    if lib_link.exists() && !lib_link.is_symlink() {
        fs::remove_dir_all(&lib_link)?;
    }
    if !lib_link.exists() {
        std::os::unix::fs::symlink("usr/lib", &lib_link)
            .context("Failed to create /lib symlink")?;
    }

    // /lib64 -> /usr/lib64 (merged usr)
    let lib64_link = ctx.staging.join("lib64");
    if lib64_link.exists() && !lib64_link.is_symlink() {
        fs::remove_dir_all(&lib64_link)?;
    }
    if !lib64_link.exists() {
        std::os::unix::fs::symlink("usr/lib64", &lib64_link)
            .context("Failed to create /lib64 symlink")?;
    }

    // /usr/bin/sh -> bash
    let sh_link = ctx.staging.join("usr/bin/sh");
    if !sh_link.exists() && !sh_link.is_symlink() {
        std::os::unix::fs::symlink("bash", &sh_link)
            .context("Failed to create /usr/bin/sh symlink")?;
    }

    println!("  Created essential symlinks");
    Ok(())
}
