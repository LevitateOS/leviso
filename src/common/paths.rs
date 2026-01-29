//! Utilities for path checking and directory management.

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

/// Find a directory from primary or fallback location, creating destination if needed.
///
/// Checks if the primary path exists as a directory. If not, checks the fallback.
/// If neither exists, returns an error with the provided message.
///
/// This consolidates the common pattern of:
/// ```ignore
/// let src = if primary_path.is_dir() {
///     primary_path
/// } else if fallback_path.is_dir() {
///     fallback_path
/// } else {
///     bail!("error message")
/// };
/// fs::create_dir_all(dst)?;
/// ```
///
/// # Arguments
/// * `primary` - Primary source path to check
/// * `fallback` - Fallback source path if primary doesn't exist
/// * `destination` - Destination directory to create
/// * `error_msg` - Error message if neither source exists
///
/// # Returns
/// Returns () on success, or error with provided message
pub fn find_and_copy_dir(
    primary: &Path,
    fallback: &Path,
    destination: &Path,
    error_msg: &str,
) -> Result<()> {
    // Check that at least one source exists
    if !primary.is_dir() && !fallback.is_dir() {
        bail!("{}", error_msg);
    }

    fs::create_dir_all(destination)?;
    Ok(())
}

/// Find a directory from multiple possible locations.
///
/// Checks locations in order and returns the first one that exists as a directory.
/// If none exist, returns an error.
///
/// # Arguments
/// * `locations` - Slice of paths to check in order
/// * `error_msg` - Error message if no location exists
///
/// # Returns
/// Returns path to first existing directory, or error
pub fn find_dir<'a>(locations: &'a [&Path], error_msg: &str) -> Result<&'a Path> {
    for loc in locations {
        if loc.is_dir() {
            return Ok(loc);
        }
    }
    bail!("{}", error_msg)
}

/// Ensure a directory exists, creating it if necessary.
///
/// This is a convenience wrapper around fs::create_dir_all that doesn't fail
/// if the directory already exists.
///
/// # Arguments
/// * `path` - Path to the directory
pub fn ensure_dir_exists(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

/// Ensure all parent directories of a file exist.
///
/// Creates all parent directories of the given path. If the path has no parents,
/// does nothing (doesn't error).
pub fn ensure_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
