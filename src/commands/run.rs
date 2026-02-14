//! Run and test commands - boot ISO in QEMU.

use anyhow::Result;
use std::path::Path;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME};

use crate::artifact;
use crate::qemu;
use crate::recipe;

/// Ensure ISO exists, building if necessary.
fn ensure_iso_built(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let iso_path = output_dir.join(ISO_FILENAME);
    if iso_path.exists() {
        return Ok(());
    }

    println!("ISO not found, building...\n");
    let rootfs_path = output_dir.join(ROOTFS_NAME);
    let initramfs_path = output_dir.join(INITRAMFS_LIVE_OUTPUT);

    if !rootfs_path.exists() {
        artifact::build_rootfs(base_dir)?;
    }
    if !initramfs_path.exists() {
        artifact::build_tiny_initramfs(base_dir)?;
    }
    artifact::create_iso(base_dir)?;

    Ok(())
}

/// Execute the run command.
pub fn cmd_run(base_dir: &Path, no_disk: bool, disk_size: String) -> Result<()> {
    recipe::ensure_qemu(base_dir)?;
    ensure_iso_built(base_dir)?;

    let disk = if no_disk { None } else { Some(disk_size) };
    qemu::run_iso(base_dir, disk)?;

    Ok(())
}

/// Execute the test command - headless boot verification.
pub fn cmd_test(base_dir: &Path, timeout: u64) -> Result<()> {
    recipe::ensure_qemu(base_dir)?;
    ensure_iso_built(base_dir)?;
    qemu::test_iso(base_dir, timeout)?;
    Ok(())
}
