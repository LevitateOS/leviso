//! Utilities for managing temporary work directories.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Prepare a work directory, removing it if it exists and creating it fresh.
///
/// This consolidates the common pattern of:
/// ```ignore
/// let work_dir = base_dir.join(name);
/// if work_dir.exists() {
///     fs::remove_dir_all(&work_dir)?;
/// }
/// fs::create_dir_all(&work_dir)?;
/// ```
///
/// # Arguments
/// * `parent_dir` - Parent directory where the work dir should be created
/// * `name` - Name of the work directory (e.g., "qcow2-work", ".systemd-boot-extract")
///
/// # Returns
/// Path to the newly created work directory
///
/// # Example
/// ```ignore
/// let work_dir = prepare_work_dir(output_dir, "qcow2-work")?;
/// // work_dir is ready to use, empty and fresh
/// ```
pub fn prepare_work_dir(parent_dir: &Path, name: &str) -> Result<PathBuf> {
    let work_dir = parent_dir.join(name);

    // Clean up if it exists from a previous run
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)?;
    }

    // Create fresh directory
    fs::create_dir_all(&work_dir)?;

    Ok(work_dir)
}

/// Clean up a work directory after use.
///
/// Safely removes a directory tree. Uses `let _ = fs::remove_dir_all()` pattern
/// to avoid errors if directory doesn't exist (idempotent).
///
/// # Arguments
/// * `path` - Path to the directory to remove
pub fn cleanup_work_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}
