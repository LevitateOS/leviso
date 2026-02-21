//! Initramfs builders for LevitateOS.
//!
//! Two initramfs types are built:
//! - **Live initramfs** (`build_tiny_initramfs`): ~5MB, busybox-based, mounts ISO EROFS
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
//! 3. /init mounts ISO, EROFS rootfs, creates overlay
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
use leviso_cheat_guard::cheat_bail;
use std::env;
use std::fs;
use std::path::Path;

use fsdbg::checklist::ChecklistType;
use fsdbg::cpio::CpioReader;

use distro_spec::levitate::{
    BOOT_DEVICE_PROBE_ORDER, BUSYBOX_URL, BUSYBOX_URL_ENV, CPIO_GZIP_LEVEL,
    INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_LIVE_OUTPUT, ISO_LABEL, LIVE_OVERLAY_ISO_PATH,
    ROOTFS_ISO_PATH,
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
/// 2. Finds and mounts the EROFS root filesystem
/// 3. Creates an overlay for writable storage
/// 4. switch_root to the live system
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
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
        live_overlay_image_path: Some(LIVE_OVERLAY_ISO_PATH.to_string()),
        live_overlay_path: Some(LIVE_OVERLAY_ISO_PATH.to_string()),
        boot_devices: BOOT_DEVICE_PROBE_ORDER
            .iter()
            .map(|s| s.to_string())
            .collect(),
        module_preset: ModulePreset::Live,
        gzip_level: CPIO_GZIP_LEVEL,
        check_builtin: true,
        extra_template_vars: Vec::new(),
    };

    recinit::build_tiny_initramfs(&config, true)?;

    // Verify the built initramfs
    let output_path = output_dir.join(INITRAMFS_LIVE_OUTPUT);
    verify_live_initramfs(&output_path)?;

    Ok(())
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
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let downloads_rootfs = base_dir.join("downloads/rootfs");

    // Use downloads/rootfs for install initramfs - it has the full systemd units
    // including initrd.target which rootfs-staging lacks (stripped for live use)
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
    // Note: Firmware is NOT included - it's available on root filesystem once mounted.
    // For initramfs we only need drivers to detect/mount root, not full firmware.
    let config = InstallConfig {
        rootfs: downloads_rootfs,
        modules_path,
        output: output_dir.join(INITRAMFS_INSTALLED_OUTPUT),
        module_preset: ModulePreset::Install,
        gzip_level: CPIO_GZIP_LEVEL,
        include_firmware: false,
    };

    recinit::build_install_initramfs(&config, true)?;

    // Verify the built initramfs
    let output_path = output_dir.join(INITRAMFS_INSTALLED_OUTPUT);
    verify_install_initramfs(&output_path)?;

    Ok(())
}

/// Find the kernel modules directory.
///
/// For CUSTOM kernels: `output/staging/usr/lib/modules`
/// For ROCKY kernels: `downloads/rootfs/usr/lib/modules`
///
/// ANTI-CHEAT: If using custom modules, the vmlinuz must also exist.
fn find_kernel_modules_dir(base_dir: &Path) -> Result<std::path::PathBuf> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let custom_modules_path = output_dir.join("staging/usr/lib/modules");
    let rocky_modules_path = base_dir.join("downloads/rootfs/usr/lib/modules");
    let vmlinuz_path = output_dir.join("staging/boot/vmlinuz");

    if custom_modules_path.exists() {
        // ANTI-CHEAT: Ensure the kernel binary ACTUALLY exists if we use custom modules
        if !vmlinuz_path.exists() {
            bail!(
                "Custom modules found but vmlinuz is missing from staging.\n\
                 This indicates a broken or partial kernel build.\n\
                 Refusing to build initramfs with half-built kernel."
            );
        }
        println!(
            "  Using CUSTOM kernel modules from {}",
            custom_modules_path.display()
        );
        Ok(custom_modules_path)
    } else if rocky_modules_path.exists() {
        println!(
            "  Using ROCKY kernel modules from {}",
            rocky_modules_path.display()
        );
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
             Run 'cargo xtask kernels build leviso' or 'leviso extract rocky' first.",
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

/// Verify the live initramfs using fsdbg checklist.
pub fn verify_live_initramfs(path: &Path) -> Result<()> {
    do_verify_initramfs(path, ChecklistType::LiveInitramfs)
}

/// Verify the install initramfs using fsdbg checklist.
pub fn verify_install_initramfs(path: &Path) -> Result<()> {
    do_verify_initramfs(path, ChecklistType::InstallInitramfs)
}

/// Internal: Verify initramfs using fsdbg.
fn do_verify_initramfs(path: &Path, checklist_type: ChecklistType) -> Result<()> {
    print!("  Verifying {}... ", checklist_type.name());

    let reader = CpioReader::open(path).with_context(|| {
        format!(
            "Failed to open initramfs for verification: {}",
            path.display()
        )
    })?;

    let report = match checklist_type {
        ChecklistType::InstallInitramfs => fsdbg::checklist::install_initramfs::verify(&reader),
        ChecklistType::LiveInitramfs => fsdbg::checklist::live_initramfs::verify(&reader),
        // This function is for CPIO initramfs only - other types shouldn't reach here
        _ => bail!(
            "do_verify_initramfs() only handles initramfs types, got {:?}",
            checklist_type
        ),
    };

    let passed = report.passed();
    let total = report.total();

    if report.is_success() {
        println!("OK ({}/{} checks)", passed, total);
        Ok(())
    } else {
        println!("FAILED ({}/{} checks)", passed, total);
        let mut missing_items = Vec::new();
        for result in &report.results {
            if !result.passed {
                let msg = result.message.as_deref().unwrap_or("Missing");
                println!("    ✗ {} - {}", result.item, msg);
                missing_items.push(result.item.clone());
            }
        }
        cheat_bail!(
            protects = "Users can boot LevitateOS from ISO or installed system",
            severity = "CRITICAL",
            cheats = [
                "Move missing items to an OPTIONAL list",
                "Remove items from REQUIRED list",
                "Skip verification entirely",
                "Mark test as ignored or skipped",
                "Return Ok() without checking report.is_success()"
            ],
            consequence = "System fails to boot. Users see kernel panic or 'init not found' error.",
            "Initramfs verification FAILED: {} missing files.\n\
             Missing: {}\n\n\
             The built initramfs is incomplete and will not boot correctly.\n\
             Fix the build process to include ALL required files.",
            report.failed(),
            missing_items.join(", ")
        );
    }
}
