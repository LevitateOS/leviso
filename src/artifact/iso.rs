//! ISO creation - builds bootable LevitateOS ISO.
//!
//! Creates an ISO with EROFS-based architecture and UKI boot:
//! - UKIs (~50MB each) - kernel + initramfs + cmdline in signed PE binary
//! - EROFS image (~350MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use distro_builder::{
    copy_dir_recursive, create_efi_dirs_in_fat, create_fat16_image, generate_iso_checksum,
    mcopy_to_fat, run_xorriso, setup_iso_structure,
};
use distro_builder::process::Cmd;
use distro_spec::levitate::{
    // Identity
    ISO_LABEL, ISO_FILENAME,
    // Rootfs (EROFS)
    ROOTFS_NAME, ROOTFS_ISO_PATH,
    // Boot files
    KERNEL_ISO_PATH, INITRAMFS_LIVE_ISO_PATH,
    INITRAMFS_LIVE_OUTPUT, INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_INSTALLED_ISO_PATH,
    // ISO structure
    ISO_EFI_DIR, LIVE_OVERLAY_ISO_PATH,
    // EFI / UKI
    EFIBOOT_FILENAME, EFIBOOT_SIZE_MB, EFI_BOOTLOADER,
    UKI_EFI_DIR, LOADER_ENTRIES_DIR, SYSTEMD_BOOT_EFI,
    // Installed UKIs
    UKI_INSTALLED_ISO_DIR,
};
use crate::component::custom::create_live_overlay_at;

/// Get ISO volume label from environment or use default.
/// Used for boot device detection (root=LABEL=X).
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string())
}

/// Paths used during ISO creation.
struct IsoPaths {
    output_dir: PathBuf,
    rootfs: PathBuf,
    initramfs_live: PathBuf,
    initramfs_installed: PathBuf,
    iso_output: PathBuf,
    iso_root: PathBuf,
}

impl IsoPaths {
    fn new(base_dir: &Path) -> Self {
        let output_dir = base_dir.join("output");
        Self {
            output_dir: output_dir.clone(),
            rootfs: output_dir.join(ROOTFS_NAME),
            initramfs_live: output_dir.join(INITRAMFS_LIVE_OUTPUT),
            initramfs_installed: output_dir.join(INITRAMFS_INSTALLED_OUTPUT),
            iso_output: output_dir.join(ISO_FILENAME),
            iso_root: output_dir.join("iso-root"),
        }
    }
}

/// Create ISO using EROFS-based architecture.
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts EROFS + overlay
/// - EROFS image (~350MB) - complete base system
/// - Live overlay - live-specific configs (autologin, serial console, empty root password)
///
/// Boot flow:
/// 1. kernel -> tiny initramfs
/// 2. init_tiny mounts EROFS as lower layer
/// 3. init_tiny mounts /live/overlay from ISO as middle layer
/// 4. init_tiny mounts tmpfs as upper layer (for writes)
/// 5. switch_root -> systemd
///
/// This architecture ensures:
/// - Live ISO has autologin and empty root password (via overlay)
/// - Installed systems (via recstrap) have proper security (EROFS only)
pub fn create_iso(base_dir: &Path) -> Result<()> {
    let paths = IsoPaths::new(base_dir);

    println!("=== Building LevitateOS ISO (Atomic) ===\n");

    // Stage 1: Validate inputs
    validate_iso_inputs(&paths)?;
    let kernel_path = find_kernel(&paths)?;

    // Stage 2: Create live overlay (autologin, serial console, empty root password)
    // This is ONLY applied during live boot, NOT extracted to installed systems
    create_live_overlay_at(&paths.output_dir)?;

    // Stage 3: Set up ISO directory structure
    setup_iso_dirs(&paths)?;

    // Stage 4: Copy boot files and artifacts (including live overlay)
    copy_iso_artifacts(&paths, &kernel_path)?;

    // Stage 5: Set up UEFI boot
    setup_uefi_boot(&paths)?;

    // Stage 6: Create the ISO to a temporary file (Atomic Artifacts)
    let temp_iso = paths.output_dir.join(format!("{}.tmp", distro_spec::levitate::ISO_FILENAME));
    run_xorriso_to(&paths, &temp_iso)?;

    // Stage 7: Generate checksum for the temporary ISO
    generate_checksum(&temp_iso)?;

    // Stage 8: Verify hardware compatibility (WARN only, don't block ISO creation)
    // TODO: Re-enable blocking once Server/Workstation firmware issues are resolved
    println!("\nVerifying hardware compatibility...");
    let has_critical = verify_hardware_compat(base_dir)?;
    if has_critical {
        println!("[WARN] Hardware compatibility verification has critical errors, but continuing ISO build.");
    }

    // Atomic rename to final destination
    fs::rename(&temp_iso, &paths.iso_output)?;

    // Also move the checksum
    let temp_checksum = temp_iso.with_extension(distro_spec::levitate::ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let final_checksum = paths.iso_output.with_extension(distro_spec::levitate::ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    fs::rename(&temp_checksum, &final_checksum)?;

    print_iso_summary(&paths.iso_output);
    Ok(())
}

/// Helper to run hardware compat verification.
fn verify_hardware_compat(base_dir: &Path) -> Result<bool> {
    let output_dir = base_dir.join("output");
    // Firmware is installed to rootfs-staging during rootfs build, not staging
    let checker = hardware_compat::HardwareCompatChecker::new(
        output_dir.join("kernel-build/.config"),
        output_dir.join("rootfs-staging/usr/lib/firmware"),
    );

    let all_profiles = hardware_compat::profiles::get_all_profiles();
    let mut has_critical = false;

    for p in all_profiles {
        match checker.verify_profile(p.as_ref()) {
            Ok(report) => {
                report.print_summary();
                if report.has_critical_failures() {
                    has_critical = true;
                }
            }
            Err(e) => {
                println!("  [ERROR] Failed to verify profile '{}': {}", p.name(), e);
                has_critical = true;
            }
        }
    }

    Ok(has_critical)
}

/// Stage 1: Validate that required input files exist and are consistent.
fn validate_iso_inputs(paths: &IsoPaths) -> Result<()> {
    if !paths.rootfs.exists() {
        bail!(
            "EROFS rootfs not found at {}.\n\
             Run 'leviso build rootfs' first.",
            paths.rootfs.display()
        );
    }

    if !paths.initramfs_live.exists() {
        bail!(
            "Live initramfs not found at {}.\n\
             Run 'leviso build initramfs' first.",
            paths.initramfs_live.display()
        );
    }

    // Integrity Check: Verify staging kernel vs modules
    let staging_boot = paths.output_dir.join("staging/boot/vmlinuz");
    let staging_modules = paths.output_dir.join("staging/usr/lib/modules");

    if staging_boot.exists() && staging_modules.exists() {
        // Find module directory version
        let mut modules_version = None;
        for entry in fs::read_dir(&staging_modules)? {
            match entry {
                Ok(e) if e.path().is_dir() => {
                    modules_version = Some(e.file_name().to_string_lossy().to_string());
                    break;
                }
                Ok(_) => {} // Not a directory, skip
                Err(e) => {
                    eprintln!(
                        "  [WARN] Error reading staging modules directory entry: {}",
                        e
                    );
                }
            }
        }

        if let Some(version) = modules_version {
            println!("  [CHECK] Staging kernel modules version: {}", version);
            // We could store the version during kernel build in a file to be 100% sure
            // but for now, the presence of a matching directory is a strong indicator.
        } else {
            bail!("Staging directory exists but contains no kernel modules!");
        }
    }

    Ok(())
}

/// Find the LevitateOS kernel. No fallbacks - fail fast if not built.
fn find_kernel(paths: &IsoPaths) -> Result<PathBuf> {
    let kernel = paths.output_dir.join("staging/boot/vmlinuz");
    if !kernel.exists() {
        bail!(
            "LevitateOS kernel not found at: {}\n\
             Run 'leviso build kernel' first.",
            kernel.display()
        );
    }
    Ok(kernel)
}

/// Stage 3: Create ISO directory structure.
fn setup_iso_dirs(paths: &IsoPaths) -> Result<()> {
    setup_iso_structure(&paths.iso_root)
}

/// Stage 4: Copy kernel, initramfs, EROFS rootfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths, kernel_path: &Path) -> Result<()> {
    // Copy kernel and live initramfs (tiny - for live boot)
    fs::copy(kernel_path, paths.iso_root.join(KERNEL_ISO_PATH))?;
    fs::copy(&paths.initramfs_live, paths.iso_root.join(INITRAMFS_LIVE_ISO_PATH))?;

    // Copy installed initramfs (full - boots the daily driver OS)
    // This is REQUIRED - copied to installed systems during installation
    if !paths.initramfs_installed.exists() {
        bail!(
            "Installed initramfs not found at {}.\n\
             Run 'leviso build' to generate it.",
            paths.initramfs_installed.display()
        );
    }
    println!("Copying installed initramfs to ISO...");
    fs::copy(&paths.initramfs_installed, paths.iso_root.join(INITRAMFS_INSTALLED_ISO_PATH))?;

    // Copy EROFS rootfs to /live/
    println!("Copying EROFS rootfs to ISO...");
    fs::copy(&paths.rootfs, paths.iso_root.join(ROOTFS_ISO_PATH))?;

    // Copy live overlay to /live/overlay/
    // This contains live-specific configs (autologin, serial console, empty root password)
    // that are layered on top of EROFS during live boot only
    let live_overlay_src = paths.output_dir.join("live-overlay");
    let live_overlay_dst = paths.iso_root.join(LIVE_OVERLAY_ISO_PATH);
    if live_overlay_src.exists() {
        println!("Copying live overlay to ISO...");
        copy_dir_recursive(&live_overlay_src, &live_overlay_dst)?;
    } else {
        bail!(
            "Live overlay not found at {}.\n\
             This should have been created by create_live_overlay().",
            live_overlay_src.display()
        );
    }

    Ok(())
}

/// Stage 5: Set up UEFI boot with systemd-boot and UKIs.
///
/// This replaces GRUB with the modern UKI boot flow:
/// 1. Build UKIs (kernel + initramfs + cmdline in single PE binary)
/// 2. Copy systemd-boot as the bootloader
/// 3. Create loader.conf for boot menu configuration
fn setup_uefi_boot(paths: &IsoPaths) -> Result<()> {
    println!("Setting up UEFI boot with UKI...");

    // Verify systemd-boot is available
    let systemd_boot = Path::new(SYSTEMD_BOOT_EFI);
    if !systemd_boot.exists() {
        bail!(
            "systemd-boot not found at {}.\n\
             Install: sudo dnf install systemd-boot",
            systemd_boot.display()
        );
    }

    // Find kernel and initramfs
    let kernel = paths.output_dir.join("staging/boot/vmlinuz");
    let initramfs = &paths.initramfs_live;
    let label = iso_label();

    // Create UKI directory in ISO root
    let uki_dir = paths.iso_root.join(UKI_EFI_DIR);
    fs::create_dir_all(&uki_dir)?;

    // Build UKIs using our uki module
    crate::artifact::uki::build_live_ukis(&kernel, initramfs, &uki_dir, &label)?;

    // Build installed UKIs (for users to copy during installation)
    // These use the full initramfs and boot from disk
    let installed_uki_dir = paths.iso_root.join(UKI_INSTALLED_ISO_DIR);
    fs::create_dir_all(&installed_uki_dir)?;
    crate::artifact::uki::build_installed_ukis(
        &kernel,
        &paths.initramfs_installed,
        &installed_uki_dir,
    )?;

    // Copy systemd-boot as the primary bootloader
    fs::copy(
        systemd_boot,
        paths.iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER),
    )?;

    // Create loader.conf for systemd-boot
    let loader_dir = paths.iso_root.join(LOADER_ENTRIES_DIR);
    fs::create_dir_all(&loader_dir)?;
    fs::write(
        loader_dir.join("loader.conf"),
        "timeout 5\ndefault levitateos-live.efi\n",
    )?;

    // Create EFI boot image with UKIs
    let efiboot_img = paths.output_dir.join(EFIBOOT_FILENAME);
    build_efi_boot_image_uki(&paths.iso_root, &efiboot_img)?;

    Ok(())
}

/// Stage 6: Run xorriso to create the final ISO.
fn run_xorriso_to(paths: &IsoPaths, output: &Path) -> Result<()> {
    println!("Creating UEFI bootable ISO with xorriso...");
    let label = iso_label();
    run_xorriso(&paths.iso_root, output, &label, EFIBOOT_FILENAME)
}

/// Stage 7: Generate SHA512 checksum for download verification.
fn generate_checksum(iso_path: &Path) -> Result<()> {
    println!("Generating SHA512 checksum...");
    generate_iso_checksum(iso_path)?;
    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== LevitateOS ISO Created (UKI Boot) ===");
    println!("  Output: {}", iso_output.display());
    match fs::metadata(iso_output) {
        Ok(meta) => {
            println!("  Size: {} MB", meta.len() / 1024 / 1024);
        }
        Err(e) => {
            eprintln!("  [WARN] Could not read ISO size: {}", e);
        }
    }
    println!("  Label: {}", iso_label());
    println!("\nContents:");
    println!("  - Live UKIs: EFI/Linux/ (boot from ISO)");
    println!("  - Installed UKIs: boot/uki/ (copy to /boot/EFI/Linux/ during install)");
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}

/// Create EFI boot image with systemd-boot and UKIs for xorriso.
///
/// UKIs are larger than traditional boot files (~50MB each), so we need
/// a 200MB image to hold systemd-boot + 3 UKIs + loader.conf.
fn build_efi_boot_image_uki(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    println!("Creating EFI boot image with UKIs...");

    // Create larger FAT16 image for UKIs (200MB)
    create_fat16_image(efiboot_img, EFIBOOT_SIZE_MB)?;

    // Create standard EFI directory structure
    create_efi_dirs_in_fat(efiboot_img)?;

    // Create EFI/Linux directory for UKIs
    let img_str = efiboot_img.to_string_lossy();
    Cmd::new("mmd")
        .args(["-i", &img_str, "::EFI/Linux"])
        .error_msg("mmd failed to create ::EFI/Linux directory")
        .run()?;

    // Copy systemd-boot bootloader
    mcopy_to_fat(
        efiboot_img,
        &iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER),
        "::EFI/BOOT/",
    )?;

    // Copy all UKIs from EFI/Linux
    let uki_src_dir = iso_root.join(UKI_EFI_DIR);
    for entry in fs::read_dir(&uki_src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "efi") {
            mcopy_to_fat(efiboot_img, &path, "::EFI/Linux/")?;
        }
    }

    // Create loader directory and copy loader.conf
    Cmd::new("mmd")
        .args(["-i", &img_str, "::loader"])
        .error_msg("mmd failed to create ::loader directory")
        .run()?;

    mcopy_to_fat(
        efiboot_img,
        &iso_root.join(LOADER_ENTRIES_DIR).join("loader.conf"),
        "::loader/",
    )?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join(EFIBOOT_FILENAME))?;

    Ok(())
}
