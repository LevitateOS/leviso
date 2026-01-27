//! Rootfs builder - creates the complete LevitateOS system image.
//!
//! The rootfs (EROFS) serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (extracted to disk by recstrap)
//!
//! # EROFS vs Squashfs
//!
//! We use EROFS (Enhanced Read-Only File System) instead of squashfs because:
//! - Better random-access performance (no linear directory search)
//! - Fixed 4KB output blocks (better disk I/O alignment)
//! - Lower memory overhead during decompression
//! - Used by Fedora 42+, RHEL 10, Android
//!
//! # Architecture
//!
//! ```text
//! ISO Contents:
//! ├── boot/
//! │   ├── vmlinuz              # Kernel
//! │   └── initramfs.img        # Tiny (~5MB) - busybox + mount logic
//! ├── live/
//! │   └── filesystem.erofs     # COMPLETE system (~350MB)
//! └── EFI/BOOT/
//!     ├── BOOTX64.EFI
//!     └── grub.cfg
//!
//! Live Boot Flow:
//! 1. GRUB loads kernel + tiny initramfs
//! 2. Tiny init mounts ISO by LABEL
//! 3. Mounts filesystem.erofs read-only via loop device
//! 4. Creates overlay: erofs (lower) + tmpfs (upper)
//! 5. switch_root to overlay
//! 6. systemd boots as PID 1
//! ```
//!
//! DESIGN: Live = Installed (same content, zero duplication)
//!
//! # Implementation
//!
//! The actual EROFS building is done by `distro_builder::artifact::rootfs`.
//! This module provides the LevitateOS-specific orchestration (staging, atomicity).

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_spec::levitate::ROOTFS_NAME;
use distro_spec::shared::{
    FHS_DIRS, BIN_UTILS, AUTH_BIN, SSH_BIN, NM_BIN,
    ESSENTIAL_UNITS, NM_UNITS, WPA_UNITS,
    ETC_FILES,
};
use crate::build::BuildContext;
use distro_builder::build_erofs_default;

/// Build the complete rootfs (EROFS) system image.
///
/// This creates a filesystem.erofs in output/ containing the complete
/// LevitateOS system ready for both live boot and installation.
///
/// # Atomicity
///
/// Uses Gentoo-style "work directory" pattern to ensure build interruption
/// never corrupts existing artifacts:
/// - Build into `.work` files (rootfs-staging.work, filesystem.erofs.work)
/// - Only swap to final locations after successful completion
/// - If cancelled mid-build, existing rootfs-staging/ and filesystem.erofs are preserved
pub fn build_rootfs(base_dir: &Path) -> Result<()> {
    println!("=== Building EROFS System Image ===\n");

    check_host_tools()?;

    // Gentoo-style: separate "work" vs "final" locations
    let work_staging = base_dir.join("output/rootfs-staging.work");
    let work_output = base_dir.join("output/filesystem.erofs.work");
    let final_staging = base_dir.join("output/rootfs-staging");
    let final_output = base_dir.join("output").join(ROOTFS_NAME);

    // 1. Clean WORK directories only (preserve final)
    // Use let _ = to ignore errors (may not exist)
    let _ = fs::remove_dir_all(&work_staging);
    let _ = fs::remove_file(&work_output);
    fs::create_dir_all(&work_staging)?;

    // 2. Build into work directory (may fail - final is preserved)
    let build_result = (|| -> Result<()> {
        let ctx = BuildContext::new(base_dir, &work_staging)?;
        crate::component::build_system(&ctx)?;

        // Verify staging directory before creating EROFS
        verify_staging(&work_staging)?;

        // IMPORTANT: create_erofs_internal doesn't delete output first
        create_erofs_internal(&work_staging, &work_output)?;
        Ok(())
    })();

    // 3. On failure, clean up work files and propagate error
    if let Err(e) = build_result {
        let _ = fs::remove_dir_all(&work_staging);
        let _ = fs::remove_file(&work_output);
        return Err(e);
    }

    // 4. Atomic swap (only reached if build succeeded)
    // Order matters: remove old, then rename new
    // If rename fails after removal, we've lost the old but have the new in .work
    println!("\nSwapping work files to final locations...");
    let _ = fs::remove_dir_all(&final_staging);
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_staging, &final_staging)
        .context("Failed to move rootfs-staging.work to rootfs-staging")?;
    fs::rename(&work_output, &final_output)
        .context("Failed to move filesystem.erofs.work to filesystem.erofs")?;

    println!("\n=== EROFS Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Check that required host tools are available.
fn check_host_tools() -> Result<()> {
    use distro_builder::process;

    let tools = [
        ("mkfs.erofs", "erofs-utils"),
        ("readelf", "binutils"),
    ];

    for (tool, package) in tools {
        if !process::exists(tool) {
            anyhow::bail!(
                "{} not found. Install {} package.\n\
                 On Fedora: sudo dnf install {}\n\
                 On Ubuntu: sudo apt install {}\n\
                 \n\
                 NOTE: erofs-utils 1.8+ is required for zstd compression.",
                tool,
                package,
                package,
                package
            );
        }
    }

    Ok(())
}

/// Create an EROFS image from the staging directory (internal, non-destructive).
///
/// Uses the shared implementation from `distro_builder::artifact::rootfs`.
/// This ensures both LevitateOS and AcornOS use the same EROFS building code.
///
/// NOTE: This does NOT delete the output file first - mkfs.erofs creates a fresh file.
/// Caller is responsible for cleanup.
fn create_erofs_internal(staging: &Path, output: &Path) -> Result<()> {
    // Use the shared distro-builder implementation
    build_erofs_default(staging, output)
}

/// Verify the staging directory contains required files before creating EROFS.
///
/// Uses distro-spec constants to ensure the rootfs has all required components.
/// Fails the build immediately if ANY required file is missing.
fn verify_staging(staging: &Path) -> Result<()> {
    println!();
    println!("  Verifying staging directory...");

    let mut missing = Vec::new();
    let mut passed = 0;

    // Combine all bin lists (same logic as fsdbg)
    let all_bins: Vec<&str> = BIN_UTILS.iter()
        .chain(AUTH_BIN.iter())
        .chain(SSH_BIN.iter())
        .chain(NM_BIN.iter())
        .copied()
        .collect();

    // Check binaries (in usr/bin)
    for binary in &all_bins {
        let bin_path = format!("usr/bin/{}", binary);
        if staging.join(&bin_path).exists() {
            passed += 1;
        } else {
            missing.push(*binary);
        }
    }
    // Also check bash (handled separately)
    if staging.join("usr/bin/bash").exists() {
        passed += 1;
    } else {
        missing.push("bash");
    }

    // Combine all unit lists
    let all_units: Vec<&str> = ESSENTIAL_UNITS.iter()
        .chain(NM_UNITS.iter())
        .chain(WPA_UNITS.iter())
        .copied()
        // Filter out systemd-networkd/resolved - LevitateOS uses NetworkManager
        .filter(|u| !u.contains("networkd") && !u.contains("resolved"))
        // Filter out system.slice - auto-generated at runtime
        .filter(|u| *u != "system.slice")
        .collect();

    // Check units
    for unit in &all_units {
        let unit_path = format!("usr/lib/systemd/system/{}", unit);
        if staging.join(&unit_path).exists() {
            passed += 1;
        } else {
            missing.push(*unit);
        }
    }

    // Check directories
    for dir in FHS_DIRS {
        let dir_path = staging.join(dir);
        // Check for either directory or symlink (bin -> usr/bin)
        if dir_path.exists() || dir_path.is_symlink() {
            passed += 1;
        } else {
            missing.push(*dir);
        }
    }

    // Check /etc files
    for etc_file in ETC_FILES {
        if staging.join(etc_file).exists() {
            passed += 1;
        } else {
            missing.push(*etc_file);
        }
    }

    let total = passed + missing.len();

    if missing.is_empty() {
        println!("  ✓ Verification PASSED ({}/{} checks)", passed, total);
        Ok(())
    } else {
        println!(
            "  ✗ Verification FAILED ({}/{} checks)",
            passed, total
        );
        for item in &missing {
            println!("    ✗ {} - Missing", item);
        }
        bail!(
            "Rootfs verification FAILED: {} missing files.\n\
             The staging directory is incomplete and would produce a broken rootfs.",
            missing.len()
        );
    }
}
