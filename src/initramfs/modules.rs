//! Kernel module setup.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::context::BuildContext;

/// Essential kernel modules for disk access and filesystems.
/// Order matters: dependencies must come before modules that need them.
const ESSENTIAL_MODULES: &[&str] = &[
    // Block device driver
    "kernel/drivers/block/virtio_blk.ko.xz",
    // ext4 filesystem and dependencies
    "kernel/fs/mbcache.ko.xz",
    "kernel/fs/jbd2/jbd2.ko.xz",
    "kernel/fs/ext4/ext4.ko.xz",
    // FAT/vfat filesystem for EFI partition
    "kernel/fs/fat/fat.ko.xz",
    "kernel/fs/fat/vfat.ko.xz",
];

/// Set up kernel modules in initramfs.
pub fn setup_modules(ctx: &BuildContext) -> Result<()> {
    println!("Setting up kernel modules...");

    // Find kernel version
    let modules_base = ctx.rootfs.join("usr/lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;
    println!("  Kernel version: {}", kernel_version);

    let src_modules = modules_base.join(&kernel_version);
    let dst_modules = ctx.initramfs.join("lib/modules").join(&kernel_version);
    fs::create_dir_all(&dst_modules)?;

    // Copy essential modules
    for module in ESSENTIAL_MODULES {
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

    // Copy modules.dep (needed for modprobe, but we use insmod directly)
    let moddep_src = src_modules.join("modules.dep");
    if moddep_src.exists() {
        fs::copy(&moddep_src, dst_modules.join("modules.dep"))?;
    }

    Ok(())
}

/// Find the kernel version directory.
fn find_kernel_version(modules_base: &Path) -> Result<String> {
    for entry in fs::read_dir(modules_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip non-version entries
        if name_str.contains('.') && entry.path().is_dir() {
            return Ok(name_str.to_string());
        }
    }
    anyhow::bail!("Could not find kernel modules directory")
}

