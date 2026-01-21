//! Network stack setup (NetworkManager, wpa_supplicant, WiFi firmware).

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::binary::{copy_library, get_all_dependencies};
use super::context::BuildContext;

/// NetworkManager binaries to copy.
const NETWORKMANAGER_BINARIES: &[(&str, &str)] = &[
    ("NetworkManager", "usr/sbin"),     // Main daemon
    ("nmcli", "usr/bin"),               // CLI tool
    ("nmtui", "usr/bin"),               // TUI tool (optional, nice to have)
    ("nm-online", "usr/bin"),           // Check connectivity
];

/// wpa_supplicant binaries to copy.
const WPA_BINARIES: &[(&str, &str)] = &[
    ("wpa_supplicant", "usr/sbin"),     // WiFi authentication daemon
    ("wpa_cli", "usr/bin"),             // WiFi CLI control
    ("wpa_passphrase", "usr/bin"),      // PSK generation
];

/// iproute2 binaries to copy.
const IPROUTE_BINARIES: &[(&str, &str)] = &[
    ("ip", "usr/sbin"),                 // Network configuration
];

/// NetworkManager systemd units.
const NM_UNITS: &[&str] = &[
    "NetworkManager.service",
    "NetworkManager-dispatcher.service",
    // Note: NOT enabling NetworkManager-wait-online.service - it delays boot
];

/// wpa_supplicant systemd units.
const WPA_UNITS: &[&str] = &[
    "wpa_supplicant.service",
];

/// D-Bus policy files for NetworkManager.
const NM_DBUS_POLICIES: &[&str] = &[
    "org.freedesktop.NetworkManager.conf",
];

/// WiFi firmware directories to copy (for common chipsets).
/// These are the most common WiFi chipsets in laptops/desktops.
const WIFI_FIRMWARE_DIRS: &[&str] = &[
    "iwlwifi",          // Intel WiFi (most common in laptops)
    "ath10k",           // Qualcomm Atheros
    "ath11k",           // Newer Qualcomm Atheros
    "rtlwifi",          // Realtek legacy
    "rtw88",            // Realtek newer
    "rtw89",            // Realtek newest
    "brcm",             // Broadcom
    "cypress",          // Cypress (Broadcom symlink targets)
    "mediatek",         // MediaTek
];

/// WiFi firmware file patterns to copy (for files in /lib/firmware root).
const WIFI_FIRMWARE_PATTERNS: &[&str] = &[
    "iwlwifi-",         // Intel WiFi firmware files
];

/// Set up networking stack.
pub fn setup_network(ctx: &BuildContext) -> Result<()> {
    println!("Setting up networking...");

    // Create network directories
    create_network_directories(ctx)?;

    // Copy network binaries
    copy_network_binaries(ctx)?;

    // Copy NetworkManager configs and plugins
    copy_networkmanager_configs(ctx)?;
    copy_networkmanager_plugins(ctx)?;

    // Copy D-Bus policies for NetworkManager
    copy_dbus_policies(ctx)?;

    // Copy systemd units
    copy_network_units(ctx)?;

    // Enable NetworkManager service
    enable_networkmanager(ctx)?;

    // Copy WiFi firmware
    copy_wifi_firmware(ctx)?;

    // Ensure nm-openconnect user exists (used by some NM plugins)
    ensure_network_users(ctx)?;

    println!("  Networking configured");

    Ok(())
}

/// Create network-related directories.
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
        fs::create_dir_all(ctx.initramfs.join(dir))?;
    }

    Ok(())
}

/// Copy network binaries and their library dependencies.
fn copy_network_binaries(ctx: &BuildContext) -> Result<()> {
    // Helper to copy a binary from a specific source directory
    let copy_binary = |name: &str, src_dir: &str| -> Result<bool> {
        let src = ctx.rootfs.join(src_dir).join(name);
        if !src.exists() {
            return Ok(false);
        }

        // Determine destination directory
        let dest_dir = if src_dir.contains("sbin") {
            ctx.initramfs.join("usr/sbin")
        } else {
            ctx.initramfs.join("usr/bin")
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

        // Copy library dependencies
        let libs = get_all_dependencies(&ctx.rootfs, &src)?;
        for lib_name in &libs {
            if let Err(e) = copy_library(&ctx.rootfs, lib_name, &ctx.initramfs) {
                println!("    Warning: Failed to copy library {}: {}", lib_name, e);
            }
        }

        Ok(true)
    };

    // Copy NetworkManager binaries
    for (binary, src_dir) in NETWORKMANAGER_BINARIES {
        if !copy_binary(binary, src_dir)? {
            if *binary == "NetworkManager" || *binary == "nmcli" {
                println!("  Warning: Required binary {} not found", binary);
            }
        }
    }

    // Copy wpa_supplicant binaries
    for (binary, src_dir) in WPA_BINARIES {
        if !copy_binary(binary, src_dir)? {
            if *binary == "wpa_supplicant" {
                println!("  Warning: wpa_supplicant not found (WiFi won't work)");
            }
        }
    }

    // Copy iproute2 binaries
    for (binary, src_dir) in IPROUTE_BINARIES {
        copy_binary(binary, src_dir)?;
    }

    Ok(())
}

/// Copy NetworkManager configuration files.
fn copy_networkmanager_configs(ctx: &BuildContext) -> Result<()> {
    let nm_conf_src = ctx.rootfs.join("etc/NetworkManager");
    let nm_conf_dst = ctx.initramfs.join("etc/NetworkManager");

    // Copy main NetworkManager.conf
    let main_conf_src = nm_conf_src.join("NetworkManager.conf");
    if main_conf_src.exists() {
        fs::copy(&main_conf_src, nm_conf_dst.join("NetworkManager.conf"))?;
        println!("  Copied NetworkManager.conf");
    } else {
        // Create minimal config if not found
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

    // Copy conf.d files
    let conf_d_src = nm_conf_src.join("conf.d");
    let conf_d_dst = nm_conf_dst.join("conf.d");
    if conf_d_src.is_dir() {
        copy_dir_contents(&conf_d_src, &conf_d_dst)?;
    }

    Ok(())
}

/// Copy NetworkManager plugins.
fn copy_networkmanager_plugins(ctx: &BuildContext) -> Result<()> {
    let plugin_src = ctx.rootfs.join("usr/lib64/NetworkManager");
    let plugin_dst = ctx.initramfs.join("usr/lib64/NetworkManager");

    if !plugin_src.is_dir() {
        println!("  Warning: NetworkManager plugins directory not found");
        return Ok(());
    }

    // Copy all .so files (plugins)
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

                // Copy plugin's library dependencies
                let libs = get_all_dependencies(&ctx.rootfs, &path)?;
                for lib_name in &libs {
                    let _ = copy_library(&ctx.rootfs, lib_name, &ctx.initramfs);
                }
            }
        }
    }

    if count > 0 {
        println!("  Copied {} NetworkManager plugins", count);
    }

    Ok(())
}

/// Copy D-Bus policies for NetworkManager.
fn copy_dbus_policies(ctx: &BuildContext) -> Result<()> {
    let dbus_src = ctx.rootfs.join("usr/share/dbus-1/system.d");
    let dbus_dst = ctx.initramfs.join("usr/share/dbus-1/system.d");

    for policy in NM_DBUS_POLICIES {
        let src = dbus_src.join(policy);
        let dst = dbus_dst.join(policy);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied D-Bus policy: {}", policy);
        }
    }

    // Also copy wpa_supplicant D-Bus policy if it exists
    let wpa_policy_src = dbus_src.join("wpa_supplicant.conf");
    if wpa_policy_src.exists() {
        fs::copy(&wpa_policy_src, dbus_dst.join("wpa_supplicant.conf"))?;
        println!("  Copied D-Bus policy: wpa_supplicant.conf");
    }

    // Also try fi.w1.wpa_supplicant1.conf
    let wpa_policy2_src = dbus_src.join("fi.w1.wpa_supplicant1.conf");
    if wpa_policy2_src.exists() {
        fs::copy(&wpa_policy2_src, dbus_dst.join("fi.w1.wpa_supplicant1.conf"))?;
        println!("  Copied D-Bus policy: fi.w1.wpa_supplicant1.conf");
    }

    Ok(())
}

/// Copy network-related systemd units.
fn copy_network_units(ctx: &BuildContext) -> Result<()> {
    let unit_src = ctx.rootfs.join("usr/lib/systemd/system");
    let unit_dst = ctx.initramfs.join("usr/lib/systemd/system");

    // Copy NetworkManager units
    for unit in NM_UNITS {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
            println!("  Copied {}", unit);
        }
    }

    // Copy wpa_supplicant units
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

/// Enable NetworkManager service in multi-user.target.
fn enable_networkmanager(ctx: &BuildContext) -> Result<()> {
    let multi_user_wants = ctx
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;

    // Enable NetworkManager.service
    let nm_link = multi_user_wants.join("NetworkManager.service");
    if !nm_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/NetworkManager.service",
            &nm_link,
        )?;
        println!("  Enabled NetworkManager.service");
    }

    // Note: wpa_supplicant is NOT enabled as a standalone service.
    // NetworkManager manages wpa_supplicant directly via D-Bus when WiFi is used.
    // Enabling it standalone causes failures due to missing environment variables.

    Ok(())
}

/// Copy WiFi firmware for common chipsets.
fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.rootfs.join("lib/firmware");
    let firmware_dst = ctx.initramfs.join("lib/firmware");

    // Also check /usr/lib/firmware (some distros put it there)
    let alt_firmware_src = ctx.rootfs.join("usr/lib/firmware");
    let actual_src = if firmware_src.is_dir() {
        &firmware_src
    } else if alt_firmware_src.is_dir() {
        &alt_firmware_src
    } else {
        println!("  Warning: No firmware directory found");
        return Ok(());
    };

    fs::create_dir_all(&firmware_dst)?;

    let mut total_size: u64 = 0;

    // Copy firmware directories
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

    // Copy firmware files matching patterns (e.g., iwlwifi-* in root)
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

/// Ensure network-related system users exist.
fn ensure_network_users(ctx: &BuildContext) -> Result<()> {
    // nm-openconnect user (used by NetworkManager openconnect plugin)
    // This may not be strictly necessary, but prevents warnings
    super::users::ensure_user(
        &ctx.rootfs,
        &ctx.initramfs,
        "nm-openconnect",
        993,
        988,
        "/",
        "/sbin/nologin",
    )?;
    super::users::ensure_group(&ctx.rootfs, &ctx.initramfs, "nm-openconnect", 988)?;

    Ok(())
}

/// Copy directory contents (non-recursive, files only).
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = path.file_name().unwrap();
                fs::copy(&path, dst.join(filename))?;
            }
        }
    }

    Ok(())
}

/// Copy directory recursively and return total size in bytes.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64> {
    let mut total_size: u64 = 0;

    if !src.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().unwrap();
        let dest_path = dst.join(filename);

        if path.is_dir() {
            total_size += copy_dir_recursive(&path, &dest_path)?;
        } else if path.is_symlink() {
            // Copy symlinks
            let target = fs::read_link(&path)?;
            if !dest_path.exists() {
                std::os::unix::fs::symlink(&target, &dest_path)?;
            }
        } else {
            fs::copy(&path, &dest_path)?;
            if let Ok(meta) = fs::metadata(&dest_path) {
                total_size += meta.len();
            }
        }
    }

    Ok(total_size)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_constants_not_empty() {
        use super::*;
        assert!(!NETWORKMANAGER_BINARIES.is_empty());
        assert!(!WPA_BINARIES.is_empty());
        assert!(!NM_UNITS.is_empty());
        assert!(!WIFI_FIRMWARE_DIRS.is_empty());
    }
}
