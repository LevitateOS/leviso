//! Squashfs packing using mksquashfs.
//!
//! Creates the final filesystem.squashfs from the staging directory.

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::process::Cmd;

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

    // mksquashfs is interactive (shows progress), so use run_interactive
    Cmd::new("mksquashfs")
        .arg_path(staging)
        .arg_path(output)
        .args(["-comp", "gzip"]) // Universal compatibility - all kernels support gzip
        .args(["-b", "1M"]) // 1MB blocks for better compression
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
