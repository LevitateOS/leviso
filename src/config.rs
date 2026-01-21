//! Configuration management for leviso.
//!
//! Reads configuration from .env file and environment variables.
//! Environment variables take precedence over .env file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default git URL for LevitateOS Linux kernel fork.
pub const DEFAULT_LINUX_GIT_URL: &str = "https://github.com/LevitateOS/linux.git";

/// Default Rocky Linux configuration.
pub mod rocky_defaults {
    pub const VERSION: &str = "10.1";
    pub const ARCH: &str = "x86_64";
    pub const URL: &str =
        "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso";
    pub const FILENAME: &str = "Rocky-10.1-x86_64-dvd1.iso";
    pub const SIZE: &str = "8.6GB";
    // From https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM
    pub const SHA256: &str = "bd29df7f8a99b6fc4686f52cbe9b46cf90e07f90be2c0c5f1f18c2ecdd432d34";
}

/// Rocky Linux source configuration.
#[derive(Debug, Clone)]
pub struct RockyConfig {
    /// Rocky version (e.g., "10.1")
    pub version: String,
    /// Architecture (e.g., "x86_64")
    pub arch: String,
    /// Download URL for the ISO
    pub url: String,
    /// ISO filename
    pub filename: String,
    /// Human-readable size (e.g., "8.6GB")
    pub size: String,
    /// SHA256 checksum for verification
    pub sha256: String,
}

impl Default for RockyConfig {
    fn default() -> Self {
        Self {
            version: rocky_defaults::VERSION.to_string(),
            arch: rocky_defaults::ARCH.to_string(),
            url: rocky_defaults::URL.to_string(),
            filename: rocky_defaults::FILENAME.to_string(),
            size: rocky_defaults::SIZE.to_string(),
            sha256: rocky_defaults::SHA256.to_string(),
        }
    }
}

impl RockyConfig {
    /// Load Rocky config from environment variables.
    fn from_env(env_vars: &HashMap<String, String>) -> Self {
        Self {
            version: env_vars
                .get("ROCKY_VERSION")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::VERSION.to_string()),
            arch: env_vars
                .get("ROCKY_ARCH")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::ARCH.to_string()),
            url: env_vars
                .get("ROCKY_URL")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::URL.to_string()),
            filename: env_vars
                .get("ROCKY_FILENAME")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::FILENAME.to_string()),
            size: env_vars
                .get("ROCKY_SIZE")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::SIZE.to_string()),
            sha256: env_vars
                .get("ROCKY_SHA256")
                .cloned()
                .unwrap_or_else(|| rocky_defaults::SHA256.to_string()),
        }
    }

    /// Get the ISO path within the downloads directory.
    pub fn iso_path(&self, downloads_dir: &Path) -> PathBuf {
        downloads_dir.join(&self.filename)
    }
}

/// Default essential modules for initramfs.
pub mod module_defaults {
    /// Default essential kernel modules.
    /// Format: paths relative to /lib/modules/<version>/
    pub const ESSENTIAL_MODULES: &[&str] = &[
        // Block device driver (for virtual disks)
        "kernel/drivers/block/virtio_blk.ko.xz",
        // ext4 filesystem and dependencies
        "kernel/fs/mbcache.ko.xz",
        "kernel/fs/jbd2/jbd2.ko.xz",
        "kernel/fs/ext4/ext4.ko.xz",
        // FAT/vfat filesystem for EFI partition
        "kernel/fs/fat/fat.ko.xz",
        "kernel/fs/fat/vfat.ko.xz",
        // SCSI/CD-ROM support (for installation media access)
        "kernel/drivers/scsi/virtio_scsi.ko.xz",
        "kernel/drivers/cdrom/cdrom.ko.xz",
        "kernel/drivers/scsi/sr_mod.ko.xz",
        // ISO 9660 filesystem (to mount installation media)
        "kernel/fs/isofs/isofs.ko.xz",
        // Network - virtio (VM networking)
        "kernel/net/core/failover.ko.xz",       // Required by net_failover
        "kernel/drivers/net/net_failover.ko.xz", // Required by virtio_net
        "kernel/drivers/net/virtio_net.ko.xz",
        // Network - common ethernet drivers
        "kernel/drivers/net/ethernet/intel/e1000/e1000.ko.xz",
        "kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko.xz",
        "kernel/drivers/net/ethernet/realtek/r8169.ko.xz",
    ];
}

/// Leviso configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to Linux kernel source tree (default: downloads/linux)
    pub linux_source: PathBuf,
    /// Kernel version suffix (e.g., "-levitate")
    pub kernel_localversion: String,
    /// Git URL for Linux kernel
    pub linux_git_url: String,
    /// Rocky Linux source configuration
    pub rocky: RockyConfig,
    /// Additional kernel modules to include in initramfs
    pub extra_modules: Vec<String>,
}

impl Config {
    /// Load configuration from .env file and environment.
    ///
    /// Searches for .env in:
    /// 1. Current directory
    /// 2. Leviso base directory (CARGO_MANIFEST_DIR)
    pub fn load(base_dir: &Path) -> Self {
        let mut env_vars = HashMap::new();

        // Try to load .env file
        let env_path = base_dir.join(".env");
        if env_path.exists() {
            if let Ok(content) = fs::read_to_string(&env_path) {
                for line in content.lines() {
                    let line = line.trim();
                    // Skip comments and empty lines
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    // Parse KEY=value
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim();
                        // Remove quotes if present
                        let value = value.trim_matches('"').trim_matches('\'');
                        env_vars.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        // Environment variables override .env file
        for (key, value) in std::env::vars() {
            env_vars.insert(key, value);
        }

        // Build config with defaults
        // Default: downloads/linux (like other downloaded dependencies)
        let linux_source = env_vars
            .get("LINUX_SOURCE")
            .map(|s| {
                let path = PathBuf::from(s);
                if path.is_absolute() {
                    path
                } else {
                    base_dir.join(path)
                }
            })
            .unwrap_or_else(|| base_dir.join("downloads/linux"));

        let kernel_localversion = env_vars
            .get("KERNEL_LOCALVERSION")
            .cloned()
            .unwrap_or_else(|| "-levitate".to_string());

        let linux_git_url = env_vars
            .get("LINUX_GIT_URL")
            .cloned()
            .unwrap_or_else(|| DEFAULT_LINUX_GIT_URL.to_string());

        let rocky = RockyConfig::from_env(&env_vars);

        // Parse extra modules from comma-separated list
        let extra_modules = env_vars
            .get("EXTRA_MODULES")
            .map(|s| {
                s.split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            linux_source,
            kernel_localversion,
            linux_git_url,
            rocky,
            extra_modules,
        }
    }

    /// Check if Linux source is available.
    pub fn has_linux_source(&self) -> bool {
        self.linux_source.join("Makefile").exists()
    }

    /// Print configuration for debugging.
    pub fn print(&self) {
        println!("Configuration:");
        println!("  LINUX_SOURCE: {}", self.linux_source.display());
        println!("  KERNEL_LOCALVERSION: {}", self.kernel_localversion);
        println!("  LINUX_GIT_URL: {}", self.linux_git_url);
        println!();
        println!("  Rocky Linux:");
        println!("    ROCKY_VERSION: {}", self.rocky.version);
        println!("    ROCKY_ARCH: {}", self.rocky.arch);
        println!("    ROCKY_URL: {}", self.rocky.url);
        println!("    ROCKY_FILENAME: {}", self.rocky.filename);
        println!("    ROCKY_SHA256: {}...", &self.rocky.sha256[..16]);
        println!();
        if !self.extra_modules.is_empty() {
            println!("  Extra modules:");
            for module in &self.extra_modules {
                println!("    - {}", module);
            }
            println!();
        }
        if self.has_linux_source() {
            println!("  Linux source: FOUND");
        } else {
            println!("  Linux source: NOT FOUND (run 'leviso download linux' to fetch)");
        }
    }

    /// Get all modules (defaults + extra).
    pub fn all_modules(&self) -> Vec<&str> {
        let mut modules: Vec<&str> = module_defaults::ESSENTIAL_MODULES.to_vec();
        for extra in &self.extra_modules {
            modules.push(extra);
        }
        modules
    }
}
