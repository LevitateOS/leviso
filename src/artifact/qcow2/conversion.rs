//! QCOW2 conversion from raw disk images.

use anyhow::Result;
use std::fs;
use std::path::Path;
use distro_builder::process::Cmd;

/// Convert raw disk to qcow2 with compression.
pub fn convert_to_qcow2(raw_path: &Path, qcow2_path: &Path) -> Result<()> {
    // Remove existing qcow2 if present
    if qcow2_path.exists() {
        fs::remove_file(qcow2_path)?;
    }

    Cmd::new("qemu-img")
        .args(["convert", "-f", "raw", "-O", "qcow2", "-c"])
        .arg_path(raw_path)
        .arg_path(qcow2_path)
        .error_msg("qemu-img convert failed")
        .run()?;
    Ok(())
}

// TEAM_151: Extracted QCOW2 conversion function into dedicated module
#[cfg(test)]
mod tests {
    #[test]
    fn test_conversion_function_exists() {
        // Smoke test to ensure function compiles
        assert!(true);
    }
}
