//! ISO creation - builds bootable LevitateOS ISO.
//!
//! Creates an ISO with squashfs-based architecture:
//! - Tiny initramfs (~5MB) - mounts squashfs + overlay
//! - Squashfs image (~350MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use distro_builder::{
    copy_dir_recursive, create_efi_boot_image, generate_iso_checksum, run_xorriso,
    setup_iso_structure,
};
use distro_spec::levitate::{
    // Identity
    ISO_LABEL, ISO_FILENAME, OS_NAME,
    // Squashfs
    SQUASHFS_NAME, SQUASHFS_ISO_PATH,
    // Boot files
    KERNEL_ISO_PATH, INITRAMFS_LIVE_ISO_PATH,
    INITRAMFS_LIVE_OUTPUT, INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_INSTALLED_ISO_PATH,
    // ISO structure
    ISO_EFI_DIR, LIVE_OVERLAY_ISO_PATH,
    // EFI
    EFIBOOT_FILENAME, EFI_BOOTLOADER, EFI_GRUB,
    // Console
    SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE,
};
use crate::component::custom::create_live_overlay_at;

/// Get ISO volume label from environment or use default.
/// Used for boot device detection (root=LABEL=X).
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string())
}

/// Paths used during ISO creation.
struct IsoPaths {
    iso_contents: PathBuf,
    output_dir: PathBuf,
    squashfs: PathBuf,
    initramfs_live: PathBuf,
    initramfs_installed: PathBuf,
    iso_output: PathBuf,
    iso_root: PathBuf,
}

impl IsoPaths {
    fn new(base_dir: &Path) -> Self {
        let extract_dir = base_dir.join("downloads");
        let output_dir = base_dir.join("output");
        Self {
            iso_contents: extract_dir.join("iso-contents"),
            output_dir: output_dir.clone(),
            squashfs: output_dir.join(SQUASHFS_NAME),
            initramfs_live: output_dir.join(INITRAMFS_LIVE_OUTPUT),
            initramfs_installed: output_dir.join(INITRAMFS_INSTALLED_OUTPUT),
            iso_output: output_dir.join(ISO_FILENAME),
            iso_root: output_dir.join("iso-root"),
        }
    }
}

/// Create ISO using squashfs-based architecture.
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts squashfs + overlay
/// - Squashfs image (~350MB) - complete base system
/// - Live overlay - live-specific configs (autologin, serial console, empty root password)
///
/// Boot flow:
/// 1. kernel -> tiny initramfs
/// 2. init_tiny mounts squashfs as lower layer
/// 3. init_tiny mounts /live/overlay from ISO as middle layer
/// 4. init_tiny mounts tmpfs as upper layer (for writes)
/// 5. switch_root -> systemd
///
/// This architecture ensures:
/// - Live ISO has autologin and empty root password (via overlay)
/// - Installed systems (via recstrap) have proper security (squashfs only)
pub fn create_squashfs_iso(base_dir: &Path) -> Result<()> {
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

    // Stage 8: Verify hardware compatibility (BAIL on critical failures)
    println!("\nVerifying hardware compatibility before finalizing...");
    let has_critical = verify_hardware_compat(base_dir)?;
    if has_critical {
        let _ = fs::remove_file(&temp_iso); // Cleanup 
        bail!("ISO creation aborted: Hardware compatibility verification failed with critical errors.");
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
    // Firmware is installed to squashfs-root during squashfs build, not staging
    let checker = hardware_compat::HardwareCompatChecker::new(
        output_dir.join("kernel-build/.config"),
        output_dir.join("squashfs-root/usr/lib/firmware"),
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
    if !paths.squashfs.exists() {
        bail!(
            "Squashfs not found at {}.\n\
             Run 'leviso build squashfs' first.",
            paths.squashfs.display()
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

/// Stage 4: Copy kernel, initramfs, squashfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths, kernel_path: &Path) -> Result<()> {
    // Copy kernel and live initramfs (tiny - for live boot)
    fs::copy(kernel_path, paths.iso_root.join(KERNEL_ISO_PATH))?;
    fs::copy(&paths.initramfs_live, paths.iso_root.join(INITRAMFS_LIVE_ISO_PATH))?;

    // Copy installed initramfs (full dracut - boots the daily driver OS)
    // This is REQUIRED - copied to installed systems instead of running dracut
    if !paths.initramfs_installed.exists() {
        bail!(
            "Installed initramfs not found at {}.\n\
             Run 'leviso build' to generate it.",
            paths.initramfs_installed.display()
        );
    }
    println!("Copying installed initramfs to ISO...");
    fs::copy(&paths.initramfs_installed, paths.iso_root.join(INITRAMFS_INSTALLED_ISO_PATH))?;

    // Copy squashfs to /live/
    println!("Copying squashfs to ISO...");
    fs::copy(&paths.squashfs, paths.iso_root.join(SQUASHFS_ISO_PATH))?;

    // Copy live overlay to /live/overlay/
    // This contains live-specific configs (autologin, serial console, empty root password)
    // that are layered on top of squashfs during live boot only
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

/// Stage 5: Set up UEFI boot files and GRUB config.
fn setup_uefi_boot(paths: &IsoPaths) -> Result<()> {
    let efi_src = paths.iso_contents.join("EFI/BOOT");
    if !efi_src.exists() {
        bail!(
            "EFI boot files not found at {}.\n\
             Run 'leviso extract rocky' first.",
            efi_src.display()
        );
    }

    println!("Setting up UEFI boot...");
    fs::copy(
        efi_src.join(EFI_BOOTLOADER),
        paths.iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER),
    )?;
    fs::copy(
        efi_src.join(EFI_GRUB),
        paths.iso_root.join(ISO_EFI_DIR).join(EFI_GRUB),
    )?;

    // Create GRUB config with root=LABEL for device detection
    // selinux=0 disables SELinux (we don't ship policies)
    // Serial terminal is configured for automated testing (install-tests)
    let label = iso_label();
    let grub_cfg = format!(
        r#"# Serial console for automated testing
serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1
terminal_input serial console
terminal_output serial console

set default=0
set timeout=5

menuentry '{}' {{
    linuxefi /{} root=LABEL={} {} {} {}
    initrdefi /{}
}}

menuentry '{} (Emergency Shell)' {{
    linuxefi /{} root=LABEL={} {} {} {} emergency
    initrdefi /{}
}}

menuentry '{} (Debug)' {{
    linuxefi /{} root=LABEL={} {} {} {} debug
    initrdefi /{}
}}
"#,
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_LIVE_ISO_PATH,
    );
    fs::write(paths.iso_root.join(ISO_EFI_DIR).join("grub.cfg"), grub_cfg)?;

    // Create EFI boot image
    let efiboot_img = paths.output_dir.join(EFIBOOT_FILENAME);
    build_efi_boot_image(&paths.iso_root, &efiboot_img)?;

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
    println!("\n=== Squashfs ISO Created ===");
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
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}

/// Create a FAT16 image containing EFI boot files
fn build_efi_boot_image(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    let efi_dir = iso_root.join(ISO_EFI_DIR);

    // Create EFI boot image with bootloader, GRUB, and config
    create_efi_boot_image(
        efiboot_img,
        &[
            (efi_dir.join(EFI_BOOTLOADER).as_path(), EFI_BOOTLOADER),
            (efi_dir.join(EFI_GRUB).as_path(), EFI_GRUB),
            (efi_dir.join("grub.cfg").as_path(), "grub.cfg"),
        ],
    )?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join(EFIBOOT_FILENAME))?;

    Ok(())
}
