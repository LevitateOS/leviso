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

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_spec::levitate::{
    EROFS_CHUNK_SIZE, EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, ROOTFS_NAME,
};
use crate::build::BuildContext;
use distro_builder::process::{self, Cmd};

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
    let tools = [
        ("mkfs.erofs", "erofs-utils"),
        ("readelf", "binutils"),
    ];

    for (tool, package) in tools {
        if !process::exists(tool) {
            bail!(
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
/// Uses zstd compression at level 6 (Fedora's choice - good balance).
/// Requires kernel CONFIG_EROFS_FS_ZIP_ZSTD=y (Linux 6.10+).
///
/// NOTE: This does NOT delete the output file first - mkfs.erofs creates a fresh file.
/// Caller is responsible for cleanup.
fn create_erofs_internal(staging: &Path, output: &Path) -> Result<()> {
    println!(
        "Creating EROFS with {} compression (level {})...",
        EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL
    );

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // Format compression argument: algorithm,level
    let compression_arg = format!("{},{}", EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL);

    // IMPORTANT: mkfs.erofs argument order is OUTPUT SOURCE (opposite of mksquashfs!)
    Cmd::new("mkfs.erofs")
        .args(["-z", &compression_arg])  // zstd,6
        .args(["-C", &EROFS_CHUNK_SIZE.to_string()])  // 1MB chunks
        .arg("--all-root")  // All files owned by root (required for sshd, etc.)
        .arg("-T0")  // Reproducible builds (timestamp=0)
        .arg_path(output)   // OUTPUT FIRST (different from mksquashfs!)
        .arg_path(staging)  // SOURCE SECOND
        .error_msg(
            "mkfs.erofs failed. Install erofs-utils: sudo dnf install erofs-utils\n\
             NOTE: erofs-utils 1.8+ is required for zstd compression."
        )
        .run_interactive()?;

    // Print size
    let metadata = fs::metadata(output)?;
    println!(
        "EROFS created: {} MB",
        metadata.len() / 1024 / 1024
    );

    Ok(())
}
