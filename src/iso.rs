use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::download::download_syslinux;

pub fn create_iso(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_contents = extract_dir.join("iso-contents");
    let output_dir = base_dir.join("output");
    let initramfs = output_dir.join("initramfs.cpio.gz");
    let iso_output = output_dir.join("leviso.iso");

    if !initramfs.exists() {
        bail!("Initramfs not found. Run 'leviso initramfs' first.");
    }

    // Download syslinux for BIOS boot
    let syslinux_dir = download_syslinux(base_dir)?;

    // Find kernel from Rocky
    let kernel_candidates = [
        iso_contents.join("images/pxeboot/vmlinuz"),
        iso_contents.join("isolinux/vmlinuz"),
    ];

    let kernel_path = kernel_candidates
        .iter()
        .find(|p| p.exists())
        .context("Could not find kernel in Rocky ISO")?;

    println!("Using kernel: {}", kernel_path.display());

    // Create ISO directory structure
    let iso_root = output_dir.join("iso-root");
    if iso_root.exists() {
        fs::remove_dir_all(&iso_root)?;
    }

    fs::create_dir_all(iso_root.join("isolinux"))?;
    fs::create_dir_all(iso_root.join("boot"))?;
    fs::create_dir_all(iso_root.join("EFI/BOOT"))?;

    // Copy kernel and initramfs
    fs::copy(kernel_path, iso_root.join("boot/vmlinuz"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.img"))?;

    // Copy base tarball if it exists
    let base_tarball = output_dir.join("levitateos-base.tar.xz");

    if base_tarball.exists() {
        println!("Copying base tarball from: {}", base_tarball.display());
        fs::copy(&base_tarball, iso_root.join("levitateos-base.tar.xz"))?;
        println!("  Copied to ISO root as levitateos-base.tar.xz");
    } else {
        println!("Warning: base tarball not found. Installation will not work.");
        println!("  Build it with: cargo run -- rootfs");
    }

    // === BIOS Boot Setup (isolinux) ===
    fs::copy(
        syslinux_dir.join("bios/core/isolinux.bin"),
        iso_root.join("isolinux/isolinux.bin"),
    )?;
    fs::copy(
        syslinux_dir.join("bios/com32/elflink/ldlinux/ldlinux.c32"),
        iso_root.join("isolinux/ldlinux.c32"),
    )?;

    let isolinux_cfg = r#"DEFAULT leviso
TIMEOUT 30
PROMPT 1

LABEL leviso
    MENU LABEL Leviso
    LINUX /boot/vmlinuz
    INITRD /boot/initramfs.img
    APPEND console=ttyS0,115200n8 console=tty0 earlyprintk=ttyS0,115200 panic=30 rdinit=/init
"#;
    fs::write(iso_root.join("isolinux/isolinux.cfg"), isolinux_cfg)?;

    // === UEFI Boot Setup (GRUB EFI) ===
    let efi_src = iso_contents.join("EFI/BOOT");
    if efi_src.exists() {
        println!("Setting up UEFI boot...");

        // Copy GRUB EFI bootloader
        fs::copy(
            efi_src.join("BOOTX64.EFI"),
            iso_root.join("EFI/BOOT/BOOTX64.EFI"),
        )?;
        fs::copy(
            efi_src.join("grubx64.efi"),
            iso_root.join("EFI/BOOT/grubx64.efi"),
        )?;

        // Create GRUB config for Leviso
        let grub_cfg = r#"set default=0
set timeout=5

menuentry 'Leviso' {
    linuxefi /boot/vmlinuz console=ttyS0,115200n8 console=tty0 earlyprintk=ttyS0,115200 panic=30 rdinit=/init
    initrdefi /boot/initramfs.img
}
"#;
        fs::write(iso_root.join("EFI/BOOT/grub.cfg"), grub_cfg)?;

        // Create EFI boot image (efiboot.img)
        let efiboot_img = output_dir.join("efiboot.img");
        create_efi_boot_image(&iso_root, &efiboot_img)?;

        // Create hybrid ISO with both BIOS and UEFI boot
        println!("Creating hybrid BIOS/UEFI bootable ISO with xorriso...");
        let status = Command::new("xorriso")
            .args([
                "-as", "mkisofs",
                "-o", iso_output.to_str().unwrap(),
                // BIOS boot
                "-isohybrid-mbr", syslinux_dir.join("bios/mbr/isohdpfx.bin").to_str().unwrap(),
                "-c", "isolinux/boot.cat",
                "-b", "isolinux/isolinux.bin",
                "-no-emul-boot",
                "-boot-load-size", "4",
                "-boot-info-table",
                // UEFI boot
                "-eltorito-alt-boot",
                "-e", "efiboot.img",
                "-no-emul-boot",
                "-isohybrid-gpt-basdat",
                // Source
                iso_root.to_str().unwrap(),
            ])
            .status()
            .context("Failed to run xorriso")?;

        if !status.success() {
            bail!("xorriso failed");
        }
    } else {
        // Fallback: BIOS-only boot
        println!("EFI files not found, creating BIOS-only bootable ISO...");
        let status = Command::new("xorriso")
            .args([
                "-as", "mkisofs",
                "-o", iso_output.to_str().unwrap(),
                "-isohybrid-mbr", syslinux_dir.join("bios/mbr/isohdpfx.bin").to_str().unwrap(),
                "-c", "isolinux/boot.cat",
                "-b", "isolinux/isolinux.bin",
                "-no-emul-boot",
                "-boot-load-size", "4",
                "-boot-info-table",
                iso_root.to_str().unwrap(),
            ])
            .status()
            .context("Failed to run xorriso")?;

        if !status.success() {
            bail!("xorriso failed");
        }
    }

    println!("Created ISO at: {}", iso_output.display());
    println!("\nTo run in QEMU GUI (UEFI by default):");
    println!("  cargo run -- run");
    println!("\nTo force BIOS boot:");
    println!("  cargo run -- run --bios");

    Ok(())
}

/// Create a FAT12/16 image containing EFI boot files
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

    // Format as FAT16 (FAT12 can't handle files >4MB well)
    let status = Command::new("mkfs.fat")
        .args(["-F", "16", efiboot_img.to_str().unwrap()])
        .status()
        .context("Failed to format efiboot.img")?;

    if !status.success() {
        bail!("mkfs.fat failed");
    }

    // Mount and copy EFI files using mtools (no root required)
    // Create EFI/BOOT directory structure
    let status = Command::new("mmd")
        .args(["-i", efiboot_img.to_str().unwrap(), "::EFI"])
        .status();

    if status.is_err() || !status.unwrap().success() {
        // mtools not available, try mcopy directly
        println!("Note: mtools not fully available, using alternative method");
    }

    let _ = Command::new("mmd")
        .args(["-i", efiboot_img.to_str().unwrap(), "::EFI/BOOT"])
        .status();

    // Copy EFI files
    let _ = Command::new("mcopy")
        .args([
            "-i", efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/BOOTX64.EFI").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    let _ = Command::new("mcopy")
        .args([
            "-i", efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grubx64.efi").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    let _ = Command::new("mcopy")
        .args([
            "-i", efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grub.cfg").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join("efiboot.img"))?;

    Ok(())
}
