//! Linux kernel via recipe.
//!
//! Delegates to the shared distro-builder recipe wrapper.

pub use distro_builder::recipe::linux::{has_linux_source, LinuxPaths};

use anyhow::Result;
use distro_spec::levitate::KERNEL_SOURCE;
use std::path::Path;

/// Run the linux.rhai recipe and return the output paths.
///
/// This handles the full kernel workflow: acquire source, build, install to staging.
/// The recipe returns a ctx with all paths and the kernel version.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn linux(base_dir: &Path) -> Result<LinuxPaths> {
    distro_builder::recipe::linux::linux(
        base_dir,
        &KERNEL_SOURCE,
        distro_spec::levitate::MODULE_INSTALL_PATH,
    )
}
