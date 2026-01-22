//! Network stack setup (NetworkManager, wpa_supplicant, WiFi firmware).

use anyhow::{bail, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::common::binary::copy_dir_recursive;

use super::libdeps::{copy_library, get_all_dependencies};
use super::context::BuildContext;

const NETWORKMANAGER_BINARIES: &[(&str, &str)] = &[
    ("NetworkManager", "usr/sbin"),
    ("nmcli", "usr/bin"),
    ("nmtui", "usr/bin"),
    ("nm-online", "usr/bin"),
];

const WPA_BINARIES: &[(&str, &str)] = &[
    ("wpa_supplicant", "usr/sbin"),
    ("wpa_cli", "usr/bin"),
    ("wpa_passphrase", "usr/bin"),
];

const NM_HELPERS: &[&str] = &[
    "nm-dhcp-helper",
    "nm-daemon-helper",
    "nm-dispatcher",
];

const NM_UNITS: &[&str] = &[
    "NetworkManager.service",
    "NetworkManager-dispatcher.service",
];

const WPA_UNITS: &[&str] = &["wpa_supplicant.service"];

const NM_DBUS_POLICIES: &[&str] = &["org.freedesktop.NetworkManager.conf"];

const WIFI_FIRMWARE_DIRS: &[&str] = &[
    "iwlwifi", "ath10k", "ath11k", "rtlwifi", "rtw88", "rtw89", "brcm", "cypress", "mediatek",
];

const WIFI_FIRMWARE_PATTERNS: &[&str] = &["iwlwifi-"];

/// Set up networking stack.
pub fn setup_network(ctx: &BuildContext) -> Result<()> {
    println!("Setting up networking...");

    create_network_directories(ctx)?;
    copy_network_binaries(ctx)?;
    copy_networkmanager_configs(ctx)?;
    copy_networkmanager_plugins(ctx)?;
    copy_dbus_policies(ctx)?;
    copy_network_units(ctx)?;
    enable_networkmanager(ctx)?;
    copy_wifi_firmware(ctx)?;
    ensure_network_users(ctx)?;

    println!("  Networking configured");
    Ok(())
}

fn create_network_directories(ctx: &BuildContext) -> Result<()> {
    let dirs = [
        "etc/NetworkManager",
        "etc/NetworkManager/conf.d",
        "etc/NetworkManager/system-connections",
        "etc/NetworkManager/dispatcher.d",
        "etc/wpa_supplicant",
        "var/lib/NetworkManager",
        "var/run/NetworkManager",
        "usr/lib64/NetworkManager",
        "usr/share/dbus-1/system.d",
        "lib/firmware",
    ];
    for dir in dirs {
        fs::create_dir_all(ctx.staging.join(dir))?;
    }
    Ok(())
}

fn copy_network_binaries(ctx: &BuildContext) -> Result<()> {
    let copy_binary = |name: &str, src_dir: &str| -> Result<bool> {
        let src = ctx.source.join(src_dir).join(name);
        if !src.exists() {
            return Ok(false);
        }

        let dest_dir = if src_dir.contains("sbin") {
            ctx.staging.join("usr/sbin")
        } else {
            ctx.staging.join("usr/bin")
        };
        fs::create_dir_all(&dest_dir)?;

        let dest = dest_dir.join(name);
        if !dest.exists() {
            fs::copy(&src, &dest)?;
            let mut perms = fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest, perms)?;
            println!("  Copied {}", name);
        }

        let libs = get_all_dependencies(&ctx.source, &src, &[])?;
        for lib_name in &libs {
            // Library dependencies are REQUIRED - if the binary needs it, we need it
            copy_library(ctx, lib_name)?;
        }
        Ok(true)
    };

    // NetworkManager and nmcli are REQUIRED for a daily driver OS
    // Users MUST be able to connect to networks
    for (binary, src_dir) in NETWORKMANAGER_BINARIES {
        if !copy_binary(binary, src_dir)?
            && (*binary == "NetworkManager" || *binary == "nmcli")
        {
            // FAIL FAST - these are REQUIRED
            bail!(
                "Required networking binary {} not found.\n\
                 \n\
                 NetworkManager and nmcli are REQUIRED for network connectivity.\n\
                 LevitateOS is a daily driver - users MUST be able to connect to networks.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                binary
            );
        }
    }

    // wpa_supplicant is REQUIRED for WiFi
    for (binary, src_dir) in WPA_BINARIES {
        if !copy_binary(binary, src_dir)? && *binary == "wpa_supplicant" {
            // FAIL FAST - wpa_supplicant is REQUIRED for WiFi
            bail!(
                "wpa_supplicant not found.\n\
                 \n\
                 wpa_supplicant is REQUIRED for WiFi connectivity.\n\
                 LevitateOS is a daily driver - users need WiFi.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST."
            );
        }
    }

    let libexec_src = ctx.source.join("usr/libexec");
    let libexec_dst = ctx.staging.join("usr/libexec");
    fs::create_dir_all(&libexec_dst)?;

    for helper in NM_HELPERS {
        let src = libexec_src.join(helper);
        let dst = libexec_dst.join(helper);
        if src.exists() && !dst.exists() {
            fs::copy(&src, &dst)?;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
            println!("  Copied {}", helper);

            let libs = get_all_dependencies(&ctx.source, &src, &[])?;
            for lib_name in &libs {
                let _ = copy_library(ctx, lib_name);
            }
        }
    }

    Ok(())
}

fn copy_networkmanager_configs(ctx: &BuildContext) -> Result<()> {
    let nm_conf_src = ctx.source.join("etc/NetworkManager");
    let nm_conf_dst = ctx.staging.join("etc/NetworkManager");

    let main_conf_src = nm_conf_src.join("NetworkManager.conf");
    if main_conf_src.exists() {
        fs::copy(&main_conf_src, nm_conf_dst.join("NetworkManager.conf"))?;
        println!("  Copied NetworkManager.conf");
    } else {
        let minimal_config = r#"[main]
plugins=keyfile

[keyfile]
unmanaged-devices=none

[logging]
level=INFO
"#;
        fs::write(nm_conf_dst.join("NetworkManager.conf"), minimal_config)?;
        println!("  Created minimal NetworkManager.conf");
    }

    let conf_d_src = nm_conf_src.join("conf.d");
    let conf_d_dst = nm_conf_dst.join("conf.d");
    if conf_d_src.is_dir() {
        fs::create_dir_all(&conf_d_dst)?;
        for entry in fs::read_dir(&conf_d_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), conf_d_dst.join(entry.file_name()))?;
            }
        }
    }

    let manage_all_config = r#"# LevitateOS: Manage all devices by default
[main]
no-auto-default=

[device]
managed=true
"#;
    fs::write(conf_d_dst.join("99-leviso-manage-ethernet.conf"), manage_all_config)?;
    println!("  Added 99-leviso-manage-ethernet.conf");

    Ok(())
}

fn copy_networkmanager_plugins(ctx: &BuildContext) -> Result<()> {
    let plugin_src = ctx.source.join("usr/lib64/NetworkManager");
    let plugin_dst = ctx.staging.join("usr/lib64/NetworkManager");

    if !plugin_src.is_dir() {
        // FAIL FAST - plugins are REQUIRED for NetworkManager to work
        bail!(
            "NetworkManager plugins directory not found at {}.\n\
             \n\
             NetworkManager plugins are REQUIRED for network connectivity.\n\
             Without plugins, NetworkManager cannot manage network connections.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            plugin_src.display()
        );
    }

    let mut count = 0;
    for entry in fs::read_dir(&plugin_src)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "so").unwrap_or(false) {
            let filename = path.file_name().unwrap();
            let dest = plugin_dst.join(filename);
            if !dest.exists() {
                fs::copy(&path, &dest)?;
                count += 1;
                let libs = get_all_dependencies(&ctx.source, &path, &[])?;
                for lib_name in &libs {
                    let _ = copy_library(ctx, lib_name);
                }
            }
        }
    }

    if count > 0 {
        println!("  Copied {} NetworkManager plugins", count);
    }

    Ok(())
}

fn copy_dbus_policies(ctx: &BuildContext) -> Result<()> {
    let dbus_src = ctx.source.join("usr/share/dbus-1/system.d");
    let dbus_dst = ctx.staging.join("usr/share/dbus-1/system.d");

    for policy in NM_DBUS_POLICIES {
        let src = dbus_src.join(policy);
        let dst = dbus_dst.join(policy);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied D-Bus policy: {}", policy);
        }
    }

    let wpa_policy_src = dbus_src.join("wpa_supplicant.conf");
    if wpa_policy_src.exists() {
        fs::copy(&wpa_policy_src, dbus_dst.join("wpa_supplicant.conf"))?;
    }

    let wpa_policy2_src = dbus_src.join("fi.w1.wpa_supplicant1.conf");
    if wpa_policy2_src.exists() {
        fs::copy(&wpa_policy2_src, dbus_dst.join("fi.w1.wpa_supplicant1.conf"))?;
    }

    Ok(())
}

fn copy_network_units(ctx: &BuildContext) -> Result<()> {
    let unit_src = ctx.source.join("usr/lib/systemd/system");
    let unit_dst = ctx.staging.join("usr/lib/systemd/system");

    for unit in NM_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied {}", unit);
        }
    }

    for unit in WPA_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied {}", unit);
        }
    }

    Ok(())
}

fn enable_networkmanager(ctx: &BuildContext) -> Result<()> {
    let multi_user_wants = ctx.staging.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    let nm_link = multi_user_wants.join("NetworkManager.service");
    if !nm_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/NetworkManager.service",
            &nm_link,
        )?;
        println!("  Enabled NetworkManager.service");
    }
    Ok(())
}

fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.source.join("lib/firmware");
    let alt_firmware_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("lib/firmware");

    let actual_src = if firmware_src.is_dir() {
        &firmware_src
    } else if alt_firmware_src.is_dir() {
        &alt_firmware_src
    } else {
        // FAIL FAST - firmware is REQUIRED for WiFi hardware
        bail!(
            "No firmware directory found.\n\
             \n\
             Checked:\n\
             - {}\n\
             - {}\n\
             \n\
             WiFi firmware is REQUIRED - without it, WiFi won't work.\n\
             LevitateOS is a daily driver for real hardware.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            firmware_src.display(),
            alt_firmware_src.display()
        );
    };

    fs::create_dir_all(&firmware_dst)?;

    let mut total_size: u64 = 0;

    for dir_name in WIFI_FIRMWARE_DIRS {
        let src_dir = actual_src.join(dir_name);
        if src_dir.is_dir() {
            let dst_dir = firmware_dst.join(dir_name);
            let size = copy_dir_recursive(&src_dir, &dst_dir)?;
            if size > 0 {
                println!("  Copied {} firmware ({:.1} MB)", dir_name, size as f64 / 1_000_000.0);
                total_size += size;
            }
        }
    }

    if let Ok(entries) = fs::read_dir(actual_src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = path.file_name().unwrap().to_string_lossy();
                for pattern in WIFI_FIRMWARE_PATTERNS {
                    if filename.starts_with(pattern) {
                        let dst = firmware_dst.join(&*filename);
                        if !dst.exists() {
                            fs::copy(&path, &dst)?;
                            if let Ok(meta) = fs::metadata(&dst) {
                                total_size += meta.len();
                            }
                        }
                    }
                }
            }
        }
    }

    println!("  Total firmware: {:.1} MB", total_size as f64 / 1_000_000.0);
    Ok(())
}

fn ensure_network_users(ctx: &BuildContext) -> Result<()> {
    super::users::ensure_user(
        &ctx.source,
        &ctx.staging,
        "nm-openconnect",
        993,
        988,
        "/",
        "/sbin/nologin",
    )?;
    super::users::ensure_group(&ctx.source, &ctx.staging, "nm-openconnect", 988)?;
    Ok(())
}

