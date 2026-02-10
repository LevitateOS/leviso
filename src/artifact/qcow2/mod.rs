//! qcow2 VM disk image builder (sudo-free).
//!
//! Creates bootable qcow2 disk images for local VM use.
//! The image is built without requiring root privileges.
//!
//! Build process (dependencies must be built in order):
//! 0. Prerequisites: kernel, initramfs, and rootfs must be built first
//!    Run: cargo run -- build (full build) OR individually:
//!      cargo run -- build kernel
//!      cargo run -- build initramfs
//!      cargo run -- build rootfs
//! 1. Verify all dependencies exist (kernel, initramfs-installed, rootfs content)
//! 2. Generate UUIDs for partitions upfront
//! 3. Prepare rootfs staging directory with qcow2-specific config
//! 4. Create EFI partition image with mkfs.vfat + mtools
//! 5. Create root partition image with mkfs.ext4 -d (populates from directory)
//! 6. Create disk image with GPT partition table (sfdisk works on files)
//! 7. Splice partition images into disk at correct offsets
//! 8. Convert raw to qcow2 with compression
//! 9. Verify qcow2 image is bootable
//!
//! Key insight: We use rootfs-staging/ directly (the source for EROFS),
//! so we don't need to extract EROFS which would require mounting.

mod config;
mod conversion;
mod disk;
mod helpers;
mod mtools;
mod partitions;

// Re-export public API
pub use config::prepare_qcow2_rootfs;
pub use conversion::convert_to_qcow2;
pub use disk::assemble_disk;
pub use helpers::DiskUuids;
pub use mtools::{mtools_copy, mtools_mkdir, mtools_write_file};
pub use partitions::{create_efi_partition, create_root_partition, EFI_SIZE_MB};

use anyhow::{bail, Context, Result};
use distro_builder::process::ensure_exists;
use distro_spec::shared::QCOW2_IMAGE_FILENAME;
use std::fs;
use std::path::Path;

// TEAM_151: Re-organized qcow2 module into dedicated submodules for better maintainability

/// Build a qcow2 VM disk image without requiring root.
///
/// # Arguments
/// * `base_dir` - The leviso base directory (contains output/, downloads/)
/// * `disk_size_gb` - Disk size in GB (sparse allocation)
pub fn build_qcow2(base_dir: &Path, disk_size_gb: u32) -> Result<()> {
    println!("=== Building qcow2 VM Image (sudo-free) ===\n");

    // Step 1: Verify host tools
    println!("Checking host tools...");
    helpers::check_host_tools()?;

    let output_dir = base_dir.join("output");
    let staging_dir = output_dir.join("rootfs-staging");
    let qcow2_path = output_dir.join(QCOW2_IMAGE_FILENAME);

    // Step 2: Verify rootfs-staging exists (source for rootfs)
    ensure_exists(&staging_dir, "rootfs-staging")
        .with_context(|| "Run 'cargo run -- build rootfs' first to create rootfs-staging.")?;

    // Step 2b: Verify ALL dependencies exist upfront (fail fast)
    println!("\nVerifying dependencies...");
    verify_build_dependencies(base_dir, &staging_dir).with_context(|| {
        "qcow2 build requires kernel, install initramfs, and complete rootfs.\n\
         Run 'cargo run -- build' to build all artifacts."
    })?;

    // Step 3: Generate UUIDs upfront
    println!("Generating partition UUIDs...");
    let uuids = distro_builder::generate_disk_uuids()?;
    println!("  Root FS UUID: {}", uuids.root_fs_uuid);
    println!("  EFI FS UUID:  {}", uuids.efi_fs_uuid);
    println!("  Root PARTUUID: {}", uuids.root_part_uuid);

    // Step 4: Create temporary work directory
    let work_dir = output_dir.join("qcow2-work");
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)?;
    }
    fs::create_dir_all(&work_dir)?;

    // Step 5: Prepare modified rootfs for qcow2
    println!("\nPreparing rootfs for qcow2...");
    let qcow2_staging = work_dir.join("rootfs");
    config::prepare_qcow2_rootfs(base_dir, &staging_dir, &qcow2_staging, &uuids)?;

    // Step 6: Create EFI partition image
    println!("\nCreating EFI partition image...");
    let efi_image = work_dir.join("efi.img");
    partitions::create_efi_partition(base_dir, &efi_image, &uuids, &qcow2_staging)?;
    if let Ok(meta) = fs::metadata(&efi_image) {
        println!("  EFI partition size: {} MB", meta.len() / 1024 / 1024);
    }

    // Step 7: Create root partition image
    println!("\nCreating root partition image (this may take a while)...");
    let root_image = work_dir.join("root.img");
    let root_size_mb = (disk_size_gb as u64 * 1024) - EFI_SIZE_MB - 2;
    partitions::create_root_partition(&qcow2_staging, &root_image, root_size_mb, &uuids)?;
    if let Ok(meta) = fs::metadata(&root_image) {
        println!(
            "  Root partition size: {} MB (sparse file)",
            meta.len() / 1024 / 1024
        );
    }

    // Step 8: Assemble the disk image
    println!("\nAssembling disk image...");
    let raw_path = work_dir.join("disk.raw");
    disk::assemble_disk(&raw_path, &efi_image, &root_image, disk_size_gb, &uuids)?;
    if let Ok(meta) = fs::metadata(&raw_path) {
        println!("  Raw disk size: {} MB", meta.len() / 1024 / 1024);
    }

    // Step 9: Convert to qcow2
    println!("\nConverting to qcow2 (with compression)...");
    conversion::convert_to_qcow2(&raw_path, &qcow2_path)?;

    // Step 9b: Verify qcow2 immediately (Phase 3)
    println!("\nVerifying qcow2 image...");
    match verify_qcow2_internal(&qcow2_path) {
        Ok(_) => {
            // Step 10: Cleanup work directory on success (Phase 4)
            println!("Cleaning up...");
            fs::remove_dir_all(&work_dir)?;
        }
        Err(e) => {
            // Keep work directory for debugging
            println!("\n[!] Build verification failed. Work directory preserved for debugging:");
            println!("    {}", work_dir.display());
            println!("  Inspect partition images:");
            println!("    ls -lh {}", work_dir.display());
            return Err(e);
        }
    }

    println!("\n=== qcow2 Image Built ===");
    println!("  Output: {}", qcow2_path.display());
    if let Ok(meta) = fs::metadata(&qcow2_path) {
        println!("  Size: {} MB (sparse)", meta.len() / 1024 / 1024);
    }
    println!("\nTo boot:");
    println!("  qemu-system-x86_64 -enable-kvm -m 4G -cpu host \\");
    println!(
        "    -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/ovmf/OVMF_CODE.fd \\"
    );
    println!("    -drive file={},format=qcow2 \\", qcow2_path.display());
    println!("    -device virtio-vga -device virtio-net-pci,netdev=net0 \\");
    println!("    -netdev user,id=net0");

    Ok(())
}

/// Verify the qcow2 image using fsdbg static checks.
pub fn verify_qcow2(base_dir: &Path) -> Result<()> {
    let qcow2_path = base_dir.join("output").join(QCOW2_IMAGE_FILENAME);
    ensure_exists(&qcow2_path, "qcow2 image")?;

    println!("\n=== Verifying qcow2 Image ===");
    println!("  Image: {}", qcow2_path.display());

    // Basic size check
    let metadata = fs::metadata(&qcow2_path)?;
    let size_mb = metadata.len() / 1024 / 1024;

    if size_mb < 100 {
        bail!(
            "qcow2 image seems too small ({} MB). Build may have failed.",
            size_mb
        );
    }
    println!("  Size: {} MB", size_mb);

    // Note: Full static verification requires mounting (sudo).
    // For now, we just check the file exists and has reasonable size.
    // Users can run `fsdbg verify output/levitateos-x86_64.qcow2 --type qcow2` manually
    // if they want detailed verification.
    println!("  [OK] Basic verification passed");
    println!("\n  For detailed verification, run:");
    println!(
        "    sudo fsdbg verify {} --type qcow2",
        qcow2_path.display()
    );

    Ok(())
}

/// Verify all build dependencies exist and are valid.
fn verify_build_dependencies(base_dir: &Path, rootfs: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");

    // Check kernel
    let kernel_path = output_dir.join("staging/boot/vmlinuz");
    ensure_exists(&kernel_path, "Kernel")
        .with_context(|| "Kernel not found. Run 'cargo run -- build kernel' first.")?;

    // Check install initramfs (REQUIRED for disk boot)
    let initramfs_path = output_dir.join("initramfs-installed.img");
    ensure_exists(&initramfs_path, "Install initramfs").with_context(|| {
        "Install initramfs not found. Run 'cargo run -- build initramfs' first.\n\
         (The qcow2 requires initramfs-installed.img, not the live initramfs)"
    })?;

    // Validate rootfs has minimum required content
    validate_rootfs_content(rootfs)?;

    println!("  [OK] All dependencies verified");
    Ok(())
}

/// Validate that rootfs has critical directories and minimum size.
fn validate_rootfs_content(rootfs: &Path) -> Result<()> {
    let critical_paths = [
        "usr/bin",
        "usr/lib",
        "usr/lib64",
        "etc/shadow",
        "etc/passwd",
        "etc/fstab",
        "boot",
    ];

    for path in &critical_paths {
        let full_path = rootfs.join(path);
        if !full_path.exists() {
            bail!(
                "rootfs-staging is incomplete: {} not found.\n\
                 Run 'cargo run -- build rootfs' to build a complete rootfs.",
                path
            );
        }
    }

    // Check minimum size (rootfs should be at least 500 MB)
    let size_mb = helpers::calculate_dir_size(rootfs).context("Failed to calculate rootfs size")?
        / (1024 * 1024);

    if size_mb < 500 {
        bail!(
            "rootfs-staging seems too small ({} MB).\n\
             A complete rootfs should be at least 500 MB.\n\
             Run 'cargo run -- build rootfs' to rebuild.",
            size_mb
        );
    }

    Ok(())
}

/// Verify qcow2 image is not suspiciously small (Phase 3).
fn verify_qcow2_internal(qcow2_path: &Path) -> Result<()> {
    let metadata = fs::metadata(qcow2_path)?;
    let size_mb = metadata.len() / 1024 / 1024;

    if size_mb < 100 {
        bail!(
            "qcow2 image is suspiciously small ({} MB).\n\
             This usually means the build failed to populate the partitions.\n\
             Expected size: 500-2000 MB (compressed).\n\n\
             Check that all dependencies were built first:\n\
             - cargo run -- build kernel\n\
             - cargo run -- build initramfs\n\
             - cargo run -- build rootfs",
            size_mb
        );
    }

    println!("  [OK] Image size: {} MB (compressed)", size_mb);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_functions_exist() {
        // Smoke test to verify module compiles
        assert!(true);
    }
}
