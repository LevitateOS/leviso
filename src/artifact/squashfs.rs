//! Squashfs builder - creates the complete LevitateOS system image.
//!
//! The squashfs serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! # Architecture
//!
//! ```text
//! ISO Contents:
//! ├── boot/
//! │   ├── vmlinuz              # Kernel
//! │   └── initramfs.img        # Tiny (~5MB) - busybox + mount logic
//! ├── live/
//! │   └── filesystem.squashfs  # COMPLETE system (~350MB)
//! └── EFI/BOOT/
//!     ├── BOOTX64.EFI
//!     └── grub.cfg
//!
//! Live Boot Flow:
//! 1. GRUB loads kernel + tiny initramfs
//! 2. Tiny init mounts ISO by LABEL
//! 3. Mounts filesystem.squashfs read-only via loop device
//! 4. Creates overlay: squashfs (lower) + tmpfs (upper)
//! 5. switch_root to overlay
//! 6. systemd boots as PID 1
//! ```
//!
//! DESIGN: Live = Installed (same content, zero duplication)

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_spec::levitate::{SQUASHFS_BLOCK_SIZE, SQUASHFS_COMPRESSION, SQUASHFS_NAME};
use crate::build::BuildContext;
use distro_builder::process::{self, Cmd};

/// Build the complete squashfs system image.
///
/// This creates a filesystem.squashfs in output/ containing the complete
/// LevitateOS system ready for both live boot and installation.
///
/// # Atomicity
///
/// Uses Gentoo-style "work directory" pattern to ensure build interruption
/// never corrupts existing artifacts:
/// - Build into `.work` files (squashfs-root.work, filesystem.squashfs.work)
/// - Only swap to final locations after successful completion
/// - If cancelled mid-build, existing squashfs-root/ and filesystem.squashfs are preserved
pub fn build_squashfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Squashfs System Image ===\n");

    check_host_tools()?;

    // Gentoo-style: separate "work" vs "final" locations
    let work_staging = base_dir.join("output/squashfs-root.work");
    let work_output = base_dir.join("output/filesystem.squashfs.work");
    let final_staging = base_dir.join("output/squashfs-root");
    let final_output = base_dir.join("output").join(SQUASHFS_NAME);

    // 1. Clean WORK directories only (preserve final)
    // Use let _ = to ignore errors (may not exist)
    let _ = fs::remove_dir_all(&work_staging);
    let _ = fs::remove_file(&work_output);
    fs::create_dir_all(&work_staging)?;

    // 2. Build into work directory (may fail - final is preserved)
    let build_result = (|| -> Result<()> {
        let ctx = BuildContext::new(base_dir, &work_staging)?;
        crate::component::build_system(&ctx)?;
        // IMPORTANT: create_squashfs_internal doesn't delete output first
        create_squashfs_internal(&work_staging, &work_output)?;
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
        .context("Failed to move squashfs-root.work to squashfs-root")?;
    fs::rename(&work_output, &final_output)
        .context("Failed to move filesystem.squashfs.work to filesystem.squashfs")?;

    println!("\n=== Squashfs Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Check that required host tools are available.
fn check_host_tools() -> Result<()> {
    let tools = [
        ("mksquashfs", "squashfs-tools"),
        ("readelf", "binutils"),
    ];

    for (tool, package) in tools {
        if !process::exists(tool) {
            bail!(
                "{} not found. Install {} package.\n\
                 On Fedora: sudo dnf install {}\n\
                 On Ubuntu: sudo apt install {}",
                tool,
                package,
                package,
                package
            );
        }
    }

    Ok(())
}

/// Create a squashfs image from the staging directory (internal, non-destructive).
///
/// Uses gzip compression for universal kernel compatibility.
/// (zstd requires CONFIG_SQUASHFS_ZSTD=y which not all kernels have)
///
/// NOTE: This does NOT delete the output file first - it uses mksquashfs -noappend
/// which creates a fresh file. Caller is responsible for cleanup.
fn create_squashfs_internal(staging: &Path, output: &Path) -> Result<()> {
    println!("Creating squashfs with {} compression...", SQUASHFS_COMPRESSION);

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // mksquashfs with -noappend creates fresh file (overwrites if exists)
    Cmd::new("mksquashfs")
        .arg_path(staging)
        .arg_path(output)
        .args(["-comp", SQUASHFS_COMPRESSION]) // Universal compatibility - all kernels support gzip
        .args(["-b", SQUASHFS_BLOCK_SIZE]) // 1MB blocks for better compression
        .arg("-no-xattrs") // Skip extended attributes
        .arg("-noappend") // Always create fresh (overwrites existing)
        .arg("-all-root") // Make all files owned by root (required for sshd, etc.)
        .arg("-progress") // Show progress
        .error_msg("mksquashfs failed. Install squashfs-tools: sudo dnf install squashfs-tools")
        .run_interactive()?;

    // Print size
    let metadata = fs::metadata(output)?;
    println!(
        "Squashfs created: {} MB",
        metadata.len() / 1024 / 1024
    );

    Ok(())
}
