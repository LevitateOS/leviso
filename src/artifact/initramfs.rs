//! Initramfs builders for LevitateOS.
//!
//! Two initramfs types are built:
//! - **Live initramfs** (`build_tiny_initramfs`): ~5MB, busybox-based, mounts ISO squashfs
//! - **Install initramfs** (`build_install_initramfs`): ~30-50MB, systemd-based, boots from disk
//!
//! Both use recinit for the actual build - this module provides leviso-specific wrappers
//! that handle finding the correct kernel modules and busybox binary.
//!
//! # Live Boot Flow
//!
//! ```text
//! 1. GRUB loads kernel + live initramfs
//! 2. Kernel extracts initramfs to rootfs, runs /init (busybox script)
//! 3. /init mounts ISO, squashfs, creates overlay
//! 4. switch_root to overlay, systemd takes over
//! ```
//!
//! # Installed Boot Flow
//!
//! ```text
//! 1. systemd-boot loads kernel + install initramfs
//! 2. Kernel extracts initramfs to rootfs, runs /init (-> systemd)
//! 3. systemd mounts root partition, switch_root
//! 4. Real systemd takes over
//! ```

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::Path;

use distro_spec::levitate::{
    BOOT_DEVICE_PROBE_ORDER, BUSYBOX_URL, BUSYBOX_URL_ENV, CPIO_GZIP_LEVEL, INITRAMFS_INSTALLED_OUTPUT,
    INITRAMFS_LIVE_OUTPUT, ISO_LABEL, LIVE_OVERLAY_ISO_PATH, ROOTFS_ISO_PATH,
};
use recinit::{download_busybox, InstallConfig, ModulePreset, TinyConfig};

/// Get busybox download URL from environment or use default.
fn busybox_url() -> String {
    env::var(BUSYBOX_URL_ENV).unwrap_or_else(|_| BUSYBOX_URL.to_string())
}

/// Build the tiny initramfs for live ISO boot.
///
/// This creates a small (~5MB) busybox-based initramfs that:
/// 1. Loads kernel modules for CDROM/storage access
/// 2. Finds and mounts the squashfs/erofs root filesystem
/// 3. Creates an overlay for writable storage
/// 4. switch_root to the live system
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");
    let downloads_dir = base_dir.join("downloads");

    // Find kernel modules directory
    let modules_dir = find_kernel_modules_dir(base_dir)?;

    // Find kernel version from modules directory
    let kernel_version = find_kernel_version(&modules_dir)?;
    let modules_path = modules_dir.join(&kernel_version);

    // Ensure busybox is available
    let busybox_path = downloads_dir.join("busybox-static");
    if !busybox_path.exists() {
        let url = busybox_url();
        println!("Downloading static busybox from {}", url);
        download_busybox(&url, &busybox_path)?;
    }

    // Build using recinit
    let config = TinyConfig {
        modules_dir: modules_path,
        busybox_path,
        template_path: base_dir.join("profile/init_tiny.template"),
        output: output_dir.join(INITRAMFS_LIVE_OUTPUT),
        iso_label: ISO_LABEL.to_string(),
        rootfs_path: ROOTFS_ISO_PATH.to_string(),
        live_overlay_path: Some(LIVE_OVERLAY_ISO_PATH.to_string()),
        boot_devices: BOOT_DEVICE_PROBE_ORDER.iter().map(|s| s.to_string()).collect(),
        module_preset: ModulePreset::Live,
        gzip_level: CPIO_GZIP_LEVEL,
        check_builtin: true,
    };

    recinit::build_tiny_initramfs(&config, true)
}

/// Build a full initramfs for installed systems.
///
/// This creates a larger (~30-50MB) systemd-based initramfs that:
/// 1. Loads all common storage drivers (NVMe, SATA, USB)
/// 2. Includes systemd for service management
/// 3. Mounts the root filesystem from disk
/// 4. Hands off to systemd
///
/// By pre-building this during ISO creation, we save time during installation.
/// The initramfs is generic (all drivers) so it works on any hardware.
pub fn build_install_initramfs(base_dir: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");
    let downloads_rootfs = base_dir.join("downloads/rootfs");

    // Use downloads/rootfs for install initramfs - it has the full systemd units
    // including initrd.target which squashfs-root lacks (stripped for live use)
    if !downloads_rootfs.exists() {
        bail!(
            "downloads/rootfs not found at {}.\n\
             Run 'leviso build' to extract the Rocky Linux rootfs first.",
            downloads_rootfs.display()
        );
    }

    // Check for custom kernel modules - use those if available
    // Custom kernel modules are in output/staging, Rocky modules are in downloads/rootfs
    let custom_modules_path = output_dir.join("staging");
    let modules_path = if custom_modules_path.join("usr/lib/modules").exists() {
        println!("  Using CUSTOM kernel modules for install initramfs");
        Some(custom_modules_path)
    } else {
        println!("  Using ROCKY kernel modules for install initramfs");
        None
    };

    // Build using recinit
    let config = InstallConfig {
        rootfs: downloads_rootfs,
        modules_path,
        output: output_dir.join(INITRAMFS_INSTALLED_OUTPUT),
        module_preset: ModulePreset::Install,
        gzip_level: CPIO_GZIP_LEVEL,
        include_firmware: true,
    };

    recinit::build_install_initramfs(&config, true)
}

/// Find the kernel modules directory.
///
/// For CUSTOM kernels: `output/staging/usr/lib/modules`
/// For ROCKY kernels: `downloads/rootfs/usr/lib/modules`
///
/// ANTI-CHEAT: If using custom modules, the vmlinuz must also exist.
fn find_kernel_modules_dir(base_dir: &Path) -> Result<std::path::PathBuf> {
    let custom_modules_path = base_dir.join("output/staging/usr/lib/modules");
    let rocky_modules_path = base_dir.join("downloads/rootfs/usr/lib/modules");
    let vmlinuz_path = base_dir.join("output/staging/boot/vmlinuz");

    if custom_modules_path.exists() {
        // ANTI-CHEAT: Ensure the kernel binary ACTUALLY exists if we use custom modules
        if !vmlinuz_path.exists() {
            bail!(
                "Custom modules found but vmlinuz is missing from staging.\n\
                 This indicates a broken or partial kernel build.\n\
                 Refusing to build initramfs with half-built kernel."
            );
        }
        println!("  Using CUSTOM kernel modules from {}", custom_modules_path.display());
        Ok(custom_modules_path)
    } else if rocky_modules_path.exists() {
        println!("  Using ROCKY kernel modules from {}", rocky_modules_path.display());
        Ok(rocky_modules_path)
    } else {
        bail!(
            "No kernel modules found. Expected at:\n\
             - {}\n\
             - {}\n\
             \n\
             CDROM kernel modules (sr_mod, cdrom, isofs) are REQUIRED.\n\
             Without them, the ISO cannot boot.\n\
             \n\
             Run 'leviso build kernel' or 'leviso extract rocky' first.",
            custom_modules_path.display(),
            rocky_modules_path.display()
        );
    }
}

/// Find kernel version from modules directory.
fn find_kernel_version(modules_dir: &Path) -> Result<String> {
    fs::read_dir(modules_dir)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .context(format!(
            "No kernel version directory found in {}.\n\
             The modules directory exists but contains no kernel version.\n\
             This indicates a corrupted or incomplete rootfs extraction.",
            modules_dir.display()
        ))
}
