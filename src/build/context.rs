//! Build context shared across all build modules.
//!
//! Provides paths needed to build the LevitateOS system image.

use anyhow::Result;
use distro_builder::BuildContext as BuildContextTrait;
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
        let output = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);

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
        })
    }

    /// Create a build context for testing without validation.
    ///
    /// This bypasses the check for Rocky rootfs existence.
    /// Only use in tests with mock filesystems.
    #[allow(dead_code)]
    pub fn for_testing(source: &Path, staging: &Path, base_dir: &Path) -> Self {
        Self {
            source: source.to_path_buf(),
            staging: staging.to_path_buf(),
            base_dir: base_dir.to_path_buf(),
            output: base_dir.join("output"),
        }
    }
}

// Implement distro-builder's BuildContext trait
impl BuildContextTrait for BuildContext {
    fn source(&self) -> &Path {
        &self.source
    }

    fn staging(&self) -> &Path {
        &self.staging
    }

    fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn output(&self) -> &Path {
        &self.output
    }

    fn config(&self) -> &dyn distro_builder::DistroConfig {
        &super::distro_config::LevitateOsConfig
    }
}
