//! Recipe package manager integration.
//!
//! Copies the recipe binary into the squashfs.

use anyhow::{bail, Context, Result};
use std::fs;

use super::libdeps::make_executable;
use super::context::BuildContext;

/// Copy recipe binary to the staging directory.
pub fn copy_recipe(ctx: &BuildContext) -> Result<()> {
    println!("Copying recipe package manager...");

    // Check if recipe binary path is configured
    let recipe_path = match &ctx.recipe_binary {
        Some(path) => {
            // Explicitly provided path - MUST exist
            if !path.exists() {
                anyhow::bail!(
                    "Recipe binary explicitly specified but not found at: {}\n\
                     Build it with: cd recipe && cargo build --release",
                    path.display()
                );
            }
            path.clone()
        }
        None => {
            // No path specified - try to find in common locations
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let search_paths = [
                manifest_dir.parent().unwrap().join("recipe/target/release/recipe"),
                manifest_dir.join("../recipe/target/release/recipe"),
            ];

            match search_paths.iter().find(|p| p.exists()) {
                Some(path) => path.clone(),
                None => {
                    bail!(
                        "recipe binary not found. LevitateOS REQUIRES the package manager.\n\
                         \n\
                         Searched:\n\
                         {}\n\
                         \n\
                         Build recipe first: cd ../recipe && cargo build --release\n\
                         \n\
                         DO NOT remove this check. An ISO without recipe is BROKEN.",
                        search_paths.iter().map(|p| format!("  - {}", p.display())).collect::<Vec<_>>().join("\n")
                    );
                }
            }
        }
    };

    // Copy to /usr/bin/recipe
    let dest = ctx.staging.join("usr/bin/recipe");
    fs::copy(&recipe_path, &dest)
        .with_context(|| format!("Failed to copy recipe from {:?}", recipe_path))?;
    make_executable(&dest)?;

    println!("  Copied recipe to /usr/bin/recipe");
    Ok(())
}

/// Create recipe configuration directory.
pub fn setup_recipe_config(ctx: &BuildContext) -> Result<()> {
    println!("Setting up recipe configuration...");

    // Create recipe directories
    let recipe_dirs = [
        "etc/recipe",
        "etc/recipe/repos",
        "etc/recipe/repos/rocky10",
        "var/lib/recipe",
        "var/cache/recipe",
    ];

    for dir in recipe_dirs {
        fs::create_dir_all(ctx.staging.join(dir))?;
    }

    // Create basic recipe configuration
    fs::write(
        ctx.staging.join("etc/recipe/recipe.conf"),
        r#"# Recipe package manager configuration

# Repository URL (set during installation)
# repository = "https://packages.levitateos.org"

# Recipe path (system recipes)
recipe_path = "/etc/recipe/repos/rocky10"

# Cache directory
cache_dir = "/var/cache/recipe"

# Database directory
db_dir = "/var/lib/recipe"
"#,
    )?;

    // Create profile.d script for RECIPE_PATH
    fs::write(
        ctx.staging.join("etc/profile.d/recipe.sh"),
        r#"# Recipe package manager environment
export RECIPE_PATH=/etc/recipe/repos/rocky10
"#,
    )?;

    println!("  Created recipe configuration");
    Ok(())
}
