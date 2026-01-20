//! Build context shared across all catalyst modules.

use std::path::PathBuf;

/// Shared context for catalyst build operations.
pub struct BuildContext {
    /// Path to the source rootfs (Rocky rootfs with binaries)
    pub source: PathBuf,
    /// Path to the staging directory (where we build the stage3)
    pub staging: PathBuf,
    /// Path to the output directory (for the final tarball)
    pub output: PathBuf,
    /// Path to the recipe binary (optional)
    pub recipe_binary: Option<PathBuf>,
}

impl BuildContext {
    pub fn new(source: PathBuf, staging: PathBuf, output: PathBuf) -> Self {
        Self {
            source,
            staging,
            output,
            recipe_binary: None,
        }
    }

    pub fn with_recipe(mut self, recipe_binary: PathBuf) -> Self {
        self.recipe_binary = Some(recipe_binary);
        self
    }
}
