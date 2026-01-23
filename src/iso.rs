use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::binary::copy_dir_recursive;
use crate::component::custom::create_live_overlay_at;
use crate::process::Cmd;

/// Get ISO volume label from environment or use default.
/// Used for boot device detection (root=LABEL=X).
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| "LEVITATEOS".to_string())
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
            squashfs: output_dir.join("filesystem.squashfs"),
            initramfs: output_dir.join("initramfs-tiny.cpio.gz"),
            iso_output: output_dir.join("levitateos.iso"),
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

    fs::create_dir_all(paths.iso_root.join("boot"))?;
    fs::create_dir_all(paths.iso_root.join("live"))?;
    fs::create_dir_all(paths.iso_root.join("EFI/BOOT"))?;

    Ok(())
}

/// Stage 4: Copy kernel, initramfs, squashfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths, kernel_path: &Path) -> Result<()> {
    // Copy kernel and initramfs
    fs::copy(kernel_path, paths.iso_root.join("boot/vmlinuz"))?;
    fs::copy(&paths.initramfs, paths.iso_root.join("boot/initramfs.img"))?;

    // Copy squashfs to /live/
    println!("Copying squashfs to ISO...");
    fs::copy(&paths.squashfs, paths.iso_root.join("live/filesystem.squashfs"))?;

    // Copy live overlay to /live/overlay/
    // This contains live-specific configs (autologin, serial console, empty root password)
    // that are layered on top of squashfs during live boot only
    let live_overlay_src = paths.output_dir.join("live-overlay");
    let live_overlay_dst = paths.iso_root.join("live/overlay");
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

    // Copy tarball for installation
    // NOTE: This is for the old tarball-based installation method.
    // With squashfs-based installation (recstrap), the tarball is NOT required.
    // recstrap extracts directly from the squashfs.
    // Keeping this for backwards compatibility but it's optional now.
    let base_tarball = paths.output_dir.join("levitateos-base.tar.xz");
    if base_tarball.exists() {
        println!("Copying base tarball for installation...");
        fs::copy(&base_tarball, paths.iso_root.join("levitateos-base.tar.xz"))?;
    }
    // No warning needed - recstrap doesn't use the tarball

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
        efi_src.join("BOOTX64.EFI"),
        paths.iso_root.join("EFI/BOOT/BOOTX64.EFI"),
    )?;
    fs::copy(
        efi_src.join("grubx64.efi"),
        paths.iso_root.join("EFI/BOOT/grubx64.efi"),
    )?;

    // Create GRUB config with root=LABEL for device detection
    // selinux=0 disables SELinux (we don't ship policies)
    let label = iso_label();
    let grub_cfg = format!(
        r#"set default=0
set timeout=5

menuentry 'LevitateOS' {{
    linuxefi /boot/vmlinuz root=LABEL={} console=ttyS0,115200n8 console=tty0 selinux=0
    initrdefi /boot/initramfs.img
}}

menuentry 'LevitateOS (Emergency Shell)' {{
    linuxefi /boot/vmlinuz root=LABEL={} console=ttyS0,115200n8 console=tty0 selinux=0 emergency
    initrdefi /boot/initramfs.img
}}

menuentry 'LevitateOS (Debug)' {{
    linuxefi /boot/vmlinuz root=LABEL={} console=ttyS0,115200n8 console=tty0 selinux=0 debug
    initrdefi /boot/initramfs.img
}}
"#,
        label, label, label
    );
    fs::write(paths.iso_root.join("EFI/BOOT/grub.cfg"), grub_cfg)?;

    // Create EFI boot image
    let efiboot_img = paths.output_dir.join("efiboot.img");
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
        .args(["-partition_offset", "16"])
        .args(["-full-iso9660-filenames", "-joliet", "-rational-rock"])
        .args(["-e", "efiboot.img", "-no-emul-boot", "-isohybrid-gpt-basdat"])
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
    let checksum_content = format!("{}  {}\n", hash, filename);

    let checksum_path = iso_path.with_extension("iso.sha512");
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
    let size_mb = 16;
    let efiboot_str = efiboot_img.to_string_lossy();

    // Create empty file
    Cmd::new("dd")
        .args(["if=/dev/zero", &format!("of={}", efiboot_str)])
        .args(["bs=1M", &format!("count={}", size_mb)])
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
        .arg_path(&iso_root.join("EFI/BOOT/BOOTX64.EFI"))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy BOOTX64.EFI")
        .run()?;

    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join("EFI/BOOT/grubx64.efi"))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy grubx64.efi")
        .run()?;

    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join("EFI/BOOT/grub.cfg"))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy grub.cfg")
        .run()?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join("efiboot.img"))?;

    Ok(())
}
