//! Dependency checks (Rocky ISO, recipe tools, etc).

use std::path::Path;

use anyhow::Result;
use distro_spec::shared::LEVITATE_CARGO_TOOLS;

use crate::recipe;

use super::types::CheckResult;
use super::validators::{validate_executable, validate_rocky_iso_size};

/// Check all build dependencies.
pub fn check_dependencies(base_dir: &Path) -> Result<Vec<CheckResult>> {
    let mut results = Vec::new();

    // Linux source
    if recipe::has_linux_source(base_dir) {
        results.push(CheckResult::pass_with(
            "Linux source",
            "Found (submodule or downloaded)",
        ));
    } else {
        results.push(CheckResult::warn(
            "Linux source",
            "Not found - will be downloaded on first build",
        ));
    }

    // Rocky (via recipe) - check known paths
    // Recipe puts artifacts at: downloads/Rocky-*.iso, downloads/rootfs, downloads/iso-contents
    let iso_path = base_dir.join("downloads/Rocky-10.1-x86_64-dvd1.iso");
    let rootfs_path = base_dir.join("downloads/rootfs");
    let iso_contents_path = base_dir.join("downloads/iso-contents");

    if iso_path.exists() && rootfs_path.exists() && iso_contents_path.exists() {
        // Validate ISO size (anti-cheat)
        match validate_rocky_iso_size(base_dir) {
            Ok(size_gb) => {
                results.push(CheckResult::pass_with(
                    "Rocky (recipe)",
                    &format!("ISO {:.1}GB + rootfs + iso-contents", size_gb),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "Rocky (recipe)",
                    &format!("ISO invalid: {} - run 'leviso download rocky'", e),
                ));
            }
        }
    } else if iso_path.exists() {
        // ISO exists but not fully extracted
        results.push(CheckResult::warn(
            "Rocky (recipe)",
            "ISO found but not extracted - run 'leviso download rocky'",
        ));
    } else {
        results.push(CheckResult::warn(
            "Rocky (recipe)",
            "Not found - run 'leviso download rocky' (8.6GB download)",
        ));
    }

    // Installation tools (recstrap, recfstab, recchroot)
    // Check if installed in staging (placed there by recipes)
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let staging_bin = output_dir.join("staging/usr/bin");
    for tool in LEVITATE_CARGO_TOOLS {
        let path = staging_bin.join(tool);
        if path.exists() {
            results.push(CheckResult::pass_with(
                tool,
                &format!("Installed: {}", path.display()),
            ));
        } else {
            // Check if recipe exists
            let recipe_path = base_dir.join(format!("deps/{}.rhai", tool));
            if recipe_path.exists() {
                results.push(CheckResult::warn(
                    tool,
                    "Not installed - will be built via recipe during build",
                ));
            } else {
                results.push(CheckResult::fail(
                    tool,
                    &format!(
                        "Not installed and recipe missing: {}",
                        recipe_path.display()
                    ),
                ));
            }
        }
    }

    // Recipe binary - use the same resolution logic as recipe.rs
    // Check: PATH, workspace binary, RECIPE_BIN env var
    let monorepo_dir = base_dir.parent().unwrap_or(base_dir);

    // 1. Check system PATH
    let recipe_in_path = which::which("recipe").ok();

    // 2. Check workspace binary (debug and release)
    let workspace_debug = monorepo_dir.join("target/debug/recipe");
    let workspace_release = monorepo_dir.join("target/release/recipe");

    // 3. Check RECIPE_BIN env var
    let recipe_env = std::env::var("RECIPE_BIN")
        .ok()
        .map(std::path::PathBuf::from);

    // 4. Check if source exists (can be built)
    let recipe_source = monorepo_dir.join("tools/recipe/Cargo.toml");

    // Find first available binary
    let recipe_binary = recipe_in_path
        .or_else(|| {
            if workspace_debug.exists() {
                Some(workspace_debug.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            if workspace_release.exists() {
                Some(workspace_release.clone())
            } else {
                None
            }
        })
        .or_else(|| recipe_env.filter(|p| p.exists()));

    match recipe_binary {
        Some(path) => match validate_executable(&path, "recipe") {
            Ok(version) => {
                results.push(CheckResult::pass_with(
                    "recipe",
                    &format!("{} ({})", path.display(), version),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "recipe",
                    &format!("{}: {}", path.display(), e),
                ));
            }
        },
        None if recipe_source.exists() => {
            // Source exists but not built - this is OK, it will be built on demand
            results.push(CheckResult::warn(
                "recipe",
                &format!(
                    "Source found at {}, will be built on first use",
                    monorepo_dir.join("tools/recipe").display()
                ),
            ));
        }
        None => {
            results.push(CheckResult::fail(
                "recipe",
                "Not found. Install to PATH or set RECIPE_BIN=/path/to/recipe",
            ));
        }
    }

    Ok(results)
}
