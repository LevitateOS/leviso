//! Build context shared across all initramfs modules.

use std::path::PathBuf;

/// Shared context for initramfs build operations.
pub struct BuildContext {
    /// Path to the Rocky rootfs (source of binaries)
    pub rootfs: PathBuf,
    /// Path to the initramfs root (destination)
    pub initramfs: PathBuf,
    /// Base directory of the leviso project
    pub base_dir: PathBuf,
}

impl BuildContext {
    pub fn new(rootfs: PathBuf, initramfs: PathBuf, base_dir: PathBuf) -> Self {
        Self {
            rootfs,
            initramfs,
            base_dir,
        }
    }
}
