//! Build artifact cleaning.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Clean all build outputs (preserves downloads).
pub fn clean_outputs(base_dir: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");

    if output_dir.exists() {
        println!("Removing {}...", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    }

    println!("Clean complete (downloads preserved).");
    Ok(())
}

/// Clean kernel build artifacts only.
pub fn clean_kernel(base_dir: &Path) -> Result<()> {
    let kernel_build = base_dir.join("output/kernel-build");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");
    let modules = base_dir.join("output/staging/usr/lib/modules");

    let mut cleaned = false;

    if kernel_build.exists() {
        println!("Removing kernel build directory...");
        fs::remove_dir_all(&kernel_build)?;
        cleaned = true;
    }

    if vmlinuz.exists() {
        println!("Removing vmlinuz...");
        fs::remove_file(&vmlinuz)?;
        cleaned = true;
    }

    if modules.exists() {
        println!("Removing kernel modules...");
        fs::remove_dir_all(&modules)?;
        cleaned = true;
    }

    if cleaned {
        println!("Kernel artifacts cleaned.");
    } else {
        println!("No kernel artifacts to clean.");
    }

    Ok(())
}

/// Clean ISO and initramfs only.
pub fn clean_iso(base_dir: &Path) -> Result<()> {
    let iso = base_dir.join("output/levitateos.iso");
    let initramfs = base_dir.join("output/initramfs.img");
    let initramfs_dir = base_dir.join("output/initramfs");

    let mut cleaned = false;

    if iso.exists() {
        println!("Removing ISO...");
        fs::remove_file(&iso)?;
        cleaned = true;
    }

    if initramfs.exists() {
        println!("Removing initramfs.img...");
        fs::remove_file(&initramfs)?;
        cleaned = true;
    }

    if initramfs_dir.exists() {
        println!("Removing initramfs build directory...");
        fs::remove_dir_all(&initramfs_dir)?;
        cleaned = true;
    }

    if cleaned {
        println!("ISO/initramfs artifacts cleaned.");
    } else {
        println!("No ISO/initramfs artifacts to clean.");
    }

    Ok(())
}

/// Clean rootfs tarball only.
pub fn clean_rootfs(base_dir: &Path) -> Result<()> {
    let tarball = base_dir.join("output/levitateos-base.tar.xz");
    let rootfs_dir = base_dir.join("output/rootfs");

    let mut cleaned = false;

    if tarball.exists() {
        println!("Removing rootfs tarball...");
        fs::remove_file(&tarball)?;
        cleaned = true;
    }

    if rootfs_dir.exists() {
        println!("Removing extracted rootfs...");
        fs::remove_dir_all(&rootfs_dir)?;
        cleaned = true;
    }

    if cleaned {
        println!("Rootfs artifacts cleaned.");
    } else {
        println!("No rootfs artifacts to clean.");
    }

    Ok(())
}

/// Clean downloaded files (Rocky ISO, extracted contents, Linux source).
pub fn clean_downloads(base_dir: &Path) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");

    if downloads_dir.exists() {
        println!("Removing downloads directory (8.6GB+ of data)...");
        fs::remove_dir_all(&downloads_dir)?;
        println!("Downloads cleaned.");
    } else {
        println!("No downloads to clean.");
    }

    Ok(())
}

/// Clean everything (downloads + outputs).
pub fn clean_all(base_dir: &Path) -> Result<()> {
    clean_downloads(base_dir)?;
    clean_outputs(base_dir)?;
    println!("\nFull clean complete.");
    Ok(())
}
