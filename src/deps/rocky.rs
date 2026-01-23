//! Rocky Linux ISO resolution.

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

use super::download::{self, DownloadOptions};
use super::DependencyResolver;

/// Default Rocky Linux configuration.
pub mod defaults {
    pub const VERSION: &str = "10.1";
    pub const ARCH: &str = "x86_64";
    pub const URL: &str =
        "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso";
    pub const TORRENT_URL: &str =
        "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.torrent";
    pub const FILENAME: &str = "Rocky-10.1-x86_64-dvd1.iso";
    pub const SIZE_BYTES: u64 = 9_278_128_128; // ~8.6 GiB
    pub const SIZE: &str = "8.6GB";
    // From https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM
    pub const SHA256: &str = "bd29df7f8a99b6fc4686f52cbe9b46cf90e07f90be2c0c5f1f18c2ecdd432d34";
}

/// Resolved Rocky Linux ISO.
#[derive(Debug, Clone)]
pub struct RockyIso {
    /// Path to the ISO file.
    pub path: PathBuf,
    /// How it was resolved (useful for debugging/logging).
    #[allow(dead_code)]
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
    /// Downloaded via BitTorrent.
    Torrent,
    /// Downloaded via HTTP.
    Http,
}

/// Rocky Linux ISO configuration.
#[derive(Debug, Clone)]
pub struct RockyConfig {
    pub version: String,
    pub arch: String,
    pub url: String,
    pub torrent_url: String,
    pub filename: String,
    pub size: String,
    pub size_bytes: u64,
    pub sha256: String,
}

impl Default for RockyConfig {
    fn default() -> Self {
        Self {
            version: defaults::VERSION.to_string(),
            arch: defaults::ARCH.to_string(),
            url: defaults::URL.to_string(),
            torrent_url: defaults::TORRENT_URL.to_string(),
            filename: defaults::FILENAME.to_string(),
            size: defaults::SIZE.to_string(),
            size_bytes: defaults::SIZE_BYTES,
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
            torrent_url: env::var("ROCKY_TORRENT_URL")
                .unwrap_or_else(|_| defaults::TORRENT_URL.to_string()),
            filename: env::var("ROCKY_FILENAME")
                .unwrap_or_else(|_| defaults::FILENAME.to_string()),
            size: env::var("ROCKY_SIZE").unwrap_or_else(|_| defaults::SIZE.to_string()),
            size_bytes: env::var("ROCKY_SIZE_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults::SIZE_BYTES),
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
        download::verify_sha256(&self.path, &self.config.sha256, true)?;
        println!("  Checksum verified OK");
        Ok(())
    }
}

/// Validate an existing ISO file by checking size and optionally checksum.
fn validate_existing_iso(path: &Path, config: &RockyConfig, verify_checksum: bool) -> bool {
    // First check file exists
    if !path.exists() {
        return false;
    }

    // Check file size matches expected (catches partial downloads)
    match std::fs::metadata(path) {
        Ok(meta) => {
            let actual_size = meta.len();
            if actual_size != config.size_bytes {
                eprintln!(
                    "    Warning: {} size mismatch (expected {} bytes, got {} bytes)",
                    path.display(),
                    config.size_bytes,
                    actual_size
                );
                return false;
            }
        }
        Err(_) => return false,
    }

    // Optionally verify checksum (expensive but thorough)
    if verify_checksum {
        if let Err(e) = download::verify_sha256(path, &config.sha256, false) {
            eprintln!("    Warning: {} checksum mismatch: {}", path.display(), e);
            return false;
        }
    }

    true
}

/// Find existing Rocky ISO without downloading.
///
/// Note: This only checks file existence and size, not checksum (for speed).
/// Use `resolve()` for full validation with checksum.
pub fn find_existing(resolver: &DependencyResolver) -> Option<RockyIso> {
    let config = RockyConfig::from_env();

    // 1. Check env var for existing ISO
    if let Ok(path) = env::var("ROCKY_ISO_PATH") {
        let path = PathBuf::from(path);
        if validate_existing_iso(&path, &config, false) {
            return Some(RockyIso {
                path,
                source: RockySourceType::EnvVar,
                config,
            });
        }
    }

    // 2. Check downloads directory
    let downloaded = resolver.downloads_dir().join(&config.filename);
    if validate_existing_iso(&downloaded, &config, false) {
        return Some(RockyIso {
            path: downloaded,
            source: RockySourceType::ExistingDownload,
            config,
        });
    }

    None
}

/// Resolve Rocky ISO, downloading if necessary.
///
/// This verifies the checksum of existing files to catch corrupted downloads.
pub fn resolve(resolver: &DependencyResolver) -> Result<RockyIso> {
    let config = RockyConfig::from_env();

    // Check if already available (with full checksum verification)
    // 1. Check env var for existing ISO
    if let Ok(path) = env::var("ROCKY_ISO_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            println!("  Rocky ISO: {} (from ROCKY_ISO_PATH)", path.display());
            println!("  Verifying checksum...");
            if validate_existing_iso(&path, &config, true) {
                println!("  Checksum OK");
                return Ok(RockyIso {
                    path,
                    source: RockySourceType::EnvVar,
                    config,
                });
            } else {
                println!("  Checksum FAILED - file may be corrupted");
                // Don't delete user-provided file, just warn
                anyhow::bail!(
                    "Rocky ISO at {} is corrupted (checksum mismatch). Please re-download or remove ROCKY_ISO_PATH.",
                    path.display()
                );
            }
        }
    }

    // 2. Check downloads directory
    let downloaded = resolver.downloads_dir().join(&config.filename);
    if downloaded.exists() {
        println!("  Rocky ISO: {} (existing download)", downloaded.display());
        println!("  Verifying checksum...");
        if validate_existing_iso(&downloaded, &config, true) {
            println!("  Checksum OK");
            return Ok(RockyIso {
                path: downloaded,
                source: RockySourceType::ExistingDownload,
                config,
            });
        } else {
            println!("  Checksum FAILED - removing corrupted file");
            std::fs::remove_file(&downloaded).ok();
            // Fall through to download
        }
    }

    // Need to download - use tokio runtime
    download_rocky(resolver, config)
}

/// Download Rocky Linux ISO using BitTorrent (preferred) or HTTP fallback.
fn download_rocky(resolver: &DependencyResolver, config: RockyConfig) -> Result<RockyIso> {
    let downloads_dir = resolver.downloads_dir().to_path_buf();
    let expected_dest = downloads_dir.join(&config.filename);

    println!(
        "  Downloading Rocky Linux {} ISO ({})...",
        config.version, config.size
    );

    // Check disk space before starting (need ~10GB for ISO + temp space)
    let required_space = config.size_bytes + (2 * 1024 * 1024 * 1024); // ISO + 2GB buffer
    download::check_disk_space(&downloads_dir, required_space)?;

    // Create a new tokio runtime for the download
    let rt = tokio::runtime::Runtime::new()?;

    // Try BitTorrent first, fall back to HTTP
    let (final_path, source) = rt.block_on(async {
        // Try BitTorrent
        println!("    Method: BitTorrent");
        match download::torrent(
            &config.torrent_url,
            &downloads_dir,
            &DownloadOptions::large_file(config.size_bytes),
        )
        .await
        {
            Ok(torrent_path) => {
                println!("    Downloaded successfully via BitTorrent");
                // Torrent may have different filename - rename if needed
                if torrent_path != expected_dest {
                    println!("    Renaming {} -> {}", torrent_path.display(), expected_dest.display());
                    if expected_dest.exists() {
                        std::fs::remove_file(&expected_dest).ok();
                    }
                    std::fs::rename(&torrent_path, &expected_dest)
                        .with_context(|| format!(
                            "Failed to rename {} to {}",
                            torrent_path.display(),
                            expected_dest.display()
                        ))?;
                }
                return Ok((expected_dest.clone(), RockySourceType::Torrent));
            }
            Err(e) => {
                println!("    BitTorrent failed: {}", e);
                println!("    Falling back to HTTP...");
            }
        }

        // Fall back to HTTP
        println!("    Method: HTTP");
        download::http(
            &config.url,
            &expected_dest,
            &DownloadOptions::large_file(config.size_bytes),
        )
        .await?;
        println!("    Downloaded successfully via HTTP");
        Ok::<_, anyhow::Error>((expected_dest.clone(), RockySourceType::Http))
    })?;

    let iso = RockyIso {
        path: final_path,
        source,
        config,
    };

    // Verify checksum - if it fails, delete the corrupted file
    if let Err(e) = iso.verify_checksum() {
        eprintln!("  Checksum verification failed, removing corrupted download");
        std::fs::remove_file(&iso.path).ok();
        return Err(e);
    }

    Ok(iso)
}
