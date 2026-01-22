//! Kernel module setup.

use anyhow::{Context, Result};
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

    // Copy specified modules
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
            println!("  Warning: {} not found", module);
        }
    }

    // Copy module metadata files
    println!("  Copying module metadata for modprobe...");
    for metadata_file in MODULE_METADATA_FILES {
        let src = src_modules.join(metadata_file);
        if src.exists() {
            fs::copy(&src, dst_modules.join(metadata_file))?;
        }
    }

    // Run depmod
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
        Ok(status) => println!("  Warning: depmod exited with status {}", status),
        Err(e) => println!("  Warning: Could not run depmod: {}", e),
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
