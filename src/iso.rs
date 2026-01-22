use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::build;
use crate::common::binary::copy_dir_recursive;

/// ISO volume label - used for boot device detection.
const ISO_LABEL: &str = "LEVITATEOS";

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
    build::systemd::create_live_overlay(&paths.output_dir)?;

    // Stage 3: Set up ISO directory structure
    setup_iso_structure(&paths)?;

    // Stage 4: Copy boot files and artifacts (including live overlay)
    copy_iso_artifacts(&paths, &kernel_path)?;

    // Stage 5: Set up UEFI boot
    setup_uefi_boot(&paths)?;

    // Stage 6: Create the ISO
    run_xorriso(&paths)?;

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

/// Find the kernel to use (LevitateOS or Rocky fallback).
fn find_kernel(paths: &IsoPaths) -> Result<PathBuf> {
    let levitate_kernel = paths.output_dir.join("staging/boot/vmlinuz");
    if levitate_kernel.exists() {
        println!("Using LevitateOS kernel: {}", levitate_kernel.display());
        return Ok(levitate_kernel);
    }

    let rocky_kernel = paths.iso_contents.join("images/pxeboot/vmlinuz");
    if rocky_kernel.exists() {
        println!("Using Rocky kernel (fallback): {}", rocky_kernel.display());
        return Ok(rocky_kernel);
    }

    bail!(
        "No kernel found.\n\
         Build LevitateOS kernel: leviso build kernel\n\
         Or extract Rocky ISO: leviso extract rocky"
    );
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
        ISO_LABEL, ISO_LABEL, ISO_LABEL
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
    let status = Command::new("xorriso")
        .args([
            "-as",
            "mkisofs",
            "-o",
            paths.iso_output.to_str().unwrap(),
            "-V",
            ISO_LABEL, // CRITICAL: Volume label for device detection
            "-partition_offset",
            "16",
            "-full-iso9660-filenames",
            "-joliet",
            "-rational-rock",
            "-e",
            "efiboot.img",
            "-no-emul-boot",
            "-isohybrid-gpt-basdat",
            paths.iso_root.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run xorriso. Is xorriso installed?")?;

    if !status.success() {
        bail!("xorriso failed");
    }

    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== Squashfs ISO Created ===");
    println!("  Output: {}", iso_output.display());
    if let Ok(meta) = fs::metadata(iso_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }
    println!("  Label: {}", ISO_LABEL);
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}

/// Create a FAT16 image containing EFI boot files
fn create_efi_boot_image(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    // Create a FAT image file (16MB for FAT16 minimum + space for EFI files)
    let size_mb = 16;

    // Create empty file
    let status = Command::new("dd")
        .args([
            "if=/dev/zero",
            &format!("of={}", efiboot_img.to_str().unwrap()),
            "bs=1M",
            &format!("count={}", size_mb),
        ])
        .status()
        .context("Failed to create efiboot.img")?;

    if !status.success() {
        bail!("dd failed");
    }

    // Format as FAT16
    let status = Command::new("mkfs.fat")
        .args(["-F", "16", efiboot_img.to_str().unwrap()])
        .status()
        .context("Failed to format efiboot.img. Is dosfstools installed?")?;

    if !status.success() {
        bail!("mkfs.fat failed");
    }

    // Create EFI/BOOT directory structure using mtools
    let status = Command::new("mmd")
        .args(["-i", efiboot_img.to_str().unwrap(), "::EFI"])
        .status()
        .context(
            "mtools (mmd) not found. Install mtools:\n\
             - Fedora: sudo dnf install mtools\n\
             - Ubuntu: sudo apt install mtools\n\
             - Arch: sudo pacman -S mtools",
        )?;

    if !status.success() {
        bail!("mmd failed to create ::EFI directory in efiboot.img");
    }

    let status = Command::new("mmd")
        .args(["-i", efiboot_img.to_str().unwrap(), "::EFI/BOOT"])
        .status()?;

    if !status.success() {
        bail!("mmd failed to create ::EFI/BOOT directory in efiboot.img");
    }

    // Copy EFI files - these must succeed for UEFI boot to work
    let status = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/BOOTX64.EFI").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status()?;

    if !status.success() {
        bail!("mcopy failed to copy BOOTX64.EFI to efiboot.img");
    }

    let status = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grubx64.efi").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status()?;

    if !status.success() {
        bail!("mcopy failed to copy grubx64.efi to efiboot.img");
    }

    let status = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grub.cfg").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status()?;

    if !status.success() {
        bail!("mcopy failed to copy grub.cfg to efiboot.img");
    }

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join("efiboot.img"))?;

    Ok(())
}
