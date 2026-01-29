//! Utilities for file operations with automatic parent directory creation.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Write a file, creating parent directories as needed.
///
/// This is a convenience function that combines creating the parent directory
/// with writing the file content, eliminating the common pattern of:
/// ```ignore
/// if let Some(parent) = path.parent() {
///     fs::create_dir_all(parent)?;
/// }
/// fs::write(path, content)?;
/// ```
///
/// # Arguments
/// * `path` - Path to the file to write
/// * `content` - Content to write (anything that implements AsRef<[u8]>)
pub fn write_file_with_dirs<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

/// Write a file with specific Unix permissions, creating parent directories as needed.
///
/// Combines file creation with parent directory creation and permission setting.
///
/// # Arguments
/// * `path` - Path to the file to write
/// * `content` - Content to write
/// * `mode` - Unix permission bits (e.g., 0o644, 0o600)
pub fn write_file_mode<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    content: C,
    mode: u32,
) -> Result<()> {
    let path = path.as_ref();
    write_file_with_dirs(path, content)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}
