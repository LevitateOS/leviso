//! Kernel module setup.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::context::BuildContext;

/// Essential kernel modules for disk access and filesystems.
/// With modprobe, order no longer matters - dependencies are resolved automatically.
const ESSENTIAL_MODULES: &[&str] = &[
    // Block device driver (for virtual disks)
    "kernel/drivers/block/virtio_blk.ko.xz",
    // ext4 filesystem and dependencies (modprobe handles ordering)
    "kernel/fs/mbcache.ko.xz",
    "kernel/fs/jbd2/jbd2.ko.xz",
    "kernel/fs/ext4/ext4.ko.xz",
    // FAT/vfat filesystem for EFI partition
    "kernel/fs/fat/fat.ko.xz",
    "kernel/fs/fat/vfat.ko.xz",
    // SCSI/CD-ROM support (for installation media access)
    "kernel/drivers/scsi/virtio_scsi.ko.xz",
    "kernel/drivers/cdrom/cdrom.ko.xz",
    "kernel/drivers/scsi/sr_mod.ko.xz",
    // ISO 9660 filesystem (to mount installation media)
    "kernel/fs/isofs/isofs.ko.xz",
];

/// Module metadata files needed by modprobe for dependency resolution.
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

    // Copy module metadata files (required for modprobe dependency resolution)
    println!("  Copying module metadata for modprobe...");
    for metadata_file in MODULE_METADATA_FILES {
        let src = src_modules.join(metadata_file);
        if src.exists() {
            fs::copy(&src, dst_modules.join(metadata_file))?;
        }
    }

    // Run depmod to regenerate dependency info for our subset of modules
    // This creates a modules.dep specific to the modules we copied
    println!("  Running depmod to generate dependency info...");
    let depmod_status = std::process::Command::new("depmod")
        .args([
            "-a",
            "-b",
            ctx.initramfs.to_str().unwrap(),
            &kernel_version,
        ])
        .status();

    match depmod_status {
        Ok(status) if status.success() => {
            println!("  depmod completed successfully");
        }
        Ok(status) => {
            println!(
                "  Warning: depmod exited with status {} (module loading may still work)",
                status
            );
        }
        Err(e) => {
            println!(
                "  Warning: Could not run depmod: {} (using pre-built modules.dep)",
                e
            );
        }
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
