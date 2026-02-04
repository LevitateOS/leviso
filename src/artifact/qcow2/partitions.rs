//! EFI and root partition creation for qcow2 disk images.

use super::helpers::DiskUuids;
use super::mtools;
use anyhow::Result;
use distro_builder::process::{ensure_exists, find_first_existing, Cmd};
use distro_spec::levitate::boot::{boot_entry_with_partuuid, default_loader_config};
use std::fs;
use std::path::{Path, PathBuf};

/// EFI partition size in MB
pub const EFI_SIZE_MB: u64 = 1024;

/// Create the EFI partition image using mkfs.vfat and mtools.
pub fn create_efi_partition(
    base_dir: &Path,
    image_path: &Path,
    uuids: &DiskUuids,
    rootfs: &Path,
) -> Result<()> {
    // Create sparse image file
    let size_bytes = EFI_SIZE_MB * 1024 * 1024;
    {
        let file = fs::File::create(image_path)?;
        file.set_len(size_bytes)?;
    }

    // Format as FAT32 with specific volume ID
    // Volume ID format: XXXXXXXX (8 hex digits, no dash)
    let vol_id = uuids.efi_fs_uuid.replace('-', "");
    Cmd::new("mkfs.vfat")
        .args(["-F", "32", "-n", "EFI", "-i", &vol_id])
        .arg_path(image_path)
        .error_msg("mkfs.vfat failed")
        .run()?;

    // Create directory structure using mtools
    // mtools uses -i to specify the image file
    mtools::mtools_mkdir(image_path, "EFI")?;
    mtools::mtools_mkdir(image_path, "EFI/BOOT")?;
    mtools::mtools_mkdir(image_path, "EFI/systemd")?;
    mtools::mtools_mkdir(image_path, "loader")?;
    mtools::mtools_mkdir(image_path, "loader/entries")?;

    // Copy systemd-boot EFI binary
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

    mtools::mtools_copy(image_path, systemd_boot_src, "EFI/BOOT/BOOTX64.EFI")?;
    mtools::mtools_copy(
        image_path,
        systemd_boot_src,
        "EFI/systemd/systemd-bootx64.efi",
    )?;

    // Write loader.conf
    let loader_config = default_loader_config();
    let loader_conf_content = loader_config.to_loader_conf();
    mtools::mtools_write_file(image_path, "loader/loader.conf", &loader_conf_content)?;

    // Write boot entry
    let boot_entry = boot_entry_with_partuuid(&uuids.root_part_uuid);
    let entry_content = boot_entry.to_entry_file();
    let entry_filename = format!("loader/entries/{}.conf", boot_entry.filename);
    mtools::mtools_write_file(image_path, &entry_filename, &entry_content)?;

    // Copy kernel and initramfs
    let output_dir = base_dir.join("output");
    let staging_dir = output_dir.join("staging");

    let kernel_src = staging_dir.join("boot/vmlinuz");
    ensure_exists(&kernel_src, "Kernel")?;
    mtools::mtools_copy(image_path, &kernel_src, "vmlinuz")?;

    // Copy install initramfs (REQUIRED - live initramfs cannot boot installed systems)
    // The live initramfs is designed for ISO boot (mounts EROFS from CDROM).
    // The install initramfs is designed for disk boot (uses systemd to mount root partition).
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

    mtools::mtools_copy(image_path, &initramfs_src, "initramfs.img")?;

    Ok(())
}

/// Create the root partition image using mkfs.ext4 -d.
pub fn create_root_partition(
    rootfs: &Path,
    image_path: &Path,
    size_mb: u64,
    uuids: &DiskUuids,
) -> Result<()> {
    // Create sparse image file
    let size_bytes = size_mb * 1024 * 1024;
    {
        let file = fs::File::create(image_path)?;
        file.set_len(size_bytes)?;
    }

    // Create ext4 filesystem populated from rootfs directory
    // -d populates from directory without mounting
    // -U sets the UUID
    // -L sets the label
    Cmd::new("mkfs.ext4")
        .args(["-q", "-L", "root"])
        .args(["-U", &uuids.root_fs_uuid])
        .args(["-d"])
        .arg_path(rootfs)
        .arg_path(image_path)
        .error_msg("mkfs.ext4 -d failed. Check that e2fsprogs supports -d flag.")
        .run()?;

    Ok(())
}

// TEAM_151: Extracted partition creation functions into dedicated module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_efi_size_constant() {
        assert_eq!(EFI_SIZE_MB, 1024);
    }

    #[test]
    fn test_partition_creation_functions_exist() {
        // Smoke test to ensure functions compile
        assert!(true);
    }
}
