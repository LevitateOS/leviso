//! Recipe-based dependency resolution.
//!
//! This module provides a bridge between leviso and the recipe package manager's
//! execute capability. It allows dependency recipes in `deps/` to be executed
//! via the `recipe install` command.
//!
//! # Design Philosophy
//!
//! Recipes use phase separation: acquire() → build() → install().
//! This module calls `recipe install` which runs all phases via execute().
//! The output path is determined by convention (BUILD_DIR/rootfs for rocky).
//!
//! See: https://www.anthropic.com/research/emergent-misalignment-reward-hacking
//!
//! # Migration Path
//!
//! This is intended to eventually replace leviso-deps. The migration plan:
//! 1. Dependencies are defined as .rhai recipes in `deps/`
//! 2. Each recipe has acquire(), build(), install() phases
//! 3. leviso calls `recipe install <name>` to execute all phases
//! 4. Output path is determined by convention (e.g., BUILD_DIR/rootfs)
//! 5. Once proven reliable, leviso-deps can be removed
//!
//! # Example
//!
//! ```ignore
//! use leviso::resolve::resolve_dep;
//!
//! // This calls `recipe install rocky` which runs acquire → build → install
//! // Then returns BUILD_DIR/rootfs by convention
//! let rocky_path = resolve_dep("rocky")?;
//! ```

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve a dependency using the recipe package manager.
///
/// This executes `recipe install <name> -r deps -b <build_dir>` which runs
/// the recipe's acquire(), build(), and install() phases via execute().
/// The output path is determined by convention based on the dependency name.
///
/// # Arguments
/// * `name` - The dependency name (e.g., "rocky")
///
/// # Returns
/// The resolved path to the dependency output
pub fn resolve_dep(name: &str) -> Result<PathBuf> {
    // Get the path to the recipe binary
    let recipe_bin = find_recipe_binary()?;

    // Get the deps directory (relative to leviso crate root)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deps_dir = manifest_dir.join("deps");

    if !deps_dir.exists() {
        bail!(
            "Deps directory not found: {}\n\
             Create dependency recipes in leviso/deps/*.rhai",
            deps_dir.display()
        );
    }

    // Use a known build directory so we can find the output
    let build_dir = manifest_dir.join("downloads");
    std::fs::create_dir_all(&build_dir)
        .with_context(|| format!("Failed to create build directory: {}", build_dir.display()))?;

    // Run recipe install (which calls execute() → acquire → build → install)
    let output = Command::new(&recipe_bin)
        .args([
            "install",
            name,
            "-r",
            deps_dir.to_str().unwrap(),
            "-b",
            build_dir.to_str().unwrap(),
            // Prefix doesn't matter for dependency recipes, but is required
            "-p",
            build_dir.join("prefix").to_str().unwrap(),
        ])
        .current_dir(&manifest_dir)
        .output()
        .with_context(|| format!("Failed to run recipe install for {}", name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "Failed to execute recipe for {}:\nstderr: {}\nstdout: {}",
            name,
            stderr.trim(),
            stdout.trim()
        );
    }

    // Determine output path by convention
    let output_path = get_output_path_by_convention(name, &build_dir)?;

    // Verify the path exists
    if !output_path.exists() {
        bail!(
            "Dependency output not found at expected location: {}\n\
             The recipe may not have completed successfully.",
            output_path.display()
        );
    }

    Ok(output_path.canonicalize()?)
}

/// Get the expected output path for a dependency based on convention.
///
/// Each dependency type has a known output location:
/// - rocky: BUILD_DIR/rootfs (extracted Rocky Linux rootfs)
/// - linux: BUILD_DIR/linux-{version} (kernel source tree from tarball)
fn get_output_path_by_convention(name: &str, build_dir: &Path) -> Result<PathBuf> {
    match name {
        "rocky" => Ok(build_dir.join("rootfs")),
        "linux" => Ok(build_dir.join(distro_spec::levitate::KERNEL_SOURCE.source_dir_name())),
        _ => bail!(
            "Unknown dependency '{}'. Add output path convention to get_output_path_by_convention()",
            name
        ),
    }
}

/// Find the recipe binary.
fn find_recipe_binary() -> Result<PathBuf> {
    // Priority 1: RECIPE_BINARY env var
    if let Ok(path) = std::env::var("RECIPE_BINARY") {
        let path = PathBuf::from(&path);
        if path.exists() {
            return Ok(path);
        }
    }

    // Priority 2: Workspace target directory
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();

    // Try release build first
    let release_bin = workspace_root.join("target/release/recipe");
    if release_bin.exists() {
        return Ok(release_bin);
    }

    // Then debug build
    let debug_bin = workspace_root.join("target/debug/recipe");
    if debug_bin.exists() {
        return Ok(debug_bin);
    }

    // Priority 3: System PATH
    if let Ok(path) = which::which("recipe") {
        return Ok(path);
    }

    bail!(
        "recipe binary not found.\n\
         Build it with: cargo build -p levitate-recipe\n\
         Or set RECIPE_BINARY env var to the path."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_recipe_binary_exists() {
        // This test passes if a recipe binary exists anywhere in the search path
        // It may fail in CI if recipe isn't built yet - that's expected
        if let Ok(path) = find_recipe_binary() {
            assert!(path.exists());
        }
    }

    #[test]
    fn test_output_path_convention_rocky() {
        let build_dir = PathBuf::from("/tmp/test-build");
        let path = get_output_path_by_convention("rocky", &build_dir).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-build/rootfs"));
    }

    #[test]
    fn test_output_path_convention_linux() {
        let build_dir = PathBuf::from("/tmp/test-build");
        let path = get_output_path_by_convention("linux", &build_dir).unwrap();
        let expected = format!("/tmp/test-build/{}", distro_spec::levitate::KERNEL_SOURCE.source_dir_name());
        assert_eq!(path, PathBuf::from(expected));
    }

    #[test]
    fn test_output_path_convention_unknown() {
        let build_dir = PathBuf::from("/tmp/test-build");
        let result = get_output_path_by_convention("unknown", &build_dir);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown dependency"));
    }
}
