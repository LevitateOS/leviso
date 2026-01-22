//! Kernel module setup.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use super::context::BuildContext;

/// Module metadata files needed by modprobe.
const MODULE_METADATA_FILES: &[&str] = &[
    "modules.dep",
    "modules.dep.bin",
    "modules.alias",
    "modules.alias.bin",
    "modules.softdep",
    "modules.symbols",
    "modules.symbols.bin",
    "modules.builtin",
    "modules.builtin.bin",
    "modules.builtin.modinfo",
    "modules.order",
];

/// Set up kernel modules.
pub fn setup_modules(ctx: &BuildContext, modules: &[&str]) -> Result<()> {
    println!("Setting up kernel modules...");

    let modules_base = ctx.source.join("usr/lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;
    println!("  Kernel version: {}", kernel_version);

    let src_modules = modules_base.join(&kernel_version);
    let dst_modules = ctx.staging.join("lib/modules").join(&kernel_version);
    fs::create_dir_all(&dst_modules)?;

    // Copy specified modules - ALL specified modules are REQUIRED
    // If a module is in the list, it's because it's needed. Missing = fail.
    let mut missing = Vec::new();
    for module in modules {
        let src = src_modules.join(module);
        if src.exists() {
            let module_name = Path::new(module)
                .file_name()
                .context("Invalid module path")?;
            let dst = dst_modules.join(module_name);
            fs::copy(&src, &dst)?;
            println!("  Copied {}", module_name.to_string_lossy());
        } else {
            missing.push(*module);
        }
    }

    // FAIL FAST if any specified module is missing
    // If a module is in the config, it's REQUIRED. No "optional" modules here.
    if !missing.is_empty() {
        bail!(
            "Required kernel modules not found: {:?}\n\
             \n\
             These modules were specified in the configuration.\n\
             If a module is in the config, it's REQUIRED for hardware support.\n\
             \n\
             Either:\n\
             1. Remove the module from config if it's truly optional\n\
             2. Fix the rootfs to include the module\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            missing
        );
    }

    // Copy module metadata files
    println!("  Copying module metadata for modprobe...");
    for metadata_file in MODULE_METADATA_FILES {
        let src = src_modules.join(metadata_file);
        if src.exists() {
            fs::copy(&src, dst_modules.join(metadata_file))?;
        }
    }

    // Run depmod - REQUIRED for modprobe to work
    // Without depmod, modprobe cannot resolve module dependencies.
    println!("  Running depmod...");
    let depmod_status = std::process::Command::new("depmod")
        .args([
            "-a",
            "-b",
            ctx.staging.to_str().unwrap(),
            &kernel_version,
        ])
        .status();

    match depmod_status {
        Ok(status) if status.success() => println!("  depmod completed successfully"),
        Ok(status) => {
            // FAIL FAST - depmod failure means modprobe won't work
            bail!(
                "depmod failed with exit code {}.\n\
                 \n\
                 depmod generates module dependency information.\n\
                 Without it, modprobe cannot load modules.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                status
            );
        }
        Err(e) => {
            // FAIL FAST - depmod not found means the build environment is broken
            bail!(
                "Could not run depmod: {}\n\
                 \n\
                 depmod is REQUIRED to generate module dependencies.\n\
                 Install it: sudo dnf install kmod\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                e
            );
        }
    }

    Ok(())
}

fn find_kernel_version(modules_base: &Path) -> Result<String> {
    for entry in fs::read_dir(modules_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains('.') && entry.path().is_dir() {
            return Ok(name_str.to_string());
        }
    }
    anyhow::bail!("Could not find kernel modules directory")
}
