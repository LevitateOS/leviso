//! Linux kernel via recipe.

use super::{find_recipe, run_recipe_json};
use anyhow::Result;
use distro_builder::process::ensure_exists;
use std::path::{Path, PathBuf};

/// Paths produced by the linux.rhai recipe after execution.
#[derive(Debug, Clone)]
pub struct LinuxPaths {
    /// Path to the kernel source tree.
    pub source: PathBuf,
    /// Path to vmlinuz in staging.
    pub vmlinuz: PathBuf,
    /// Kernel version string.
    pub version: String,
}

impl LinuxPaths {
    /// Check if kernel is built and installed.
    pub fn is_installed(&self) -> bool {
        self.vmlinuz.exists()
    }
}

/// Run the linux.rhai recipe and return the output paths.
///
/// This handles the full kernel workflow: acquire source, build, install to staging.
/// The recipe returns a ctx with all paths and the kernel version.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn linux(base_dir: &Path) -> Result<LinuxPaths> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/linux.rhai");

    ensure_exists(&recipe_path, "Linux recipe").map_err(|_| {
        anyhow::anyhow!(
            "Linux recipe not found at: {}\n\
             Expected linux.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    // Find and run recipe, parse JSON output
    let recipe_bin = find_recipe(&monorepo_dir)?;
    let ctx = run_recipe_json(&recipe_bin.path, &recipe_path, &downloads_dir)?;

    // Extract paths from ctx (recipe sets these)
    let output_dir = base_dir.join("output");

    let source = ctx["source_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // Fallback: check submodule first, then downloads
            let submodule = monorepo_dir.join("linux");
            if submodule.join("Makefile").exists() {
                submodule
            } else {
                downloads_dir.join("linux")
            }
        });

    let version = ctx["kernel_version"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let vmlinuz = output_dir.join("staging/boot/vmlinuz");

    Ok(LinuxPaths {
        source,
        vmlinuz,
        version,
    })
}

/// Check if Linux source is available (without running the full recipe).
pub fn has_linux_source(base_dir: &Path) -> bool {
    let monorepo_dir = base_dir.parent().unwrap_or(base_dir);

    // Check submodule
    if monorepo_dir.join("linux/Makefile").exists() {
        return true;
    }

    // Check downloads
    if base_dir.join("downloads/linux/Makefile").exists() {
        return true;
    }

    false
}
