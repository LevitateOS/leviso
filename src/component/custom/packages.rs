//! Package manager and bootloader operations.

use anyhow::{bail, Context, Result};
use std::fs;

use leviso_elf::{copy_dir_recursive, make_executable};

use crate::build::context::BuildContext;
use distro_builder::process::shell_in;

const RECIPE_CONF: &str = include_str!("../../../profile/etc/recipe.conf");
const RECIPE_SH: &str = include_str!("../../../profile/etc/profile.d/recipe.sh");
const DRACUT_CONF: &str = include_str!("../../../profile/etc/dracut.conf.d/levitate.conf");

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

    let recipe_path = if let Ok(env_path) = std::env::var("RECIPE_BINARY") {
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
        let recipe_path = manifest_dir.parent().unwrap().join("tools/recipe/target/release/recipe");

        if recipe_path.exists() {
            recipe_path
        } else {
            bail!(
                "recipe binary not found. LevitateOS REQUIRES the package manager.\n\
                 \n\
                 Build it:\n\
                   cd tools/recipe && cargo build --release\n\
                 \n\
                 Or set env var:\n\
                   export RECIPE_BINARY=/path/to/recipe/target/release/recipe\n\
                 \n\
                 DO NOT remove this check. An ISO without recipe is BROKEN."
            );
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

    fs::write(ctx.staging.join("etc/recipe/recipe.conf"), RECIPE_CONF)?;
    fs::write(ctx.staging.join("etc/profile.d/recipe.sh"), RECIPE_SH)?;

    println!("  Created recipe configuration");
    Ok(())
}

/// Create dracut configuration for LevitateOS.
pub fn create_dracut_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating dracut configuration...");

    let dracut_conf_dir = ctx.staging.join("etc/dracut.conf.d");
    fs::create_dir_all(&dracut_conf_dir)?;

    fs::write(dracut_conf_dir.join("levitate.conf"), DRACUT_CONF)?;

    println!("  Created /etc/dracut.conf.d/levitate.conf");
    Ok(())
}

/// Copy the docs-tui binary (levitate-docs).
///
/// This provides terminal-based documentation for the live ISO.
/// Built with: cd docs/tui && bun build --compile --minify --outfile levitate-docs src/index.tsx
pub fn copy_docs_tui(ctx: &BuildContext) -> Result<()> {
    println!("Copying docs-tui (levitate-docs)...");

    let docs_tui_path = if let Ok(env_path) = std::env::var("DOCS_TUI_BINARY") {
        let path = std::path::PathBuf::from(&env_path);
        if path.exists() {
            println!("  Using levitate-docs from DOCS_TUI_BINARY env var");
            path
        } else {
            bail!(
                "DOCS_TUI_BINARY points to non-existent path: {}\n\
                 Build it or update the env var.",
                env_path
            );
        }
    } else {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let docs_tui_path = manifest_dir.parent().unwrap().join("docs/tui/levitate-docs");

        if docs_tui_path.exists() {
            docs_tui_path
        } else {
            bail!(
                "levitate-docs binary not found at {}.\n\
                 \n\
                 Build it:\n\
                   cd docs/tui && bun build --compile --minify --outfile levitate-docs src/index.tsx\n\
                 \n\
                 Or set env var:\n\
                   export DOCS_TUI_BINARY=/path/to/levitate-docs\n\
                 \n\
                 The docs TUI shows installation instructions in the live ISO.",
                docs_tui_path.display()
            );
        }
    };

    let dest = ctx.staging.join("usr/bin/levitate-docs");
    fs::copy(&docs_tui_path, &dest)
        .with_context(|| format!("Failed to copy levitate-docs from {:?}", docs_tui_path))?;
    make_executable(&dest)?;

    // Copy glibc libraries required by the Bun-compiled binary
    // These may not be copied by other binaries (which use the Rocky rootfs binaries)
    use crate::build::libdeps::copy_library;
    let required_libs = ["libpthread.so.0", "libdl.so.2", "libm.so.6"];
    for lib in required_libs {
        if let Err(e) = copy_library(ctx, lib) {
            // Copy failed - but library might already exist in staging
            // Check lib64 and lib directories
            let lib64_path = ctx.staging.join("usr/lib64").join(lib);
            let lib_path = ctx.staging.join("usr/lib").join(lib);
            if lib64_path.exists() || lib_path.exists() {
                println!("  Note: {} copy skipped - already exists", lib);
            } else {
                // Library is REQUIRED and doesn't exist - this is a real error
                bail!(
                    "Required library '{}' not found and copy failed: {}\n\
                     The Bun binary (levitate-docs) requires this library.\n\
                     Check that the Rocky rootfs contains glibc.",
                    lib, e
                );
            }
        }
    }

    let size_mb = fs::metadata(&dest)?.len() as f64 / 1_000_000.0;
    println!("  Copied levitate-docs to /usr/bin/levitate-docs ({:.1} MB)", size_mb);
    Ok(())
}
