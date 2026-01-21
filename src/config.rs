//! Configuration management for leviso.
//!
//! Reads configuration from .env file and environment variables.
//! Environment variables take precedence over .env file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default git URL for LevitateOS Linux kernel fork.
pub const DEFAULT_LINUX_GIT_URL: &str = "https://github.com/LevitateOS/linux.git";

/// Leviso configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to Linux kernel source tree (default: downloads/linux)
    pub linux_source: PathBuf,
    /// Kernel version suffix (e.g., "-levitate")
    pub kernel_localversion: String,
    /// Git URL for Linux kernel
    pub linux_git_url: String,
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

        Self {
            linux_source,
            kernel_localversion,
            linux_git_url,
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
        if self.has_linux_source() {
            println!("  Linux source: FOUND");
        } else {
            println!("  Linux source: NOT FOUND (run 'leviso download-linux' to fetch)");
        }
    }
}
