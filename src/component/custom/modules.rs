//! Kernel module operations - copying, depmod.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::build::context::BuildContext;
use crate::process::Cmd;

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

/// Copy kernel modules from source to staging.
pub fn copy_modules(ctx: &BuildContext) -> Result<()> {
    println!("Setting up kernel modules...");

    let config = crate::config::Config::load();
    let modules = config.all_modules();

    let modules_base = ctx.source.join("usr/lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;
    println!("  Kernel version: {}", kernel_version);

    let src_modules = modules_base.join(&kernel_version);
    let dst_modules = ctx.staging.join("lib/modules").join(&kernel_version);
    fs::create_dir_all(&dst_modules)?;

    // Copy specified modules - ALL specified modules are REQUIRED
    let mut missing = Vec::new();
    for module in &modules {
        let src = src_modules.join(module);
        if src.exists() {
            let module_name = Path::new(module)
                .file_name()
                .context("Invalid module path")?;
            let dst = dst_modules.join(module_name);
            fs::copy(&src, &dst)?;
            println!("  Copied {}", module_name.to_string_lossy());
        } else {
            missing.push(module.to_string());
        }
    }

    // FAIL FAST if any specified module is missing
    if !missing.is_empty() {
        bail!(
            "Required kernel modules not found: {:?}\n\
             \n\
             These modules were specified in the configuration.\n\
             If a module is in the config, it's REQUIRED for hardware support.\n\
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

    // Run depmod
    println!("  Running depmod...");
    Cmd::new("depmod")
        .args(["-a", "-b"])
        .arg_path(&ctx.staging)
        .arg(&kernel_version)
        .error_msg("depmod failed. Install: sudo dnf install kmod")
        .run()?;
    println!("  depmod completed successfully");

    Ok(())
}

/// Run depmod to regenerate module dependencies.
pub fn run_depmod(ctx: &BuildContext) -> Result<()> {
    let modules_base = ctx.staging.join("lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;

    Cmd::new("depmod")
        .args(["-a", "-b"])
        .arg_path(&ctx.staging)
        .arg(&kernel_version)
        .error_msg("depmod failed. Install: sudo dnf install kmod")
        .run()?;

    Ok(())
}

/// Find the kernel version from the modules directory.
pub fn find_kernel_version(modules_base: &Path) -> Result<String> {
    for entry in fs::read_dir(modules_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains('.') && entry.path().is_dir() {
            return Ok(name_str.to_string());
        }
    }
    bail!("Could not find kernel modules directory")
}
