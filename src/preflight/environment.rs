//! Build environment checks (directories, disk space, configs).

use std::path::Path;

use crate::process::Cmd;

use super::types::CheckResult;
use super::validators::{validate_init_script, validate_kconfig};

/// Check build environment (directories, permissions, etc.).
pub fn check_build_environment(base_dir: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Check output directory is writable
    let output_dir = base_dir.join("output");
    if output_dir.exists() {
        // Check if we can write
        let test_file = output_dir.join(".preflight-test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                results.push(CheckResult::pass("output/ writable"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "output/ writable",
                    &format!("Cannot write to output/: {}", e),
                ));
            }
        }
    } else {
        // Try to create it
        match std::fs::create_dir_all(&output_dir) {
            Ok(_) => {
                results.push(CheckResult::pass("output/ writable"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "output/ writable",
                    &format!("Cannot create output/: {}", e),
                ));
            }
        }
    }

    // Check downloads directory
    let downloads_dir = base_dir.join("downloads");
    if downloads_dir.exists() {
        results.push(CheckResult::pass("downloads/ exists"));
    } else {
        match std::fs::create_dir_all(&downloads_dir) {
            Ok(_) => {
                results.push(CheckResult::pass("downloads/ created"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "downloads/",
                    &format!("Cannot create: {}", e),
                ));
            }
        }
    }

    // Check kconfig exists AND is valid - ANTI-CHEAT: verify content, not just existence
    // An empty file or corrupted config passes .exists() but builds wrong kernel.
    let kconfig = base_dir.join("kconfig");
    if kconfig.exists() {
        match validate_kconfig(&kconfig) {
            Ok(config_count) => {
                results.push(CheckResult::pass_with(
                    "kconfig",
                    &format!("{} CONFIG_ options", config_count),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail("kconfig", &e));
            }
        }
    } else {
        results.push(CheckResult::fail(
            "kconfig",
            "Not found - kernel configuration required",
        ));
    }

    // Check profile/init_tiny exists AND is valid - ANTI-CHEAT: verify it's a real init script
    // An empty file passes .exists() but system won't boot.
    let init_tiny = base_dir.join("profile/init_tiny");
    if init_tiny.exists() {
        match validate_init_script(&init_tiny) {
            Ok(line_count) => {
                results.push(CheckResult::pass_with(
                    "profile/init_tiny",
                    &format!("{} lines, valid shebang", line_count),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail("profile/init_tiny", &e));
            }
        }
    } else {
        results.push(CheckResult::fail(
            "profile/init_tiny",
            "Not found - initramfs init script required",
        ));
    }

    // Check disk space (warn if < 20GB free)
    // Use df command to avoid nix crate dependency
    if let Ok(result) = Cmd::new("df")
        .args(["--output=avail", "-B1"])
        .arg(base_dir.to_string_lossy().as_ref())
        .allow_fail()
        .run()
    {
        if result.success() {
            // Skip header line, get available bytes
            if let Some(avail_str) = result.stdout.lines().nth(1) {
                if let Ok(avail_bytes) = avail_str.trim().parse::<u64>() {
                    let free_gb = avail_bytes / (1024 * 1024 * 1024);
                    if free_gb < 20 {
                        results.push(CheckResult::warn(
                            "disk space",
                            &format!("{}GB free - build needs ~15GB", free_gb),
                        ));
                    } else {
                        results.push(CheckResult::pass_with(
                            "disk space",
                            &format!("{}GB free", free_gb),
                        ));
                    }
                }
            }
        }
    }

    results
}
