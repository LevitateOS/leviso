//! ISO creation - builds bootable LevitateOS ISO.
//!
//! Creates an ISO with squashfs-based architecture:
//! - Tiny initramfs (~5MB) - mounts squashfs + overlay
//! - Squashfs image (~350MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use leviso_elf::copy_dir_recursive;
use distro_spec::levitate::{
    // Identity
    ISO_LABEL, ISO_FILENAME, OS_NAME,
    // Squashfs
    SQUASHFS_NAME, SQUASHFS_ISO_PATH,
    // Boot files
    KERNEL_ISO_PATH, INITRAMFS_ISO_PATH,
    // ISO structure
    ISO_BOOT_DIR, ISO_LIVE_DIR, ISO_EFI_DIR,
    LIVE_OVERLAY_ISO_PATH,
    // EFI
    EFIBOOT_FILENAME, EFIBOOT_SIZE_MB,
    EFI_BOOTLOADER, EFI_GRUB,
    // Console
    SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE,
    // Checksum
    ISO_CHECKSUM_SUFFIX, SHA512_SEPARATOR,
    // xorriso
    XORRISO_PARTITION_OFFSET, XORRISO_FS_FLAGS,
};
use crate::component::custom::create_live_overlay_at;
use crate::process::Cmd;

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
    initramfs: PathBuf,
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
            initramfs: output_dir.join(distro_spec::levitate::INITRAMFS_OUTPUT),
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

    // Stage 1: Validate inputs
    validate_iso_inputs(&paths)?;
    let kernel_path = find_kernel(&paths)?;

    // Stage 2: Create live overlay (autologin, serial console, empty root password)
    // This is ONLY applied during live boot, NOT extracted to installed systems
    create_live_overlay_at(&paths.output_dir)?;

    // Stage 3: Set up ISO directory structure
    setup_iso_structure(&paths)?;

    // Stage 4: Copy boot files and artifacts (including live overlay)
    copy_iso_artifacts(&paths, &kernel_path)?;

    // Stage 5: Set up UEFI boot
    setup_uefi_boot(&paths)?;

    // Stage 6: Create the ISO
    run_xorriso(&paths)?;

    // Stage 7: Generate checksum for download verification
    generate_iso_checksum(&paths.iso_output)?;

    print_iso_summary(&paths.iso_output);
    Ok(())
}

/// Stage 1: Validate that required input files exist.
fn validate_iso_inputs(paths: &IsoPaths) -> Result<()> {
    if !paths.squashfs.exists() {
        bail!(
            "Squashfs not found at {}.\n\
             Run 'leviso build squashfs' first.",
            paths.squashfs.display()
        );
    }

    if !paths.initramfs.exists() {
        bail!(
            "Tiny initramfs not found at {}.\n\
             Run 'leviso build initramfs' first.",
            paths.initramfs.display()
        );
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
fn setup_iso_structure(paths: &IsoPaths) -> Result<()> {
    if paths.iso_root.exists() {
        fs::remove_dir_all(&paths.iso_root)?;
    }

    fs::create_dir_all(paths.iso_root.join(ISO_BOOT_DIR))?;
    fs::create_dir_all(paths.iso_root.join(ISO_LIVE_DIR))?;
    fs::create_dir_all(paths.iso_root.join(ISO_EFI_DIR))?;

    Ok(())
}

/// Stage 4: Copy kernel, initramfs, squashfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths, kernel_path: &Path) -> Result<()> {
    // Copy kernel and initramfs
    fs::copy(kernel_path, paths.iso_root.join(KERNEL_ISO_PATH))?;
    fs::copy(&paths.initramfs, paths.iso_root.join(INITRAMFS_ISO_PATH))?;

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
    let label = iso_label();
    let grub_cfg = format!(
        r#"set default=0
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
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, INITRAMFS_ISO_PATH,
    );
    fs::write(paths.iso_root.join(ISO_EFI_DIR).join("grub.cfg"), grub_cfg)?;

    // Create EFI boot image
    let efiboot_img = paths.output_dir.join(EFIBOOT_FILENAME);
    create_efi_boot_image(&paths.iso_root, &efiboot_img)?;

    Ok(())
}

/// Stage 6: Run xorriso to create the final ISO.
fn run_xorriso(paths: &IsoPaths) -> Result<()> {
    println!("Creating UEFI bootable ISO with xorriso...");
    let label = iso_label();

    Cmd::new("xorriso")
        .args(["-as", "mkisofs", "-o"])
        .arg_path(&paths.iso_output)
        .args(["-V", &label]) // CRITICAL: Volume label for device detection
        .args(["-partition_offset", &XORRISO_PARTITION_OFFSET.to_string()])
        .args(XORRISO_FS_FLAGS)
        .args(["-e", EFIBOOT_FILENAME, "-no-emul-boot", "-isohybrid-gpt-basdat"])
        .arg_path(&paths.iso_root)
        .error_msg("xorriso failed. Install: sudo dnf install xorriso")
        .run()?;

    Ok(())
}

/// Stage 7: Generate SHA512 checksum for download verification.
///
/// Writes checksum in standard format: "<hash>  <filename>" (two spaces)
/// Uses just the filename (not full path) so users can verify with:
///   cd output && sha512sum -c levitateos.iso.sha512
fn generate_iso_checksum(iso_path: &Path) -> Result<()> {
    println!("Generating SHA512 checksum...");

    let result = Cmd::new("sha512sum")
        .arg_path(iso_path)
        .error_msg("sha512sum failed. Install: sudo dnf install coreutils")
        .run()?;

    // Extract hash and replace full path with just filename
    // sha512sum outputs: "<hash>  <full_path>"
    // We want: "<hash>  <filename>"
    let hash = result
        .stdout
        .split_whitespace()
        .next()
        .context("Could not parse sha512sum output - no hash found")?;

    let filename = iso_path
        .file_name()
        .context("Could not get ISO filename")?
        .to_string_lossy();

    // Standard format: "<hash>  <filename>" (two spaces between hash and filename)
    let checksum_content = format!("{}{}{}\n", hash, SHA512_SEPARATOR, filename);

    let checksum_path = iso_path.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    fs::write(&checksum_path, &checksum_content)?;

    // Print abbreviated hash for visual confirmation
    if hash.len() >= 16 {
        println!(
            "  SHA512: {}...{}",
            &hash[..8],
            &hash[hash.len() - 8..]
        );
    }
    println!("  Wrote: {}", checksum_path.display());

    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== Squashfs ISO Created ===");
    println!("  Output: {}", iso_output.display());
    if let Ok(meta) = fs::metadata(iso_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }
    println!("  Label: {}", iso_label());
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}

/// Create a FAT16 image containing EFI boot files
fn create_efi_boot_image(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    // Create a FAT image file (16MB for FAT16 minimum + space for EFI files)
    let efiboot_str = efiboot_img.to_string_lossy();

    // Create empty file
    Cmd::new("dd")
        .args(["if=/dev/zero", &format!("of={}", efiboot_str)])
        .args(["bs=1M", &format!("count={}", EFIBOOT_SIZE_MB)])
        .error_msg("Failed to create efiboot.img with dd")
        .run()?;

    // Format as FAT16
    Cmd::new("mkfs.fat")
        .args(["-F", "16"])
        .arg_path(efiboot_img)
        .error_msg("mkfs.fat failed. Install: sudo dnf install dosfstools")
        .run()?;

    // Create EFI/BOOT directory structure using mtools
    Cmd::new("mmd")
        .args(["-i", &efiboot_str, "::EFI"])
        .error_msg("mmd failed. Install: sudo dnf install mtools")
        .run()?;

    Cmd::new("mmd")
        .args(["-i", &efiboot_str, "::EFI/BOOT"])
        .error_msg("mmd failed to create ::EFI/BOOT directory")
        .run()?;

    // Copy EFI files - these must succeed for UEFI boot to work
    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy BOOTX64.EFI")
        .run()?;

    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join(ISO_EFI_DIR).join(EFI_GRUB))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy grubx64.efi")
        .run()?;

    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join(ISO_EFI_DIR).join("grub.cfg"))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy grub.cfg")
        .run()?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join(EFIBOOT_FILENAME))?;

    Ok(())
}
