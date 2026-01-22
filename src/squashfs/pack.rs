//! Squashfs packing using mksquashfs.
//!
//! Creates the final filesystem.squashfs from the staging directory.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Create a squashfs image from the staging directory.
///
/// Uses gzip compression for universal kernel compatibility.
/// (zstd requires CONFIG_SQUASHFS_ZSTD=y which not all kernels have)
pub fn create_squashfs(staging: &Path, output: &Path) -> Result<()> {
    println!("Creating squashfs with gzip compression...");

    // Remove existing if present
    if output.exists() {
        fs::remove_file(output)?;
    }

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new("mksquashfs")
        .args([
            staging.to_str().unwrap(),
            output.to_str().unwrap(),
            "-comp",
            "gzip", // Universal compatibility - all kernels support gzip
            "-b",
            "1M", // 1MB blocks for better compression
            "-no-xattrs", // Skip extended attributes
            "-noappend", // Always create fresh
            "-progress", // Show progress
        ])
        .status()
        .context("mksquashfs not found. Install squashfs-tools.")?;

    if !status.success() {
        bail!("mksquashfs failed");
    }

    // Print size
    let metadata = fs::metadata(output)?;
    println!(
        "Squashfs created: {} MB",
        metadata.len() / 1024 / 1024
    );

    Ok(())
}
