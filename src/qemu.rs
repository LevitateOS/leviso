use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Find OVMF firmware for UEFI boot
fn find_ovmf() -> Option<PathBuf> {
    // Common OVMF locations across distros
    let candidates = [
        // Fedora/RHEL
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        // Debian/Ubuntu
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/qemu/OVMF.fd",
        // Arch
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
        // NixOS
        "/run/libvirt/nix-ovmf/OVMF_CODE.fd",
    ];

    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Direct kernel boot (bypasses ISO/bootloader for faster debugging)
pub fn test_direct(base_dir: &Path, gui: bool) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let output_dir = base_dir.join("output");

    // Find kernel
    let kernel_path = downloads_dir.join("iso-contents/images/pxeboot/vmlinuz");
    if !kernel_path.exists() {
        bail!(
            "Kernel not found at {}. Run 'leviso extract' first.",
            kernel_path.display()
        );
    }

    // Find initramfs
    let initramfs_path = output_dir.join("initramfs.cpio.gz");
    if !initramfs_path.exists() {
        bail!(
            "Initramfs not found at {}. Run 'leviso initramfs' first.",
            initramfs_path.display()
        );
    }

    println!("Direct kernel boot (bypasses ISO/bootloader)");
    println!("  Kernel:    {}", kernel_path.display());
    println!("  Initramfs: {}", initramfs_path.display());

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-cpu",
        "Skylake-Client",
        "-m",
        "512M",
        "-kernel",
        kernel_path.to_str().unwrap(),
        "-initrd",
        initramfs_path.to_str().unwrap(),
        "-append",
        "console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30",
    ]);

    if gui {
        println!("Running with GUI window");
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        cmd.args(["-nographic", "-serial", "mon:stdio"]);
    }

    let status = cmd
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}

pub fn test_qemu(base_dir: &Path, gui: bool, force_bios: bool) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join("leviso.iso");

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso build' or 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("Starting QEMU with {}...", iso_path.display());

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-cpu",
        "Skylake-Client",
        "-cdrom",
        iso_path.to_str().unwrap(),
        "-m",
        "512M",
    ]);

    // UEFI boot by default (it's 2026), unless --bios is specified
    let use_uefi = if force_bios {
        println!("Using BIOS boot (--bios flag)");
        false
    } else if let Some(ovmf_path) = find_ovmf() {
        println!("Using UEFI boot with {}", ovmf_path.display());
        cmd.args([
            "-drive",
            &format!(
                "if=pflash,format=raw,readonly=on,file={}",
                ovmf_path.display()
            ),
        ]);
        true
    } else {
        println!("OVMF not found, falling back to BIOS boot");
        println!("  Install OVMF for UEFI testing (e.g., 'dnf install edk2-ovmf')");
        false
    };

    if gui {
        println!("Running with GUI window ({})", if use_uefi { "UEFI" } else { "BIOS" });
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        cmd.args(["-nographic", "-serial", "mon:stdio"]);
    }

    let status = cmd
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
