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

    // Download syslinux
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

    // Copy syslinux files
    fs::copy(
        syslinux_dir.join("bios/core/isolinux.bin"),
        iso_root.join("isolinux/isolinux.bin"),
    )?;
    fs::copy(
        syslinux_dir.join("bios/com32/elflink/ldlinux/ldlinux.c32"),
        iso_root.join("isolinux/ldlinux.c32"),
    )?;

    // Copy kernel and initramfs
    fs::copy(kernel_path, iso_root.join("boot/vmlinuz"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.img"))?;

    // Create isolinux.cfg
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

    // Create ISO with xorriso
    println!("Creating bootable ISO with xorriso...");
    let status = Command::new("xorriso")
        .args([
            "-as",
            "mkisofs",
            "-o",
            iso_output.to_str().unwrap(),
            "-isohybrid-mbr",
            syslinux_dir
                .join("bios/mbr/isohdpfx.bin")
                .to_str()
                .unwrap(),
            "-c",
            "isolinux/boot.cat",
            "-b",
            "isolinux/isolinux.bin",
            "-no-emul-boot",
            "-boot-load-size",
            "4",
            "-boot-info-table",
            iso_root.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run xorriso")?;

    if !status.success() {
        bail!("xorriso failed");
    }

    println!("Created ISO at: {}", iso_output.display());
    println!("\nTo test, run:");
    println!(
        "  qemu-system-x86_64 -cpu Skylake-Client -cdrom {} -m 512M",
        iso_output.display()
    );

    Ok(())
}
