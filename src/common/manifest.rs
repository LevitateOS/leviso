//! Utilities for reading manifest-relative files (compile-time bundled content).

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Read a file from a manifest-relative directory.
///
/// This reads files bundled with the binary at compile time via `env!("CARGO_MANIFEST_DIR")`.
/// Used for reading configuration templates, overlay files, etc. that are stored alongside source.
///
/// # Arguments
/// * `subdir` - Subdirectory relative to `src/component/custom/` (e.g. "etc/files", "live/overlay", "packages/files")
/// * `path` - Relative path within that subdirectory
///
/// # Examples
/// ```ignore
/// read_manifest_file("etc/files", "passwd")?
/// read_manifest_file("live/overlay", "etc/profile.d/00-levitate-test.sh")?
/// read_manifest_file("packages/files", "recipe.conf")?
/// ```
pub fn read_manifest_file(subdir: &str, path: &str) -> Result<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file_path = manifest_dir
        .join("src/component/custom")
        .join(subdir)
        .join(path);
    fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read {} from {}", path, file_path.display()))
}
