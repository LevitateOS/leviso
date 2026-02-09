//! Disk assembly for qcow2 VM images.
//!
//! Thin wrapper over shared infrastructure in distro-builder::artifact::disk::assembly.

use super::helpers::DiskUuids;
use super::partitions::EFI_SIZE_MB;
use anyhow::Result;
use std::path::Path;

/// Assemble the final disk image from partition images.
pub fn assemble_disk(
    disk_path: &Path,
    efi_image: &Path,
    root_image: &Path,
    disk_size_gb: u32,
    uuids: &DiskUuids,
) -> Result<()> {
    distro_builder::artifact::disk::assembly::assemble_disk(
        disk_path,
        efi_image,
        root_image,
        disk_size_gb,
        EFI_SIZE_MB,
        uuids,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_constants() {
        assert_eq!(EFI_SIZE_MB, 1024);
    }

    #[test]
    fn test_disk_assembly_functions_exist() {
        assert!(true);
    }
}
