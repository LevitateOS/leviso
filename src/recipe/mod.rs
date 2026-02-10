//! Recipe binary resolution and execution.
//!
//! Recipe is the general-purpose package manager used by leviso to manage
//! build dependencies like the Rocky Linux ISO.
//!
//! Recipe returns structured JSON to stdout (logs go to stderr), so leviso
//! can parse the ctx to get paths instead of hardcoding them.
//!
//! Resolution and execution are delegated to distro-builder's shared implementation.

mod linux;
mod rocky;

pub use linux::{has_linux_source, linux, LinuxPaths};
pub use rocky::{rocky, RockyPaths};

// Re-export shared recipe infrastructure from distro-builder
pub use distro_builder::recipe::{
    clear_cache, find_recipe, run_recipe, run_recipe_json, run_recipe_json_with_defines,
    RecipeBinary,
};

use anyhow::{bail, Result};
use distro_builder::process::ensure_exists;
use distro_spec::shared::LEVITATE_CARGO_TOOLS;
use std::path::Path;

// ============================================================================
// Installation tools via recipes (recstrap, recfstab, recchroot)
// ============================================================================

/// Run the tool recipes to install recstrap, recfstab, recchroot to staging.
///
/// These tools are required for the live ISO to be able to install itself.
/// The recipes install binaries to output/staging/usr/bin/.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn install_tools(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let staging_bin = base_dir.join("output/staging/usr/bin");

    // Find recipe binary once
    let recipe_bin = find_recipe(&monorepo_dir)?;

    // Run each tool recipe
    for tool in LEVITATE_CARGO_TOOLS {
        let recipe_path = base_dir.join(format!("deps/{}.rhai", tool));
        let installed_path = staging_bin.join(tool);

        // Skip if already installed
        if installed_path.exists() {
            println!("  {} already installed", tool);
            continue;
        }

        ensure_exists(&recipe_path, &format!("{} recipe", tool)).map_err(|_| {
            anyhow::anyhow!(
                "{} recipe not found at: {}\n\
                 Expected {}.rhai in leviso/deps/",
                tool,
                recipe_path.display(),
                tool
            )
        })?;

        recipe_bin.run(&recipe_path, &downloads_dir)?;

        // Verify installation
        if !installed_path.exists() {
            bail!(
                "Recipe completed but {} not found at: {}",
                tool,
                installed_path.display()
            );
        }
    }

    Ok(())
}

// ============================================================================
// Supplementary packages via recipe
// ============================================================================

/// Run the packages.rhai recipe to extract supplementary RPMs into rootfs.
///
/// This must be called after `rocky()` since it depends on the rootfs and
/// iso-contents directories created by rocky.rhai.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn packages(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/packages.rhai");

    ensure_exists(&recipe_path, "Packages recipe").map_err(|_| {
        anyhow::anyhow!(
            "Packages recipe not found at: {}\n\
             Expected packages.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    // Verify rocky.rhai has been run first
    let rootfs = downloads_dir.join("rootfs");
    let iso_contents = downloads_dir.join("iso-contents");

    if !rootfs.join("usr").exists() {
        bail!(
            "rootfs not found at: {}\n\
             Run rocky.rhai first (via rocky() function).",
            rootfs.display()
        );
    }

    if !iso_contents.join("BaseOS/Packages").exists() {
        bail!(
            "iso-contents not found at: {}\n\
             Run rocky.rhai first (via rocky() function).",
            iso_contents.display()
        );
    }

    // Find and run recipe
    let recipe_bin = find_recipe(&monorepo_dir)?;
    recipe_bin.run(&recipe_path, &downloads_dir)?;

    Ok(())
}

/// Run the epel.rhai recipe to download and extract EPEL packages into rootfs.
///
/// This must be called after `packages()` since it depends on the rootfs.
/// Downloads packages not available in Rocky 10 DVD: btrfs-progs, ntfs-3g,
/// screen, pv, ddrescue, testdisk.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn epel(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/epel.rhai");

    ensure_exists(&recipe_path, "EPEL recipe").map_err(|_| {
        anyhow::anyhow!(
            "EPEL recipe not found at: {}\n\
             Expected epel.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    // Verify rootfs exists
    let rootfs = downloads_dir.join("rootfs");
    if !rootfs.join("usr").exists() {
        bail!(
            "rootfs not found at: {}\n\
             Run rocky.rhai and packages.rhai first.",
            rootfs.display()
        );
    }

    // Find and run recipe
    let recipe_bin = find_recipe(&monorepo_dir)?;
    recipe_bin.run(&recipe_path, &downloads_dir)?;

    Ok(())
}
