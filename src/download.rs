//! Download management for leviso.
//!
//! Downloads Rocky Linux ISO and verifies checksums.
//! Configuration is loaded from .env file or environment variables.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RockyConfig;

/// Download Rocky Linux ISO using configuration.
pub fn download_rocky(base_dir: &Path, rocky: &RockyConfig) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let iso_path = rocky.iso_path(&downloads_dir);

    if iso_path.exists() {
        println!("Rocky DVD ISO already exists at {}", iso_path.display());
        return Ok(());
    }

    fs::create_dir_all(&downloads_dir)?;
    println!(
        "Downloading Rocky Linux {} DVD ISO ({})...",
        rocky.version, rocky.size
    );
    println!("URL: {}", rocky.url);

    let status = Command::new("curl")
        .args([
            "-L",
            "-o",
            iso_path.to_str().unwrap(),
            "--progress-bar",
            &rocky.url,
        ])
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        bail!("curl failed with status: {}", status);
    }

    // Verify download integrity
    verify_checksum(&iso_path, &rocky.sha256)?;

    println!("Downloaded to {}", iso_path.display());
    Ok(())
}

/// Verify SHA256 checksum of a downloaded file.
pub fn verify_checksum(file_path: &Path, expected_sha256: &str) -> Result<()> {
    println!("Verifying SHA256 checksum...");

    let output = Command::new("sha256sum")
        .arg(file_path.to_str().unwrap())
        .output()
        .context("Failed to run sha256sum")?;

    if !output.status.success() {
        bail!("sha256sum failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = stdout
        .split_whitespace()
        .next()
        .context("Could not parse sha256sum output")?;

    if actual != expected_sha256 {
        fs::remove_file(file_path)?;
        bail!(
            "Checksum mismatch!\n  Expected: {}\n  Got: {}\n\
             The download may be corrupted. Deleted partial file.",
            expected_sha256,
            actual
        );
    }

    println!("Checksum verified OK");
    Ok(())
}

