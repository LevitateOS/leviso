//! Build context shared across all build modules.
//!
//! Provides paths needed to build the LevitateOS system image.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Shared context for all build operations.
pub struct BuildContext {
    /// Path to the source rootfs (Rocky rootfs with binaries)
    pub source: PathBuf,
    /// Path to the staging directory (where we build the filesystem)
    pub staging: PathBuf,
    /// Base directory of the leviso project
    pub base_dir: PathBuf,
    /// Output directory for build artifacts
    pub output: PathBuf,
    /// Optional path to the recipe binary
    pub recipe_binary: Option<PathBuf>,
}

impl BuildContext {
    /// Create a new build context.
    ///
    /// # Arguments
    /// * `base_dir` - The leviso project root directory
    /// * `staging` - Where to build the filesystem
    pub fn new(base_dir: &Path, staging: &Path) -> Result<Self> {
        let downloads = base_dir.join("downloads");
        let source = downloads.join("rootfs");
        let output = base_dir.join("output");

        if !source.exists() {
            anyhow::bail!(
                "Rocky rootfs not found at {}.\n\
                 Run 'leviso download rocky && leviso extract rocky' first.",
                source.display()
            );
        }

        Ok(Self {
            source,
            staging: staging.to_path_buf(),
            base_dir: base_dir.to_path_buf(),
            output,
            recipe_binary: None,
        })
    }

    /// Set the recipe binary path.
    pub fn with_recipe_binary(mut self, path: PathBuf) -> Self {
        self.recipe_binary = Some(path);
        self
    }
}
