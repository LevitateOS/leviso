//! Chrony NTP daemon setup.

use anyhow::Result;
use std::fs;

use super::context::BuildContext;

/// Set up Chrony NTP daemon.
pub fn setup_chrony(ctx: &BuildContext) -> Result<()> {
    println!("Setting up Chrony NTP...");

    fs::create_dir_all(ctx.staging.join("var/lib/chrony"))?;
    fs::create_dir_all(ctx.staging.join("var/run/chrony"))?;

    // Copy chrony config
    let chrony_conf_src = ctx.source.join("etc/chrony.conf");
    let chrony_conf_dst = ctx.staging.join("etc/chrony.conf");
    if chrony_conf_src.exists() {
        fs::copy(&chrony_conf_src, &chrony_conf_dst)?;
    }

    // Copy chrony sysconfig
    let sysconfig_dir = ctx.staging.join("etc/sysconfig");
    fs::create_dir_all(&sysconfig_dir)?;
    let chrony_sysconfig_src = ctx.source.join("etc/sysconfig/chronyd");
    if chrony_sysconfig_src.exists() {
        fs::copy(&chrony_sysconfig_src, sysconfig_dir.join("chronyd"))?;
    }

    // Set up ntp-units.d for timedatectl
    let ntp_units_dir = ctx.staging.join("usr/lib/systemd/ntp-units.d");
    fs::create_dir_all(&ntp_units_dir)?;
    let ntp_units_src = ctx.source.join("usr/lib/systemd/ntp-units.d/50-chronyd.list");
    if ntp_units_src.exists() {
        fs::copy(&ntp_units_src, ntp_units_dir.join("50-chronyd.list"))?;
    }

    // Enable chronyd service
    let multi_user_wants = ctx.staging.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;
    let chronyd_link = multi_user_wants.join("chronyd.service");
    if !chronyd_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/chronyd.service", &chronyd_link)?;
    }

    println!("  Chrony configured");
    Ok(())
}
