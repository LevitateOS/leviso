//! Package manager and bootloader operations.

use anyhow::{bail, Context, Result};
use std::fs;

use leviso_elf::{copy_dir_recursive, make_executable};

use crate::build::context::BuildContext;
use crate::process::shell_in;

/// Copy dracut modules from source to staging.
pub fn copy_dracut_modules(ctx: &BuildContext) -> Result<()> {
    let dracut_src = ctx.source.join("usr/lib/dracut");
    let dracut_dst = ctx.staging.join("usr/lib/dracut");

    if dracut_src.exists() {
        fs::create_dir_all(dracut_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&dracut_src, &dracut_dst)?;
        println!(
            "  Copied dracut modules ({:.1} MB)",
            size as f64 / 1_000_000.0
        );
    } else {
        bail!(
            "Dracut modules not found at {}.\n\
             Dracut is REQUIRED - it generates the initramfs during installation.",
            dracut_src.display()
        );
    }

    Ok(())
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
pub fn copy_recipe(ctx: &BuildContext) -> Result<()> {
    println!("Copying recipe package manager...");

    let recipe_path = match &ctx.recipe_binary {
        Some(path) => {
            if !path.exists() {
                bail!(
                    "Recipe binary explicitly specified but not found at: {}\n\
                     Build it with: cd recipe && cargo build --release",
                    path.display()
                );
            }
            path.clone()
        }
        None => {
            if let Ok(env_path) = std::env::var("RECIPE_BINARY") {
                let path = std::path::PathBuf::from(&env_path);
                if path.exists() {
                    println!("  Using recipe from RECIPE_BINARY env var");
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
                let search_paths = [
                    // Monorepo layout: ../tools/recipe/
                    manifest_dir.parent().unwrap().join("tools/recipe/target/release/recipe"),
                    // Legacy layout: ../recipe/
                    manifest_dir.parent().unwrap().join("recipe/target/release/recipe"),
                ];

                match search_paths.iter().find(|p| p.exists()) {
                    Some(path) => path.clone(),
                    None => {
                        bail!(
                            "recipe binary not found. LevitateOS REQUIRES the package manager.\n\
                             \n\
                             For monorepo users:\n\
                               cd ../recipe && cargo build --release\n\
                             \n\
                             For standalone users:\n\
                               1. Clone recipe: git clone https://github.com/LevitateOS/recipe\n\
                               2. Build it: cd recipe && cargo build --release\n\
                               3. Set env var: export RECIPE_BINARY=/path/to/recipe/target/release/recipe\n\
                             \n\
                             DO NOT remove this check. An ISO without recipe is BROKEN."
                        );
                    }
                }
            }
        }
    };

    let dest = ctx.staging.join("usr/bin/recipe");
    fs::copy(&recipe_path, &dest)
        .with_context(|| format!("Failed to copy recipe from {:?}", recipe_path))?;
    make_executable(&dest)?;

    println!("  Copied recipe to /usr/bin/recipe");
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

    fs::write(
        ctx.staging.join("etc/recipe/recipe.conf"),
        r#"# Recipe package manager configuration
recipe_path = "/etc/recipe/repos/rocky10"
cache_dir = "/var/cache/recipe"
db_dir = "/var/lib/recipe"
"#,
    )?;

    fs::write(
        ctx.staging.join("etc/profile.d/recipe.sh"),
        "export RECIPE_PATH=/etc/recipe/repos/rocky10\n",
    )?;

    println!("  Created recipe configuration");
    Ok(())
}

/// Create dracut configuration for LevitateOS.
pub fn create_dracut_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating dracut configuration...");

    let dracut_conf_dir = ctx.staging.join("etc/dracut.conf.d");
    fs::create_dir_all(&dracut_conf_dir)?;

    fs::write(
        dracut_conf_dir.join("levitate.conf"),
        r#"# LevitateOS dracut defaults
add_drivers+=" ext4 vfat "
hostonly="no"
add_dracutmodules+=" base rootfs-block "
compress="gzip"
"#,
    )?;

    println!("  Created /etc/dracut.conf.d/levitate.conf");
    Ok(())
}
