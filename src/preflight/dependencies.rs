//! Dependency checks (Rocky ISO, recipe tools, etc).

use std::path::Path;

use anyhow::Result;
use leviso_deps::DependencyResolver;

use super::types::CheckResult;
use super::validators::{validate_executable, validate_rocky_iso_size};

/// Check all build dependencies.
pub fn check_dependencies(base_dir: &Path) -> Result<Vec<CheckResult>> {
    let mut results = Vec::new();
    let resolver = DependencyResolver::new(base_dir)?;

    // Linux source
    if resolver.has_linux() {
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

    // Rocky ISO - ANTI-CHEAT: verify size, not just existence
    // A partial curl download passes .exists() but fails the build later.
    // This is the "sys.exit(0)" equivalent - satisfying the literal check
    // while violating its spirit. See: anthropic.com/research/emergent-misalignment-reward-hacking
    if resolver.has_rocky_iso() {
        match validate_rocky_iso_size(base_dir) {
            Ok(size_gb) => {
                results.push(CheckResult::pass_with(
                    "Rocky ISO",
                    &format!("Found, {:.1}GB (complete)", size_gb),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "Rocky ISO",
                    &format!("Found but invalid: {} - delete and re-download", e),
                ));
            }
        }
    } else {
        results.push(CheckResult::warn(
            "Rocky ISO",
            "Not found - will download 8.6GB on first build",
        ));
    }

    // Installation tools (recstrap, recfstab, recchroot)
    // Try to resolve each one
    match resolver.recstrap() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recstrap",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recstrap",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    match resolver.recfstab() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recfstab",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recfstab",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    match resolver.recchroot() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recchroot",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recchroot",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    // Recipe binary
    // Check env var first, then fall back to default location
    let recipe_binary = match std::env::var("RECIPE_BINARY") {
        Ok(path_str) => {
            let path = std::path::PathBuf::from(&path_str);
            // Warn if env var is set but path doesn't exist (user error)
            if !path.exists() {
                eprintln!(
                    "  [WARN] RECIPE_BINARY env var set to {} but file does not exist",
                    path_str
                );
            }
            Some(path)
        }
        Err(std::env::VarError::NotPresent) => {
            // Env var not set - use default location
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            manifest_dir
                .parent()
                .map(|p| p.join("recipe/target/release/recipe"))
                .filter(|p| p.exists())
        }
        Err(std::env::VarError::NotUnicode(s)) => {
            // Env var exists but invalid Unicode - warn user
            eprintln!(
                "  [WARN] RECIPE_BINARY env var contains invalid Unicode: {:?}",
                s
            );
            None
        }
    };

    // ANTI-CHEAT: verify recipe binary is executable, not just exists
    match recipe_binary {
        Some(path) if path.exists() => {
            match validate_executable(&path, "recipe") {
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
            }
        }
        Some(path) => {
            results.push(CheckResult::fail(
                "recipe",
                &format!("Path set but not found: {}", path.display()),
            ));
        }
        None => {
            results.push(CheckResult::fail(
                "recipe",
                "Not found. Build with: cd ../recipe && cargo build --release",
            ));
        }
    }

    Ok(results)
}
