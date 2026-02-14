//! Build artifact cleaning.

use anyhow::Result;
use std::fs;
use std::path::Path;

use distro_spec::levitate::{
    EFIBOOT_FILENAME, INITRAMFS_BUILD_DIR, INITRAMFS_FILENAME, INITRAMFS_LIVE_OUTPUT,
    ISO_CHECKSUM_SUFFIX, ISO_FILENAME, ROOTFS_NAME,
};

/// Clean all build outputs (preserves downloads).
pub fn clean_outputs(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);

    if output_dir.exists() {
        println!("Removing {}...", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    }

    println!("Clean complete (downloads preserved).");
    Ok(())
}

/// Clean kernel build artifacts only.
pub fn clean_kernel(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let kernel_build = output_dir.join("kernel-build");
    let vmlinuz = output_dir.join("staging/boot/vmlinuz");
    let modules = output_dir.join("staging/usr/lib/modules");
    let kconfig_hash = output_dir.join(".kconfig.hash");

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

    if kconfig_hash.exists() {
        fs::remove_file(&kconfig_hash)?;
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
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let iso = output_dir.join(ISO_FILENAME);
    let checksum = iso.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let checksum_legacy = output_dir.join(format!("{}{}", ISO_FILENAME, ISO_CHECKSUM_SUFFIX));
    let initramfs = output_dir.join(INITRAMFS_FILENAME);
    let initramfs_live = output_dir.join(INITRAMFS_LIVE_OUTPUT);
    let initramfs_dir = output_dir.join("initramfs");
    let initramfs_live_root = output_dir.join(INITRAMFS_BUILD_DIR);
    let efiboot = output_dir.join(EFIBOOT_FILENAME);
    let live_overlay = output_dir.join("live-overlay");
    let initramfs_hash = output_dir.join(".initramfs-inputs.hash");

    let mut cleaned = false;

    if iso.exists() {
        println!("Removing ISO...");
        fs::remove_file(&iso)?;
        cleaned = true;
    }

    if checksum.exists() {
        println!("Removing ISO checksum...");
        fs::remove_file(&checksum)?;
        cleaned = true;
    }

    if checksum_legacy.exists() {
        println!("Removing legacy ISO checksum...");
        fs::remove_file(&checksum_legacy)?;
        cleaned = true;
    }

    if initramfs.exists() {
        println!("Removing initramfs.img...");
        fs::remove_file(&initramfs)?;
        cleaned = true;
    }

    if initramfs_live.exists() {
        println!("Removing initramfs-live.cpio.gz...");
        fs::remove_file(&initramfs_live)?;
        cleaned = true;
    }

    if initramfs_dir.exists() {
        println!("Removing initramfs build directory...");
        fs::remove_dir_all(&initramfs_dir)?;
        cleaned = true;
    }

    if initramfs_live_root.exists() {
        println!("Removing initramfs-live-root directory...");
        fs::remove_dir_all(&initramfs_live_root)?;
        cleaned = true;
    }

    if efiboot.exists() {
        println!("Removing efiboot.img...");
        fs::remove_file(&efiboot)?;
        cleaned = true;
    }

    if live_overlay.exists() {
        println!("Removing live-overlay directory...");
        fs::remove_dir_all(&live_overlay)?;
        cleaned = true;
    }

    if initramfs_hash.exists() {
        fs::remove_file(&initramfs_hash)?;
        cleaned = true;
    }

    if cleaned {
        println!("ISO/initramfs artifacts cleaned.");
    } else {
        println!("No ISO/initramfs artifacts to clean.");
    }

    Ok(())
}

/// Clean rootfs (EROFS) only.
pub fn clean_rootfs(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let rootfs = output_dir.join(ROOTFS_NAME);
    let rootfs_staging = output_dir.join("rootfs-staging");
    let rootfs_extracted = output_dir.join("rootfs-extracted");
    let rootfs_hash = output_dir.join(".rootfs-inputs.hash");

    let mut cleaned = false;

    if rootfs.exists() {
        println!("Removing EROFS rootfs...");
        fs::remove_file(&rootfs)?;
        cleaned = true;
    }

    if rootfs_staging.exists() {
        println!("Removing rootfs staging...");
        fs::remove_dir_all(&rootfs_staging)?;
        cleaned = true;
    }

    if rootfs_extracted.exists() {
        println!("Removing extracted rootfs...");
        fs::remove_dir_all(&rootfs_extracted)?;
        cleaned = true;
    }

    if rootfs_hash.exists() {
        fs::remove_file(&rootfs_hash)?;
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
