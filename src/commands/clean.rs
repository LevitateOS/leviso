//! Clean command - removes build artifacts.

use anyhow::Result;
use std::path::Path;

use crate::clean;
use leviso_deps::DependencyResolver;

/// Clean target for the clean command.
pub enum CleanTarget {
    /// Clean outputs only (default)
    Outputs,
    /// Clean kernel build
    Kernel,
    /// Clean ISO and initramfs
    Iso,
    /// Clean squashfs
    Squashfs,
    /// Clean downloads
    Downloads,
    /// Clean tool cache
    Cache,
    /// Clean everything
    All,
}

/// Execute the clean command.
pub fn cmd_clean(base_dir: &Path, target: CleanTarget, resolver: &DependencyResolver) -> Result<()> {
    match target {
        CleanTarget::Outputs => {
            clean::clean_outputs(base_dir)?;
        }
        CleanTarget::Kernel => {
            clean::clean_kernel(base_dir)?;
        }
        CleanTarget::Iso => {
            clean::clean_iso(base_dir)?;
        }
        CleanTarget::Squashfs => {
            clean::clean_squashfs(base_dir)?;
        }
        CleanTarget::Downloads => {
            clean::clean_downloads(base_dir)?;
        }
        CleanTarget::Cache => {
            println!("Clearing tool cache (~/.cache/levitate/)...");
            resolver.clear_cache()?;
            println!("Cache cleared.");
        }
        CleanTarget::All => {
            clean::clean_all(base_dir)?;
            resolver.clear_cache()?;
        }
    }
    Ok(())
}
