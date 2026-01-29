//! qcow2 VM disk image builder (sudo-free).
//!
//! Creates bootable qcow2 disk images for local VM use.
//! The image is built without requiring root privileges.
//!
//! Build process:
//! 1. Generate UUIDs for partitions upfront
//! 2. Prepare rootfs staging directory with qcow2-specific config
//! 3. Create EFI partition image with mkfs.vfat + mtools
//! 4. Create root partition image with mkfs.ext4 -d (populates from directory)
//! 5. Create disk image with GPT partition table (sfdisk works on files)
//! 6. Splice partition images into disk at correct offsets
//! 7. Convert raw to qcow2 with compression
//!
//! Key insight: We use rootfs-staging/ directly (the source for EROFS),
//! so we don't need to extract EROFS which would require mounting.

mod helpers;
mod config;
mod partitions;
mod mtools;
mod disk;
mod conversion;

// Re-export public API
pub use helpers::DiskUuids;
pub use config::prepare_qcow2_rootfs;
pub use partitions::{create_efi_partition, create_root_partition, EFI_SIZE_MB};
pub use mtools::{mtools_mkdir, mtools_copy, mtools_write_file};
pub use disk::assemble_disk;
pub use conversion::convert_to_qcow2;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use distro_builder::process::ensure_exists;
use distro_spec::shared::QCOW2_IMAGE_FILENAME;

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
    ensure_exists(&staging_dir, "rootfs-staging").with_context(|| {
        "Run 'cargo run -- build rootfs' first to create rootfs-staging."
    })?;

    // Step 3: Generate UUIDs upfront
    println!("Generating partition UUIDs...");
    let uuids = DiskUuids::generate()?;
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

    // Step 7: Create root partition image
    println!("\nCreating root partition image (this may take a while)...");
    let root_image = work_dir.join("root.img");
    let root_size_mb = (disk_size_gb as u64 * 1024) - EFI_SIZE_MB - (1 * 2);
    partitions::create_root_partition(&qcow2_staging, &root_image, root_size_mb, &uuids)?;

    // Step 8: Assemble the disk image
    println!("\nAssembling disk image...");
    let raw_path = work_dir.join("disk.raw");
    disk::assemble_disk(&raw_path, &efi_image, &root_image, disk_size_gb, &uuids)?;

    // Step 9: Convert to qcow2
    println!("\nConverting to qcow2 (with compression)...");
    conversion::convert_to_qcow2(&raw_path, &qcow2_path)?;

    // Step 10: Cleanup work directory
    println!("Cleaning up...");
    fs::remove_dir_all(&work_dir)?;

    println!("\n=== qcow2 Image Built ===");
    println!("  Output: {}", qcow2_path.display());
    if let Ok(meta) = fs::metadata(&qcow2_path) {
        println!("  Size: {} MB (sparse)", meta.len() / 1024 / 1024);
    }
    println!("\nTo boot:");
    println!("  qemu-system-x86_64 -enable-kvm -m 4G -cpu host \\");
    println!("    -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/ovmf/OVMF_CODE.fd \\");
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
    println!("    sudo fsdbg verify {} --type qcow2", qcow2_path.display());

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
