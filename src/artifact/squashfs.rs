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

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use distro_spec::levitate::{SQUASHFS_BLOCK_SIZE, SQUASHFS_COMPRESSION, SQUASHFS_NAME};
use crate::build::BuildContext;
use crate::process::{self, Cmd};

/// Build the complete squashfs system image.
///
/// This creates a filesystem.squashfs in output/ containing the complete
/// LevitateOS system ready for both live boot and installation.
pub fn build_squashfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Squashfs System Image ===\n");

    // 1. Check host tools
    check_host_tools()?;

    // 2. Set up paths
    let staging = base_dir.join("output/squashfs-root");
    let output = base_dir.join("output").join(SQUASHFS_NAME);

    // 3. Clean staging if exists
    if staging.exists() {
        println!("Cleaning previous staging directory...");
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    // 4. Build complete system into staging
    let ctx = BuildContext::new(base_dir, &staging)?;
    crate::component::build_system(&ctx)?;

    // 5. Pack into squashfs
    create_squashfs(&staging, &output)?;

    println!("\n=== Squashfs Build Complete ===");
    println!("  Output: {}", output.display());

    // Print size info
    if let Ok(meta) = fs::metadata(&output) {
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

/// Create a squashfs image from the staging directory.
///
/// Uses gzip compression for universal kernel compatibility.
/// (zstd requires CONFIG_SQUASHFS_ZSTD=y which not all kernels have)
fn create_squashfs(staging: &Path, output: &Path) -> Result<()> {
    println!("Creating squashfs with {} compression...", SQUASHFS_COMPRESSION);

    // Remove existing if present
    if output.exists() {
        fs::remove_file(output)?;
    }

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // mksquashfs is interactive (shows progress), so use run_interactive
    Cmd::new("mksquashfs")
        .arg_path(staging)
        .arg_path(output)
        .args(["-comp", SQUASHFS_COMPRESSION]) // Universal compatibility - all kernels support gzip
        .args(["-b", SQUASHFS_BLOCK_SIZE]) // 1MB blocks for better compression
        .arg("-no-xattrs") // Skip extended attributes
        .arg("-noappend") // Always create fresh
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
