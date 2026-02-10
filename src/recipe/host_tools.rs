//! Host build tool dependencies via recipe.
//!
//! Ensures tools like mkfs.erofs, xorriso, mkfs.fat, mtools, and ukify
//! are available before building. Built from source by leviso-deps.rhai.

use super::find_recipe;
use anyhow::Result;
use distro_builder::process::ensure_exists;
use std::path::Path;

/// Ensure all host build tools are available.
///
/// Runs the host-tools.rhai recipe which depends on leviso-deps.rhai
/// to build all required tools from source if not already present.
/// After running, prepends the tools prefix to PATH so subsequent
/// build steps can find the tools.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn ensure_host_tools(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/host-tools.rhai");

    ensure_exists(&recipe_path, "Host tools recipe").map_err(|_| {
        anyhow::anyhow!(
            "Host tools recipe not found at: {}\n\
             Expected host-tools.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    let recipes_dir = monorepo_dir.join("distro-builder/recipes");
    let recipe_bin = find_recipe(&monorepo_dir)?;

    // Run with search path so it can find leviso-deps.rhai
    recipe_bin.run_with_recipes_path(&recipe_path, &downloads_dir, Some(&recipes_dir))?;

    // Prepend the tools prefix to PATH so subsequent build steps find them.
    // The recipe installs tools to BUILD_DIR/.deps/leviso-deps/.tools/bin/
    let tools_bin = downloads_dir.join(".deps/leviso-deps/.tools/bin");
    if tools_bin.exists() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        if !current_path.contains(&tools_bin.to_string_lossy().to_string()) {
            // Safety: leviso build is single-threaded at this point
            unsafe {
                std::env::set_var("PATH", format!("{}:{}", tools_bin.display(), current_path));
            }
            println!("  Added {} to PATH", tools_bin.display());
        }
    }

    Ok(())
}
