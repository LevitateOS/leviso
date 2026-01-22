//! Network stack setup (NetworkManager, wpa_supplicant, WiFi firmware).

use anyhow::{bail, Result};
use std::fs;

use crate::common::binary::copy_dir_recursive;

use super::context::BuildContext;
use super::libdeps::{
    copy_binary_with_libs, copy_dir_tree, copy_file, copy_sbin_binary_with_libs, copy_systemd_units,
};

const NM_BINARIES: &[&str] = &["nmcli", "nmtui", "nm-online"];
const NM_SBIN: &[&str] = &["NetworkManager"];
const WPA_SBIN: &[&str] = &["wpa_supplicant"];
const WPA_BIN: &[&str] = &["wpa_cli", "wpa_passphrase"];

const NM_UNITS: &[&str] = &["NetworkManager.service", "NetworkManager-dispatcher.service"];
const WPA_UNITS: &[&str] = &["wpa_supplicant.service"];

const WIFI_FIRMWARE_DIRS: &[&str] = &[
    "iwlwifi", "ath10k", "ath11k", "rtlwifi", "rtw88", "rtw89", "brcm", "cypress", "mediatek",
];

/// Set up networking stack.
pub fn setup_network(ctx: &BuildContext) -> Result<()> {
    println!("Setting up networking...");

    // NetworkManager (required)
    for bin in NM_SBIN {
        if !copy_sbin_binary_with_libs(ctx, bin)? {
            bail!("{} not found - NetworkManager is required", bin);
        }
    }
    for bin in NM_BINARIES {
        if !copy_binary_with_libs(ctx, bin, "usr/bin")? {
            bail!("{} not found - users need nmcli/nmtui to configure networking", bin);
        }
    }

    // wpa_supplicant (required for WiFi)
    for bin in WPA_SBIN {
        if !copy_sbin_binary_with_libs(ctx, bin)? {
            bail!("{} not found - wpa_supplicant is required for WiFi", bin);
        }
    }
    for bin in WPA_BIN {
        if !copy_binary_with_libs(ctx, bin, "usr/bin")? {
            bail!("{} not found - required for WiFi configuration", bin);
        }
    }

    // NetworkManager helpers and plugins
    copy_dir_tree(ctx, "usr/libexec")?; // nm-dhcp-helper, nm-dispatcher, etc.
    copy_dir_tree(ctx, "usr/lib64/NetworkManager")?;

    // Configs
    copy_dir_tree(ctx, "etc/NetworkManager")?;
    copy_dir_tree(ctx, "etc/wpa_supplicant")?;

    // Add our config to manage all devices
    let conf_d = ctx.staging.join("etc/NetworkManager/conf.d");
    fs::create_dir_all(&conf_d)?;
    fs::write(
        conf_d.join("99-leviso-manage-ethernet.conf"),
        "[main]\nno-auto-default=\n\n[device]\nmanaged=true\n",
    )?;

    // D-Bus policies (required - NM won't start without them)
    if !copy_file(ctx, "usr/share/dbus-1/system.d/org.freedesktop.NetworkManager.conf")? {
        bail!("NetworkManager D-Bus policy not found - networking will fail");
    }
    if !copy_file(ctx, "usr/share/dbus-1/system.d/wpa_supplicant.conf")? {
        bail!("wpa_supplicant D-Bus policy not found - WiFi will fail");
    }
    // fi.w1 is the newer interface name, try to copy but don't fail
    let _ = copy_file(ctx, "usr/share/dbus-1/system.d/fi.w1.wpa_supplicant1.conf");

    // Systemd units
    copy_systemd_units(ctx, NM_UNITS)?;
    copy_systemd_units(ctx, WPA_UNITS)?;

    // Enable NetworkManager
    let multi_user_wants = ctx.staging.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;
    let nm_link = multi_user_wants.join("NetworkManager.service");
    if !nm_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/NetworkManager.service", &nm_link)?;
    }

    // WiFi firmware
    copy_wifi_firmware(ctx)?;

    // nm-openconnect user (optional VPN support)
    let _ = super::users::ensure_user(
        &ctx.source, &ctx.staging, "nm-openconnect", 993, 988, "/", "/sbin/nologin",
    );
    let _ = super::users::ensure_group(&ctx.source, &ctx.staging, "nm-openconnect", 988);

    println!("  Networking configured");
    Ok(())
}

fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.source.join("lib/firmware");
    let alt_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("lib/firmware");

    let actual_src = if firmware_src.is_dir() {
        &firmware_src
    } else if alt_src.is_dir() {
        &alt_src
    } else {
        bail!("No firmware directory found - WiFi won't work");
    };

    fs::create_dir_all(&firmware_dst)?;

    let mut total: u64 = 0;
    for dir_name in WIFI_FIRMWARE_DIRS {
        let src_dir = actual_src.join(dir_name);
        if src_dir.is_dir() {
            let dst_dir = firmware_dst.join(dir_name);
            let size = copy_dir_recursive(&src_dir, &dst_dir)?;
            if size > 0 {
                total += size;
            }
        }
    }

    // Also copy iwlwifi-* files in root firmware dir
    if let Ok(entries) = fs::read_dir(actual_src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.starts_with("iwlwifi-") {
                    let dst = firmware_dst.join(&*name);
                    if !dst.exists() {
                        fs::copy(&path, &dst)?;
                        total += fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
    }

    println!("  WiFi firmware: {:.1} MB", total as f64 / 1_000_000.0);
    Ok(())
}
