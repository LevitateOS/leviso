//! Rocky Linux ISO resolution.

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

use super::DependencyResolver;

/// Default Rocky Linux configuration.
pub mod defaults {
    pub const VERSION: &str = "10.1";
    pub const ARCH: &str = "x86_64";
    pub const URL: &str =
        "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso";
    pub const FILENAME: &str = "Rocky-10.1-x86_64-dvd1.iso";
    pub const SIZE: &str = "8.6GB";
    // From https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM
    pub const SHA256: &str = "bd29df7f8a99b6fc4686f52cbe9b46cf90e07f90be2c0c5f1f18c2ecdd432d34";
}

/// Resolved Rocky Linux ISO.
#[derive(Debug, Clone)]
pub struct RockyIso {
    /// Path to the ISO file.
    pub path: PathBuf,
    /// How it was resolved.
    pub source: RockySourceType,
    /// ISO configuration.
    pub config: RockyConfig,
}

/// How the Rocky ISO was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RockySourceType {
    /// From ROCKY_ISO_PATH env var.
    EnvVar,
    /// Found in downloads directory.
    ExistingDownload,
    /// Downloaded from Rocky mirrors.
    Downloaded,
}

/// Rocky Linux ISO configuration.
#[derive(Debug, Clone)]
pub struct RockyConfig {
    pub version: String,
    pub arch: String,
    pub url: String,
    pub filename: String,
    pub size: String,
    pub sha256: String,
}

impl Default for RockyConfig {
    fn default() -> Self {
        Self {
            version: defaults::VERSION.to_string(),
            arch: defaults::ARCH.to_string(),
            url: defaults::URL.to_string(),
            filename: defaults::FILENAME.to_string(),
            size: defaults::SIZE.to_string(),
            sha256: defaults::SHA256.to_string(),
        }
    }
}

impl RockyConfig {
    /// Load config from environment variables.
    pub fn from_env() -> Self {
        Self {
            version: env::var("ROCKY_VERSION").unwrap_or_else(|_| defaults::VERSION.to_string()),
            arch: env::var("ROCKY_ARCH").unwrap_or_else(|_| defaults::ARCH.to_string()),
            url: env::var("ROCKY_URL").unwrap_or_else(|_| defaults::URL.to_string()),
            filename: env::var("ROCKY_FILENAME").unwrap_or_else(|_| defaults::FILENAME.to_string()),
            size: env::var("ROCKY_SIZE").unwrap_or_else(|_| defaults::SIZE.to_string()),
            sha256: env::var("ROCKY_SHA256").unwrap_or_else(|_| defaults::SHA256.to_string()),
        }
    }
}

impl RockyIso {
    /// Check if the ISO file exists.
    pub fn is_valid(&self) -> bool {
        self.path.exists()
    }

    /// Verify the ISO checksum.
    pub fn verify_checksum(&self) -> Result<()> {
        println!("  Verifying SHA256 checksum...");

        let output = Command::new("sha256sum")
            .arg(&self.path)
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

        if actual != self.config.sha256 {
            bail!(
                "Checksum mismatch!\n  Expected: {}\n  Got: {}",
                self.config.sha256,
                actual
            );
        }

        println!("  Checksum verified OK");
        Ok(())
    }
}

/// Find existing Rocky ISO without downloading.
pub fn find_existing(resolver: &DependencyResolver) -> Option<RockyIso> {
    let config = RockyConfig::from_env();

    // 1. Check env var for existing ISO
    if let Ok(path) = env::var("ROCKY_ISO_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(RockyIso {
                path,
                source: RockySourceType::EnvVar,
                config,
            });
        }
    }

    // 2. Check downloads directory
    let downloaded = resolver.downloads_dir().join(&config.filename);
    if downloaded.exists() {
        return Some(RockyIso {
            path: downloaded,
            source: RockySourceType::ExistingDownload,
            config,
        });
    }

    None
}

/// Resolve Rocky ISO, downloading if necessary.
pub fn resolve(resolver: &DependencyResolver) -> Result<RockyIso> {
    let config = RockyConfig::from_env();

    // Check if already available
    if let Some(iso) = find_existing(resolver) {
        println!(
            "  Rocky ISO: {} ({})",
            iso.path.display(),
            match iso.source {
                RockySourceType::EnvVar => "from ROCKY_ISO_PATH",
                RockySourceType::ExistingDownload => "existing download",
                RockySourceType::Downloaded => "downloaded",
            }
        );
        return Ok(iso);
    }

    // Need to download
    download(resolver, config)
}

/// Download Rocky Linux ISO.
fn download(resolver: &DependencyResolver, config: RockyConfig) -> Result<RockyIso> {
    let dest = resolver.downloads_dir().join(&config.filename);

    println!("  Downloading Rocky Linux {} ISO ({})...", config.version, config.size);
    println!("    URL: {}", config.url);
    println!("    Destination: {}", dest.display());

    let status = Command::new("curl")
        .args(["-L", "-o"])
        .arg(&dest)
        .arg("--progress-bar")
        .arg(&config.url)
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        bail!("curl failed to download ISO");
    }

    let iso = RockyIso {
        path: dest,
        source: RockySourceType::Downloaded,
        config,
    };

    // Verify checksum
    iso.verify_checksum()?;

    println!("    Downloaded successfully");
    Ok(iso)
}
