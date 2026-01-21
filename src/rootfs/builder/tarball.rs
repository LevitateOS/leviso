//! Tarball operations for rootfs builder.
//!
//! This module handles creating, listing, and extracting rootfs tarballs.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Create the tarball from the staging directory.
pub(super) fn create_tarball(staging: &Path, output_dir: &Path) -> Result<PathBuf> {
    println!("Creating tarball...");

    let tarball_path = output_dir.join("levitateos-base.tar.xz");

    // Use tar command for better compatibility and performance
    let status = Command::new("tar")
        .args([
            "-cJf",
            tarball_path.to_str().unwrap(),
            "-C",
            staging.to_str().unwrap(),
            ".",
        ])
        .status()
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar command failed with status: {}", status);
    }

    // Print tarball size
    let metadata = fs::metadata(&tarball_path)?;
    let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
    println!("  Tarball size: {:.2} MB", size_mb);

    Ok(tarball_path)
}

/// List contents of an existing tarball.
pub fn list_tarball(path: &Path) -> Result<()> {
    println!("Contents of {}:", path.display());

    let status = Command::new("tar")
        .args(["-tJf", path.to_str().unwrap()])
        .status()
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar command failed with status: {}", status);
    }

    Ok(())
}

/// Extract tarball to a directory for inspection.
pub fn extract_tarball(tarball: &Path, output_dir: &Path) -> Result<()> {
    if !tarball.exists() {
        anyhow::bail!(
            "Tarball not found: {}\nRun 'leviso rootfs' first to build it.",
            tarball.display()
        );
    }

    // Clean and create output directory
    if output_dir.exists() {
        println!("Removing existing {}...", output_dir.display());
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    println!("Extracting {} to {}...", tarball.display(), output_dir.display());

    let status = Command::new("tar")
        .args([
            "-xJf",
            tarball.to_str().unwrap(),
            "-C",
            output_dir.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar extraction failed with status: {}", status);
    }

    // Print summary
    let bin_count = fs::read_dir(output_dir.join("usr/bin"))
        .map(|d| d.count())
        .unwrap_or(0);
    let sbin_count = fs::read_dir(output_dir.join("usr/sbin"))
        .map(|d| d.count())
        .unwrap_or(0);

    println!("\nExtracted rootfs:");
    println!("  {} binaries in /usr/bin", bin_count);
    println!("  {} binaries in /usr/sbin", sbin_count);
    println!("\nInspect at: {}", output_dir.display());

    Ok(())
}
