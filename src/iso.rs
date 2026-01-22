use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// ISO volume label - used for boot device detection.
const ISO_LABEL: &str = "LEVITATEOS";

/// Create ISO using squashfs-based architecture (new).
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts squashfs + overlay
/// - Squashfs image (~350MB) - complete system
///
/// Boot flow: kernel -> tiny initramfs -> mount squashfs -> overlay -> switch_root -> systemd
pub fn create_squashfs_iso(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_contents = extract_dir.join("iso-contents");
    let output_dir = base_dir.join("output");
    let squashfs = output_dir.join("filesystem.squashfs");
    let initramfs = output_dir.join("initramfs-tiny.cpio.gz");
    let iso_output = output_dir.join("levitateos.iso");

    // Verify required files exist
    if !squashfs.exists() {
        bail!(
            "Squashfs not found at {}.\n\
             Run 'leviso build squashfs' first.",
            squashfs.display()
        );
    }

    if !initramfs.exists() {
        bail!(
            "Tiny initramfs not found at {}.\n\
             Run 'leviso build initramfs' first.",
            initramfs.display()
        );
    }

    // Find kernel - prefer built LevitateOS kernel, fall back to Rocky
    let levitate_kernel = output_dir.join("staging/boot/vmlinuz");
    let kernel_path = if levitate_kernel.exists() {
        println!("Using LevitateOS kernel: {}", levitate_kernel.display());
        levitate_kernel
    } else {
        let rocky_kernel = iso_contents.join("images/pxeboot/vmlinuz");
        if !rocky_kernel.exists() {
            bail!(
                "No kernel found.\n\
                 Build LevitateOS kernel: leviso build kernel\n\
                 Or extract Rocky ISO: leviso extract rocky"
            );
        }
        println!("Using Rocky kernel (fallback): {}", rocky_kernel.display());
        rocky_kernel
    };

    // Create ISO directory structure
    let iso_root = output_dir.join("iso-root");
    if iso_root.exists() {
        fs::remove_dir_all(&iso_root)?;
    }

    fs::create_dir_all(iso_root.join("boot"))?;
    fs::create_dir_all(iso_root.join("live"))?; // New: squashfs goes here
    fs::create_dir_all(iso_root.join("EFI/BOOT"))?;

    // Copy kernel and initramfs
    fs::copy(&kernel_path, iso_root.join("boot/vmlinuz"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.img"))?;

    // Copy squashfs to /live/
    println!("Copying squashfs to ISO...");
    fs::copy(&squashfs, iso_root.join("live/filesystem.squashfs"))?;

    // Set up UEFI boot
    let efi_src = iso_contents.join("EFI/BOOT");
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
        iso_root.join("EFI/BOOT/BOOTX64.EFI"),
    )?;
    fs::copy(
        efi_src.join("grubx64.efi"),
        iso_root.join("EFI/BOOT/grubx64.efi"),
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
    fs::write(iso_root.join("EFI/BOOT/grub.cfg"), grub_cfg)?;

    // Create EFI boot image
    let efiboot_img = output_dir.join("efiboot.img");
    create_efi_boot_image(&iso_root, &efiboot_img)?;

    // Create ISO with volume label
    println!("Creating UEFI bootable ISO with xorriso...");
    let status = Command::new("xorriso")
        .args([
            "-as",
            "mkisofs",
            "-o",
            iso_output.to_str().unwrap(),
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
            iso_root.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run xorriso. Is xorriso installed?")?;

    if !status.success() {
        bail!("xorriso failed");
    }

    println!("\n=== Squashfs ISO Created ===");
    println!("  Output: {}", iso_output.display());
    if let Ok(meta) = fs::metadata(&iso_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }
    println!("  Label: {}", ISO_LABEL);
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");

    Ok(())
}

/// Create ISO using initramfs-based architecture (legacy).
///
/// This is the original method where the initramfs contains the complete system.
/// Kept for reference but not actively used - squashfs ISO is the default.
#[allow(dead_code)]
pub fn create_iso(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_contents = extract_dir.join("iso-contents");
    let output_dir = base_dir.join("output");
    let initramfs = output_dir.join("initramfs.cpio.gz");
    let iso_output = output_dir.join("levitateos.iso");

    if !initramfs.exists() {
        bail!("Initramfs not found. Run 'leviso initramfs' first.");
    }

    // Find kernel - prefer built LevitateOS kernel, fall back to Rocky
    let levitate_kernel = output_dir.join("staging/boot/vmlinuz");
    let kernel_path = if levitate_kernel.exists() {
        println!("Using LevitateOS kernel: {}", levitate_kernel.display());
        levitate_kernel
    } else {
        // Fall back to Rocky's kernel (only pxeboot path - no isolinux)
        let rocky_kernel = iso_contents.join("images/pxeboot/vmlinuz");

        if !rocky_kernel.exists() {
            bail!(
                "No kernel found.\n\
                 Build LevitateOS kernel: leviso build kernel\n\
                 Or extract Rocky ISO: leviso extract rocky"
            );
        }

        println!("Using Rocky kernel (fallback): {}", rocky_kernel.display());
        println!("  Tip: Run 'leviso build kernel' to build LevitateOS kernel");
        rocky_kernel
    };

    // Create ISO directory structure (UEFI only - no isolinux)
    let iso_root = output_dir.join("iso-root");
    if iso_root.exists() {
        fs::remove_dir_all(&iso_root)?;
    }

    fs::create_dir_all(iso_root.join("boot"))?;
    fs::create_dir_all(iso_root.join("EFI/BOOT"))?;

    // Copy kernel and initramfs
    fs::copy(&kernel_path, iso_root.join("boot/vmlinuz"))?;
    fs::copy(&initramfs, iso_root.join("boot/initramfs.img"))?;

    // Copy base tarball if it exists
    let base_tarball = output_dir.join("levitateos-base.tar.xz");

    if base_tarball.exists() {
        println!("Copying base tarball from: {}", base_tarball.display());
        fs::copy(&base_tarball, iso_root.join("levitateos-base.tar.xz"))?;
        println!("  Copied to ISO root as levitateos-base.tar.xz");
    } else {
        println!("Warning: base tarball not found. Installation will not work.");
        println!("  Build it with: cargo run -- build rootfs");
    }

    // === UEFI Boot Setup (GRUB EFI) ===
    let efi_src = iso_contents.join("EFI/BOOT");
    if !efi_src.exists() {
        bail!(
            "EFI boot files not found at {}.\n\
             LevitateOS requires UEFI boot. Run 'leviso extract rocky' first.",
            efi_src.display()
        );
    }

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

    // Create GRUB config with normal boot and emergency shell options
    let grub_cfg = r#"set default=0
set timeout=5

menuentry 'LevitateOS' {
    linuxefi /boot/vmlinuz console=ttyS0,115200n8 console=tty0 earlyprintk=ttyS0,115200 panic=30 rdinit=/init
    initrdefi /boot/initramfs.img
}

menuentry 'LevitateOS (Emergency Shell)' {
    linuxefi /boot/vmlinuz console=ttyS0,115200n8 console=tty0 earlyprintk=ttyS0,115200 emergency rdinit=/init
    initrdefi /boot/initramfs.img
}
"#;
    fs::write(iso_root.join("EFI/BOOT/grub.cfg"), grub_cfg)?;

    // Create EFI boot image (efiboot.img)
    let efiboot_img = output_dir.join("efiboot.img");
    create_efi_boot_image(&iso_root, &efiboot_img)?;

    // Create UEFI-only bootable ISO with xorriso
    println!("Creating UEFI bootable ISO with xorriso...");
    let status = Command::new("xorriso")
        .args([
            "-as",
            "mkisofs",
            "-o",
            iso_output.to_str().unwrap(),
            "-V",
            ISO_LABEL, // Volume label
            "-partition_offset",
            "16",
            "-full-iso9660-filenames",
            "-joliet",
            "-rational-rock",
            "-e",
            "efiboot.img",
            "-no-emul-boot",
            "-isohybrid-gpt-basdat",
            iso_root.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run xorriso. Is xorriso installed?")?;

    if !status.success() {
        bail!("xorriso failed");
    }

    println!("Created ISO at: {}", iso_output.display());
    println!("\nTo run in QEMU (UEFI):");
    println!("  cargo run -- run");

    Ok(())
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
        .status();

    if status.is_err() || !status.unwrap().success() {
        println!("Note: mtools not fully available, using alternative method");
    }

    let _ = Command::new("mmd")
        .args(["-i", efiboot_img.to_str().unwrap(), "::EFI/BOOT"])
        .status();

    // Copy EFI files
    let _ = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/BOOTX64.EFI").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    let _ = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grubx64.efi").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    let _ = Command::new("mcopy")
        .args([
            "-i",
            efiboot_img.to_str().unwrap(),
            iso_root.join("EFI/BOOT/grub.cfg").to_str().unwrap(),
            "::EFI/BOOT/",
        ])
        .status();

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join("efiboot.img"))?;

    Ok(())
}
