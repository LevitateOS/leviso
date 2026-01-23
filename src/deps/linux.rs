//! Linux kernel source resolution.

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

use super::DependencyResolver;

/// Default git URL for LevitateOS Linux kernel fork.
const DEFAULT_GIT_URL: &str = "https://github.com/LevitateOS/linux.git";

/// Resolved Linux kernel source.
#[derive(Debug, Clone)]
pub struct LinuxSource {
    /// Path to kernel source tree.
    pub path: PathBuf,
    /// How it was resolved.
    pub source: LinuxSourceType,
}

/// How the Linux source was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxSourceType {
    /// From LINUX_SOURCE env var.
    EnvVar,
    /// From ../linux submodule.
    Submodule,
    /// Downloaded to downloads/linux.
    Downloaded,
}

impl LinuxSource {
    /// Check if this is a valid kernel source tree.
    pub fn is_valid(&self) -> bool {
        self.path.join("Makefile").exists()
    }

    /// Get the kernel version from the source tree.
    pub fn version(&self) -> Result<String> {
        let makefile = self.path.join("Makefile");
        if !makefile.exists() {
            bail!("No Makefile in kernel source");
        }

        let content = std::fs::read_to_string(&makefile)?;
        let mut version = String::new();
        let mut patchlevel = String::new();
        let mut sublevel = String::new();

        for line in content.lines() {
            if let Some(v) = line.strip_prefix("VERSION = ") {
                version = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("PATCHLEVEL = ") {
                patchlevel = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("SUBLEVEL = ") {
                sublevel = v.trim().to_string();
            }
        }

        if version.is_empty() {
            bail!("Could not parse kernel version from Makefile");
        }

        Ok(format!("{}.{}.{}", version, patchlevel, sublevel))
    }
}

/// Find existing Linux source without downloading.
pub fn find_existing(resolver: &DependencyResolver) -> Option<LinuxSource> {
    // 1. Check env var
    if let Ok(path) = env::var("LINUX_SOURCE") {
        let path = PathBuf::from(path);
        if path.join("Makefile").exists() {
            return Some(LinuxSource {
                path,
                source: LinuxSourceType::EnvVar,
            });
        }
    }

    // 2. Check submodule at ../linux
    let submodule = resolver.monorepo_dir().join("linux");
    if submodule.join("Makefile").exists() {
        return Some(LinuxSource {
            path: submodule,
            source: LinuxSourceType::Submodule,
        });
    }

    // 3. Check downloads/linux
    let downloaded = resolver.downloads_dir().join("linux");
    if downloaded.join("Makefile").exists() {
        return Some(LinuxSource {
            path: downloaded,
            source: LinuxSourceType::Downloaded,
        });
    }

    None
}

/// Resolve Linux source, downloading if necessary.
pub fn resolve(resolver: &DependencyResolver) -> Result<LinuxSource> {
    // Check if already available
    if let Some(source) = find_existing(resolver) {
        println!("  Linux source: {} ({})", source.path.display(), match source.source {
            LinuxSourceType::EnvVar => "from LINUX_SOURCE",
            LinuxSourceType::Submodule => "submodule",
            LinuxSourceType::Downloaded => "downloaded",
        });
        return Ok(source);
    }

    // Need to download
    download(resolver)
}

/// Download Linux kernel source via git clone.
fn download(resolver: &DependencyResolver) -> Result<LinuxSource> {
    let git_url = env::var("LINUX_GIT_URL").unwrap_or_else(|_| DEFAULT_GIT_URL.to_string());
    let dest = resolver.downloads_dir().join("linux");

    println!("  Downloading Linux kernel source...");
    println!("    URL: {}", git_url);
    println!("    Destination: {}", dest.display());

    // Shallow clone by default (much faster)
    let shallow = env::var("LINUX_FULL_CLONE")
        .map(|v| v != "1" && v.to_lowercase() != "true")
        .unwrap_or(true);

    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if shallow {
        cmd.args(["--depth", "1"]);
        println!("    Mode: shallow clone (set LINUX_FULL_CLONE=1 for full history)");
    }
    cmd.arg(&git_url);
    cmd.arg(&dest);

    let status = cmd.status().context("Failed to run git clone")?;
    if !status.success() {
        bail!("git clone failed");
    }

    let source = LinuxSource {
        path: dest,
        source: LinuxSourceType::Downloaded,
    };

    if !source.is_valid() {
        bail!("Downloaded Linux source is invalid (no Makefile)");
    }

    println!("    Downloaded kernel version: {}", source.version().unwrap_or_else(|_| "unknown".to_string()));
    Ok(source)
}
