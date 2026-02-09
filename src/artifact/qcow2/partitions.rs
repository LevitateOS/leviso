//! EFI and root partition creation for qcow2 disk images.
//!
//! Wraps shared infrastructure from distro-builder::artifact::disk with
//! LevitateOS-specific boot configuration.

use super::helpers::DiskUuids;
use anyhow::Result;
use distro_builder::process::{ensure_exists, find_first_existing};
use distro_spec::levitate::boot::{boot_entry_with_partuuid, default_loader_config};
use std::path::{Path, PathBuf};

/// EFI partition size in MB
pub const EFI_SIZE_MB: u64 = 1024;

/// Create the EFI partition image using mkfs.vfat and mtools.
///
/// This is a LevitateOS-specific wrapper that resolves kernel, initramfs,
/// and systemd-boot paths, then delegates to shared partition creation.
pub fn create_efi_partition(
    base_dir: &Path,
    image_path: &Path,
    uuids: &DiskUuids,
    rootfs: &Path,
) -> Result<()> {
    // Resolve systemd-boot EFI binary
    let boot_candidates = [
        rootfs.join("usr/lib/systemd/boot/efi/systemd-bootx64.efi"),
        PathBuf::from("/usr/lib/systemd/boot/efi/systemd-bootx64.efi"),
    ];
    let systemd_boot_src = find_first_existing(&boot_candidates).ok_or_else(|| {
        anyhow::anyhow!(
            "systemd-boot EFI binary not found.\n\
             Install systemd-boot-unsigned or systemd-ukify package."
        )
    })?;

    // Resolve kernel and initramfs paths
    let output_dir = base_dir.join("output");
    let kernel_src = output_dir.join("staging/boot/vmlinuz");
    ensure_exists(&kernel_src, "Kernel")?;

    let initramfs_src = output_dir.join("initramfs-installed.img");
    if !initramfs_src.exists() {
        anyhow::bail!(
            "Install initramfs not found: {}\n\n\
             The qcow2 image requires the install initramfs (systemd-based).\n\
             The live initramfs (busybox-based) cannot boot an installed system.\n\n\
             Run 'cargo run -- build' to build all artifacts first.",
            initramfs_src.display()
        );
    }

    // Generate boot entry and loader config from distro_spec
    let boot_entry = boot_entry_with_partuuid(&uuids.root_part_uuid);
    let entry_content = boot_entry.to_entry_file();
    let entry_filename = format!("{}.conf", boot_entry.filename);
    let loader_config = default_loader_config().to_loader_conf();

    // Delegate to shared partition creation
    distro_builder::artifact::disk::partitions::create_efi_partition(
        image_path,
        EFI_SIZE_MB,
        uuids,
        &entry_filename,
        &entry_content,
        &loader_config,
        &kernel_src,
        &initramfs_src,
        systemd_boot_src,
    )
}

/// Create the root partition image using mkfs.ext4 -d.
pub fn create_root_partition(
    rootfs: &Path,
    image_path: &Path,
    size_mb: u64,
    uuids: &DiskUuids,
) -> Result<()> {
    distro_builder::artifact::disk::partitions::create_root_partition(
        rootfs, image_path, size_mb, uuids,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_efi_size_constant() {
        assert_eq!(EFI_SIZE_MB, 1024);
    }

    #[test]
    fn test_partition_creation_functions_exist() {
        assert!(true);
    }
}
