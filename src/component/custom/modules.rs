//! Kernel module operations - copying, depmod.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::build::context::BuildContext;
use distro_builder::process::Cmd;

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
///
/// For CUSTOM kernels: Copies the ENTIRE modules directory (all available modules).
/// For ROCKY kernels: Copies only modules from config (legacy behavior).
///
/// This distinction matters because:
/// - Custom kernel modules are built specifically for LevitateOS
/// - Squashfs needs ALL modules for complete hardware support
/// - Config list was designed for Rocky where we cherry-pick modules
pub fn copy_modules(ctx: &BuildContext) -> Result<()> {
    println!("Setting up kernel modules...");

    // PRIORITY: Use custom kernel modules if available (from kernel build)
    // Custom modules are installed to output/staging/usr/lib/modules/ during kernel build
    let custom_modules_base = ctx.output.join("staging/usr/lib/modules");
    let rocky_modules_base = ctx.source.join("usr/lib/modules");

    let (modules_base, is_custom_kernel) = if custom_modules_base.exists()
        && std::fs::read_dir(&custom_modules_base)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        println!(
            "  Using CUSTOM kernel modules from {}",
            custom_modules_base.display()
        );
        (custom_modules_base, true)
    } else {
        println!(
            "  Using ROCKY kernel modules from {}",
            rocky_modules_base.display()
        );
        (rocky_modules_base, false)
    };

    let kernel_version = find_kernel_version(&modules_base)?;
    println!("  Kernel version: {}", kernel_version);

    let src_modules = modules_base.join(&kernel_version);
    let dst_modules = ctx.staging.join("lib/modules").join(&kernel_version);
    fs::create_dir_all(&dst_modules)?;

    if is_custom_kernel {
        // Custom kernel: Copy ENTIRE modules directory (all 249+ modules)
        // The EROFS rootfs needs all modules for complete hardware support
        copy_modules_recursive(&src_modules, &dst_modules)?;
    } else {
        // Rocky kernel: Copy only modules from config (legacy behavior)
        copy_modules_from_config(ctx, &src_modules, &dst_modules)?;
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

/// Copy entire modules directory recursively (for custom kernels).
///
/// This preserves the kernel/ subdirectory structure which modprobe needs.
fn copy_modules_recursive(src: &Path, dst: &Path) -> Result<()> {
    let mut module_count = 0;

    // Copy the kernel/ subdirectory which contains all modules
    let kernel_src = src.join("kernel");
    let kernel_dst = dst.join("kernel");

    if kernel_src.exists() {
        copy_dir_recursive(&kernel_src, &kernel_dst, &mut module_count)?;
    }

    println!("  Copied {} kernel modules", module_count);
    Ok(())
}

/// Recursively copy a directory, counting .ko files.
fn copy_dir_recursive(src: &Path, dst: &Path, count: &mut usize) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path, count)?;
        } else {
            fs::copy(&path, &dest_path)?;
            if path.extension().map(|e| e == "ko").unwrap_or(false) {
                *count += 1;
            }
        }
    }

    Ok(())
}

/// Copy modules from config list (for Rocky kernels - legacy behavior).
fn copy_modules_from_config(
    _ctx: &BuildContext,
    src_modules: &Path,
    dst_modules: &Path,
) -> Result<()> {
    let config = crate::config::Config::load();
    let modules = config.all_modules();

    let mut missing = Vec::new();
    for module_path in &modules {
        // Try to find the module with different extensions
        let base_path = module_path
            .trim_end_matches(".ko.xz")
            .trim_end_matches(".ko.gz")
            .trim_end_matches(".ko");

        let mut found = false;
        for ext in [".ko", ".ko.xz", ".ko.gz"] {
            let full_module_path = format!("{}{}", base_path, ext);
            let src = src_modules.join(&full_module_path);
            if src.exists() {
                let module_name = Path::new(&full_module_path)
                    .file_name()
                    .context("Invalid module path")?;
                let dst = dst_modules.join(module_name);
                fs::copy(&src, &dst)?;
                println!("  Copied {}", module_name.to_string_lossy());
                found = true;
                break;
            }
        }

        if !found {
            missing.push(module_path.to_string());
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

    Ok(())
}
