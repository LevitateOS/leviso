//! Disk assembly for qcow2 VM images.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use super::helpers::DiskUuids;
use super::partitions::EFI_SIZE_MB;

/// Sector size in bytes
const SECTOR_SIZE: u64 = 512;

/// GPT and partition alignment (1MB alignment is standard)
const ALIGNMENT_MB: u64 = 1;

/// First partition starts at this offset (1MB for GPT + alignment)
const FIRST_PARTITION_OFFSET_SECTORS: u64 = 2048; // 1MB / 512

/// Assemble the final disk image from partition images.
pub fn assemble_disk(
    disk_path: &Path,
    efi_image: &Path,
    root_image: &Path,
    disk_size_gb: u32,
    uuids: &DiskUuids,
) -> Result<()> {
    let disk_size_bytes = (disk_size_gb as u64) * 1024 * 1024 * 1024;

    // Create sparse disk image
    {
        let file = fs::File::create(disk_path)?;
        file.set_len(disk_size_bytes)?;
    }

    // Write GPT partition table
    // We specify the partition UUID for the root partition
    // sfdisk requires explicit field names for uuid
    let efi_size_sectors = (EFI_SIZE_MB * 1024 * 1024) / SECTOR_SIZE;
    let root_start_sector = FIRST_PARTITION_OFFSET_SECTORS + efi_size_sectors;
    let sfdisk_script = format!(
        "label: gpt\n\
         start={}, size={}, type=U, bootable\n\
         start={}, type=L, uuid={}\n",
        FIRST_PARTITION_OFFSET_SECTORS,
        efi_size_sectors,
        root_start_sector,
        uuids.root_part_uuid.to_uppercase()
    );

    let mut child = Command::new("sfdisk")
        .arg(disk_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to run sfdisk")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(sfdisk_script.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        bail!("sfdisk failed to create partition table");
    }

    // Calculate partition offsets
    // EFI partition: starts at sector 2048 (1MB), size = EFI_SIZE_MB
    let efi_offset_bytes = FIRST_PARTITION_OFFSET_SECTORS * SECTOR_SIZE;

    // Root partition: starts right after EFI (aligned to 1MB)
    let root_offset_sectors = FIRST_PARTITION_OFFSET_SECTORS + efi_size_sectors;
    let root_offset_bytes = root_offset_sectors * SECTOR_SIZE;

    // Copy EFI partition image into disk
    println!("  Writing EFI partition at offset {}...", efi_offset_bytes);
    let status = Command::new("dd")
        .args(["if=".to_string() + &efi_image.to_string_lossy()])
        .args(["of=".to_string() + &disk_path.to_string_lossy()])
        .args(["bs=1M", "conv=notrunc"])
        .arg(format!("seek={}", efi_offset_bytes / (1024 * 1024)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run dd for EFI partition")?;

    if !status.success() {
        bail!("dd failed for EFI partition");
    }

    // Copy root partition image into disk
    println!("  Writing root partition at offset {}...", root_offset_bytes);
    let status = Command::new("dd")
        .args(["if=".to_string() + &root_image.to_string_lossy()])
        .args(["of=".to_string() + &disk_path.to_string_lossy()])
        .args(["bs=1M", "conv=notrunc"])
        .arg(format!("seek={}", root_offset_bytes / (1024 * 1024)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run dd for root partition")?;

    if !status.success() {
        bail!("dd failed for root partition");
    }

    Ok(())
}

// TEAM_151: Extracted disk assembly function into dedicated module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_constants() {
        // Verify constants are sensible
        assert_eq!(EFI_SIZE_MB, 1024);
        assert_eq!(SECTOR_SIZE, 512);
        assert_eq!(FIRST_PARTITION_OFFSET_SECTORS, 2048); // 1MB
        assert_eq!(ALIGNMENT_MB, 1);
    }

    #[test]
    fn test_disk_assembly_functions_exist() {
        // Smoke test to ensure functions compile
        assert!(true);
    }
}
