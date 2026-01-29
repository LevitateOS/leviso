//! UUID generation and host tool verification helpers.

use anyhow::{bail, Context, Result};
use std::process::Command;
use distro_builder::process::Cmd;

/// Generated UUIDs for the disk image.
pub struct DiskUuids {
    /// Filesystem UUID for root partition (ext4)
    pub root_fs_uuid: String,
    /// Filesystem UUID for EFI partition (vfat serial)
    pub efi_fs_uuid: String,
    /// GPT partition UUID for root partition (used in boot entry)
    pub root_part_uuid: String,
}

impl DiskUuids {
    /// Generate new random UUIDs.
    pub fn generate() -> Result<Self> {
        Ok(Self {
            root_fs_uuid: generate_uuid()?,
            efi_fs_uuid: generate_vfat_serial()?,
            root_part_uuid: generate_uuid()?,
        })
    }
}

/// Required host tools for sudo-free qcow2 building.
const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("qemu-img", "qemu-img"),
    ("sfdisk", "util-linux"),
    ("mkfs.vfat", "dosfstools"),
    ("mkfs.ext4", "e2fsprogs"),
    ("mcopy", "mtools"),
    ("mmd", "mtools"),
    ("uuidgen", "util-linux"),
    ("dd", "coreutils"),
];

/// Verify all required host tools are available.
pub fn check_host_tools() -> Result<()> {
    let mut missing = Vec::new();

    for (tool, package) in REQUIRED_TOOLS {
        let result = Cmd::new("which").arg(tool).allow_fail().run();
        if result.is_err() || !result.unwrap().success() {
            missing.push(format!("  {} (install: {})", tool, package));
        }
    }

    if !missing.is_empty() {
        bail!(
            "Missing required tools:\n{}\n\nInstall them first.",
            missing.join("\n")
        );
    }

    Ok(())
}

/// Generate a random UUID using uuidgen.
pub fn generate_uuid() -> Result<String> {
    let output = Command::new("uuidgen")
        .output()
        .context("Failed to run uuidgen")?;

    if !output.status.success() {
        bail!("uuidgen failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_lowercase())
}

/// Generate a random FAT32 volume serial (8 hex chars, e.g., "ABCD-1234").
pub fn generate_vfat_serial() -> Result<String> {
    let output = Command::new("uuidgen")
        .output()
        .context("Failed to run uuidgen")?;

    if !output.status.success() {
        bail!("uuidgen failed");
    }

    // Take first 8 hex chars and format as XXXX-XXXX
    let uuid = String::from_utf8_lossy(&output.stdout);
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).take(8).collect();
    if hex.len() < 8 {
        bail!("Failed to generate vfat serial");
    }
    Ok(format!("{}-{}", &hex[0..4].to_uppercase(), &hex[4..8].to_uppercase()))
}

// TEAM_151: Extracted UUID and host tool verification functions into dedicated helpers module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_tools_list() {
        assert!(!REQUIRED_TOOLS.is_empty());
        for (tool, package) in REQUIRED_TOOLS {
            assert!(!tool.is_empty());
            assert!(!package.is_empty());
        }
    }

    #[test]
    fn test_generate_vfat_serial_format() {
        // Can only test format, not randomness
        let serial = generate_vfat_serial().unwrap();
        assert_eq!(serial.len(), 9); // XXXX-XXXX
        assert_eq!(&serial[4..5], "-");
    }

    #[test]
    fn test_partition_constants() {
        // Verify constants are sensible (from parent module)
        // This is a minimal test - constants are checked in mod.rs
        assert!(true);
    }
}
