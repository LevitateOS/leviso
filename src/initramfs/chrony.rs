//! Chrony NTP daemon setup.

use anyhow::Result;
use std::fs;

use super::context::BuildContext;

/// Set up Chrony NTP daemon.
pub fn setup_chrony(ctx: &BuildContext) -> Result<()> {
    println!("Setting up Chrony NTP...");

    // Create chrony directories
    fs::create_dir_all(ctx.initramfs.join("var/lib/chrony"))?;
    fs::create_dir_all(ctx.initramfs.join("var/run/chrony"))?;

    // Copy chrony config
    copy_chrony_config(ctx)?;

    // Create /usr/sbin symlink for chronyd
    setup_chronyd_symlink(ctx)?;

    // Set up ntp-units.d for timedatectl
    setup_ntp_units(ctx)?;

    // Enable chronyd service
    enable_chronyd_service(ctx)?;

    println!("  Chrony configured");

    Ok(())
}

/// Copy chrony configuration files.
fn copy_chrony_config(ctx: &BuildContext) -> Result<()> {
    // Copy main chrony.conf
    let chrony_conf_src = ctx.rootfs.join("etc/chrony.conf");
    let chrony_conf_dst = ctx.initramfs.join("etc/chrony.conf");
    if chrony_conf_src.exists() {
        fs::copy(&chrony_conf_src, &chrony_conf_dst)?;
    }

    // Copy chrony sysconfig (contains OPTIONS for chronyd)
    let sysconfig_dir = ctx.initramfs.join("etc/sysconfig");
    fs::create_dir_all(&sysconfig_dir)?;
    let chrony_sysconfig_src = ctx.rootfs.join("etc/sysconfig/chronyd");
    let chrony_sysconfig_dst = sysconfig_dir.join("chronyd");
    if chrony_sysconfig_src.exists() {
        fs::copy(&chrony_sysconfig_src, &chrony_sysconfig_dst)?;
    }

    Ok(())
}

/// Create /usr/sbin/chronyd symlink (service expects it there).
fn setup_chronyd_symlink(ctx: &BuildContext) -> Result<()> {
    let usr_sbin = ctx.initramfs.join("usr/sbin");
    fs::create_dir_all(&usr_sbin)?;

    let chronyd_sbin_link = usr_sbin.join("chronyd");
    if !chronyd_sbin_link.exists() && ctx.initramfs.join("bin/chronyd").exists() {
        std::os::unix::fs::symlink("/bin/chronyd", &chronyd_sbin_link)?;
    }

    Ok(())
}

/// Set up ntp-units.d for timedatectl set-ntp support.
fn setup_ntp_units(ctx: &BuildContext) -> Result<()> {
    let ntp_units_dir = ctx.initramfs.join("usr/lib/systemd/ntp-units.d");
    fs::create_dir_all(&ntp_units_dir)?;

    let ntp_units_src = ctx.rootfs.join("usr/lib/systemd/ntp-units.d/50-chronyd.list");
    if ntp_units_src.exists() {
        fs::copy(&ntp_units_src, ntp_units_dir.join("50-chronyd.list"))?;
    }

    Ok(())
}

/// Enable chronyd.service in multi-user.target.
fn enable_chronyd_service(ctx: &BuildContext) -> Result<()> {
    let multi_user_wants = ctx
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let chronyd_link = multi_user_wants.join("chronyd.service");
    if !chronyd_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/chronyd.service", &chronyd_link)?;
    }

    Ok(())
}

/// Ensure chrony user and group exist.
pub fn ensure_chrony_user(ctx: &BuildContext) -> Result<()> {
    super::users::ensure_user(
        &ctx.rootfs,
        &ctx.initramfs,
        "chrony",
        992,
        987,
        "/var/lib/chrony",
        "/sbin/nologin",
    )?;
    super::users::ensure_group(&ctx.rootfs, &ctx.initramfs, "chrony", 987)?;
    Ok(())
}
