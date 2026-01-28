//! Rocky Linux dependency via recipe.

use super::{find_recipe, run_recipe_json};
use anyhow::{bail, Result};
use distro_builder::process::ensure_exists;
use std::path::{Path, PathBuf};

/// Paths produced by the rocky.rhai recipe after execution.
#[derive(Debug, Clone)]
pub struct RockyPaths {
    /// Path to the downloaded ISO.
    pub iso: PathBuf,
    /// Path to the extracted rootfs.
    pub rootfs: PathBuf,
    /// Path to the extracted ISO contents (packages).
    pub iso_contents: PathBuf,
}

impl RockyPaths {
    /// Check if all paths exist.
    pub fn exists(&self) -> bool {
        self.iso.exists() && self.rootfs.exists() && self.iso_contents.exists()
    }
}

/// Run the rocky.rhai recipe and return the output paths.
///
/// This is the entry point for leviso to use recipe for Rocky dependency.
/// The recipe returns a ctx with paths, so we don't need to hardcode them.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
///
/// # Returns
/// The paths to the Rocky artifacts (ISO, rootfs, iso-contents).
pub fn rocky(base_dir: &Path) -> Result<RockyPaths> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/rocky.rhai");

    ensure_exists(&recipe_path, "Rocky recipe").map_err(|_| {
        anyhow::anyhow!(
            "Rocky recipe not found at: {}\n\
             Expected rocky.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    // Find and run recipe, parse JSON output
    let recipe_bin = find_recipe(&monorepo_dir)?;
    let ctx = run_recipe_json(&recipe_bin.path, &recipe_path, &downloads_dir)?;

    // Extract paths from ctx (recipe sets these)
    let iso = ctx["iso_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("Rocky-10.1-x86_64-dvd1.iso"));

    let rootfs = ctx["rootfs_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("rootfs"));

    let iso_contents = ctx["iso_contents_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("iso-contents"));

    let paths = RockyPaths {
        iso,
        rootfs,
        iso_contents,
    };

    if !paths.exists() {
        bail!(
            "Recipe completed but expected paths are missing:\n\
             - ISO: {} ({})\n\
             - rootfs: {} ({})\n\
             - iso-contents: {} ({})",
            paths.iso.display(),
            if paths.iso.exists() { "OK" } else { "MISSING" },
            paths.rootfs.display(),
            if paths.rootfs.exists() { "OK" } else { "MISSING" },
            paths.iso_contents.display(),
            if paths.iso_contents.exists() { "OK" } else { "MISSING" },
        );
    }

    Ok(paths)
}
