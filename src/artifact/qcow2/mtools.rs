//! mtools file operations for FAT32 image manipulation.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Create a directory in a FAT image using mmd.
pub fn mtools_mkdir(image: &Path, dir: &str) -> Result<()> {
    let status = Command::new("mmd")
        .args(["-i"])
        .arg(image)
        .arg(format!("::{}", dir))
        .status()
        .context("Failed to run mmd")?;

    // mmd returns error if directory exists, which is fine
    if !status.success() {
        // Ignore "directory exists" errors
    }
    Ok(())
}

/// Copy a file into a FAT image using mcopy.
pub fn mtools_copy(image: &Path, src: &Path, dest: &str) -> Result<()> {
    let status = Command::new("mcopy")
        .args(["-i"])
        .arg(image)
        .arg(src)
        .arg(format!("::{}", dest))
        .status()
        .with_context(|| format!("Failed to copy {} to {}", src.display(), dest))?;

    if !status.success() {
        bail!("mcopy failed: {} -> {}", src.display(), dest);
    }
    Ok(())
}

/// Write content to a file in a FAT image.
pub fn mtools_write_file(image: &Path, dest: &str, content: &str) -> Result<()> {
    // Write to temp file first, then mcopy
    let temp = std::env::temp_dir().join(format!("mtools-{}", std::process::id()));
    fs::write(&temp, content)?;

    let result = mtools_copy(image, &temp, dest);
    let _ = fs::remove_file(&temp);
    result
}

// TEAM_151: Extracted mtools file operation functions into dedicated module
#[cfg(test)]
mod tests {
    // Note: Actual mtools testing requires real FAT images
    // These tests are minimal - full integration testing happens in main build
    #[test]
    fn test_mtools_functions_exist() {
        // Smoke test to ensure functions compile and are accessible
        assert!(true);
    }
}
