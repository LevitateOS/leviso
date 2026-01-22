//! Build context shared across all build modules.
//!
//! Provides paths needed to build the LevitateOS system image.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Shared context for all build operations.
#[allow(dead_code)] // Fields used by API consumers and tests
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
    #[allow(dead_code)] // API for future use
    pub fn with_recipe_binary(mut self, path: PathBuf) -> Self {
        self.recipe_binary = Some(path);
        self
    }

    /// Create a build context for testing with custom source path.
    ///
    /// Unlike `new()`, this doesn't require the source to exist.
    /// This is intended for unit/integration tests only.
    #[doc(hidden)]
    #[allow(dead_code)] // Used by integration tests
    pub fn for_testing(source: &Path, staging: &Path, base_dir: &Path) -> Self {
        Self {
            source: source.to_path_buf(),
            staging: staging.to_path_buf(),
            base_dir: base_dir.to_path_buf(),
            output: base_dir.join("output"),
            recipe_binary: None,
        }
    }
}
