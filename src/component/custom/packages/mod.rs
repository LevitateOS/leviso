//! Package manager and bootloader operations.
//!
//! NOTE: Dracut has been removed from LevitateOS. Initramfs is now built using
//! a custom rootless builder. See TEAM_125 and .teams/KNOWLEDGE_no-dracut.md.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use leviso_elf::{copy_dir_recursive, make_executable};

use crate::build::context::BuildContext;
use distro_builder::process::shell_in;

/// Read a file from the colocated files directory (no relative path traversal)
fn read_profile_file(_ctx: &BuildContext, path: &str) -> Result<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Navigate from leviso/ root to this module's files directory
    let file_path = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("leviso/src/component/custom/packages/files"))
        .ok_or_else(|| anyhow::anyhow!("Failed to compute file path"))?
        .join(path);
    fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read packages file from {}", file_path.display()))
}

/// Extract and copy systemd-boot EFI files from RPM.
pub fn copy_systemd_boot_efi(ctx: &BuildContext) -> Result<()> {
    let efi_dst = ctx.staging.join("usr/lib/systemd/boot/efi");

    let rpm_dir = ctx
        .base_dir
        .join("downloads/iso-contents/AppStream/Packages/s");
    let rpm_pattern = "systemd-boot-unsigned";

    let rpm_path = std::fs::read_dir(&rpm_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains(rpm_pattern))
                .unwrap_or(false)
        });

    let Some(rpm_path) = rpm_path else {
        bail!(
            "systemd-boot-unsigned RPM not found in {}.\n\
             The EFI files from this package are REQUIRED for bootctl install.",
            rpm_dir.display()
        );
    };

    let temp_dir = ctx.base_dir.join("output/.systemd-boot-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let cmd = format!("rpm2cpio '{}' | cpio -idm", rpm_path.display());
    shell_in(&cmd, &temp_dir)
        .with_context(|| format!("Failed to extract RPM: {}", rpm_path.display()))?;

    let efi_src = temp_dir.join("usr/lib/systemd/boot/efi");
    if efi_src.exists() {
        fs::create_dir_all(efi_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&efi_src, &efi_dst)?;
        println!(
            "  Copied systemd-boot EFI files ({:.1} KB)",
            size as f64 / 1_000.0
        );
    } else {
        bail!("EFI files not found in extracted RPM at {}", temp_dir.display());
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Copy keymaps for keyboard layout support.
pub fn copy_keymaps(ctx: &BuildContext) -> Result<()> {
    let keymaps_src = ctx.source.join("usr/lib/kbd/keymaps");
    let keymaps_dst = ctx.staging.join("usr/lib/kbd/keymaps");

    if keymaps_src.exists() {
        fs::create_dir_all(keymaps_dst.parent().unwrap())?;
        copy_dir_recursive(&keymaps_src, &keymaps_dst)?;
        println!("  Copied keymaps for keyboard layout support");
    } else {
        bail!(
            "Keymaps not found at {}.\n\
             Keymaps are REQUIRED for keyboard layout support (loadkeys).",
            keymaps_src.display()
        );
    }

    Ok(())
}

/// Copy the recipe package manager binary.
///
/// AUTOMATICALLY REBUILDS recipe before copying to ensure latest version.
pub fn copy_recipe(ctx: &BuildContext) -> Result<()> {
    use std::process::Command;

    println!("Building and copying recipe package manager...");

    let recipe_path = if let Ok(env_path) = std::env::var("RECIPE_BINARY") {
        let path = std::path::PathBuf::from(&env_path);
        if path.exists() {
            println!("  Using recipe from RECIPE_BINARY env var (skipping rebuild)");
            path
        } else {
            bail!(
                "RECIPE_BINARY points to non-existent path: {}\n\
                 Build it or update the env var.",
                env_path
            );
        }
    } else {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let monorepo_dir = manifest_dir.parent().unwrap();
        let recipe_path = monorepo_dir.join("target/release/recipe");

        // ALWAYS rebuild recipe to ensure latest version
        println!("  Rebuilding recipe...");
        let status = Command::new("cargo")
            .args(["build", "--release", "-p", "levitate-recipe"])
            .current_dir(monorepo_dir)
            .status()
            .context("Failed to run cargo build for recipe")?;

        if !status.success() {
            bail!(
                "Failed to build recipe. Check cargo output above.\n\
                 \n\
                 recipe is REQUIRED for the live ISO."
            );
        }

        if recipe_path.exists() {
            recipe_path
        } else {
            // Fallback to crate-local target
            let local_path = monorepo_dir.join("tools/recipe/target/release/recipe");
            if local_path.exists() {
                local_path
            } else {
                bail!(
                    "recipe binary not found after rebuild. This is a bug.\n\
                     Check that tools/recipe/Cargo.toml exists and compiles."
                );
            }
        }
    };

    let dest = ctx.staging.join("usr/bin/recipe");
    fs::copy(&recipe_path, &dest)
        .with_context(|| format!("Failed to copy recipe from {:?}", recipe_path))?;
    make_executable(&dest)?;

    println!("  Installed recipe to /usr/bin/recipe");
    Ok(())
}

/// Set up recipe package manager configuration.
pub fn setup_recipe_config(ctx: &BuildContext) -> Result<()> {
    println!("Setting up recipe configuration...");

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

    fs::write(ctx.staging.join("etc/recipe/recipe.conf"), read_profile_file(ctx, "recipe.conf")?)?;
    fs::write(ctx.staging.join("etc/profile.d/recipe.sh"), read_profile_file(ctx, "recipe.sh")?)?;

    println!("  Created recipe configuration");
    Ok(())
}


