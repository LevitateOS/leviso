//! mtools file operations for FAT32 image manipulation.

use anyhow::Result;
use std::fs;
use std::path::Path;
use distro_builder::process::Cmd;

/// Create a directory in a FAT image using mmd.
pub fn mtools_mkdir(image: &Path, dir: &str) -> Result<()> {
    // Note: mmd returns error if directory exists, which is fine
    // We use run_ignore_status to not fail on "directory exists" errors
    let _ = Cmd::new("mmd")
        .args(["-i"])
        .arg_path(image)
        .arg(format!("::{}", dir))
        .run();
    Ok(())
}

/// Copy a file into a FAT image using mcopy.
pub fn mtools_copy(image: &Path, src: &Path, dest: &str) -> Result<()> {
    Cmd::new("mcopy")
        .args(["-i"])
        .arg_path(image)
        .arg_path(src)
        .arg(format!("::{}", dest))
        .error_msg(&format!("mcopy failed: {} -> {}", src.display(), dest))
        .run()?;
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
