//! Firmware operations - WiFi, microcode.

use anyhow::{bail, Result};
use std::fs;

use leviso_elf::copy_dir_recursive;

use crate::build::context::BuildContext;

/// WiFi firmware directories to copy.
const WIFI_FIRMWARE_DIRS: &[&str] = &[
    "iwlwifi", "ath10k", "ath11k", "rtlwifi", "rtw88", "rtw89", "brcm", "cypress", "mediatek",
];

/// Copy essential WiFi firmware (subset for smaller images).
pub fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
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
                        match fs::metadata(&dst) {
                            Ok(m) => total += m.len(),
                            Err(e) => {
                                eprintln!(
                                    "  [WARN] Failed to get size of {}: {}",
                                    dst.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    println!("  WiFi firmware: {:.1} MB", total as f64 / 1_000_000.0);
    Ok(())
}

/// Copy all firmware including microcode (for full ISO).
pub fn copy_all_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("usr/lib/firmware");

    let alt_src = ctx.source.join("lib/firmware");
    let actual_src = if firmware_src.exists() {
        &firmware_src
    } else if alt_src.exists() {
        &alt_src
    } else {
        bail!(
            "No firmware directory found.\n\
             Firmware is REQUIRED - LevitateOS is a daily driver for real hardware."
        );
    };

    fs::create_dir_all(&firmware_dst)?;

    let size = copy_dir_recursive(actual_src, &firmware_dst)?;
    println!(
        "  Copied all firmware ({:.1} MB)",
        size as f64 / 1_000_000.0
    );

    // Copy Intel microcode from Rocky's non-standard location
    let intel_ucode_dst = firmware_dst.join("intel-ucode");
    let microcode_ctl_src = ctx
        .source
        .join("usr/share/microcode_ctl/ucode_with_caveats/intel/intel-ucode");
    if microcode_ctl_src.exists() && microcode_ctl_src.is_dir() {
        fs::create_dir_all(&intel_ucode_dst)?;
        let intel_size = copy_dir_recursive(&microcode_ctl_src, &intel_ucode_dst)?;
        println!(
            "  Copied Intel microcode from microcode_ctl ({:.1} KB)",
            intel_size as f64 / 1_000.0
        );
    }

    // Validate microcode directories exist (P0 critical for CPU security)
    let amd_ucode = firmware_dst.join("amd-ucode");
    let intel_ucode = firmware_dst.join("intel-ucode");

    if !amd_ucode.exists() {
        bail!(
            "AMD microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.",
            amd_ucode.display()
        );
    }
    let amd_count = fs::read_dir(&amd_ucode)?.filter(|e| e.is_ok()).count();
    if amd_count == 0 {
        bail!(
            "AMD microcode directory is empty at {}",
            amd_ucode.display()
        );
    }
    println!("  AMD microcode: {} files", amd_count);

    if !intel_ucode.exists() {
        bail!(
            "Intel microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.",
            intel_ucode.display()
        );
    }
    let intel_count = fs::read_dir(&intel_ucode)?.filter(|e| e.is_ok()).count();
    if intel_count == 0 {
        bail!(
            "Intel microcode directory is empty at {}",
            intel_ucode.display()
        );
    }
    println!("  Intel microcode: {} files", intel_count);

    Ok(())
}
