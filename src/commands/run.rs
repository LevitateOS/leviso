//! Run and test commands - boot ISO in QEMU.

use anyhow::Result;
use std::path::Path;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, ROOTFS_NAME};

use crate::artifact;
use crate::qemu;

/// Execute the run command.
pub fn cmd_run(base_dir: &Path, no_disk: bool, disk_size: String) -> Result<()> {
    // Auto-build if ISO doesn't exist
    let iso_path = base_dir.join("output/levitateos.iso");
    if !iso_path.exists() {
        println!("ISO not found, building...\n");
        let rootfs_path = base_dir.join("output").join(ROOTFS_NAME);
        let initramfs_path = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);

        if !rootfs_path.exists() {
            artifact::build_rootfs(base_dir)?;
        }
        if !initramfs_path.exists() {
            artifact::build_tiny_initramfs(base_dir)?;
        }
        artifact::create_iso(base_dir)?;
    }

    let disk = if no_disk { None } else { Some(disk_size) };
    qemu::run_iso(base_dir, disk)?;

    Ok(())
}

/// Execute the test command - headless boot verification.
pub fn cmd_test(base_dir: &Path, timeout: u64) -> Result<()> {
    // Auto-build if ISO doesn't exist
    let iso_path = base_dir.join("output/levitateos.iso");
    if !iso_path.exists() {
        println!("ISO not found, building...\n");
        let rootfs_path = base_dir.join("output").join(ROOTFS_NAME);
        let initramfs_path = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);

        if !rootfs_path.exists() {
            artifact::build_rootfs(base_dir)?;
        }
        if !initramfs_path.exists() {
            artifact::build_tiny_initramfs(base_dir)?;
        }
        artifact::create_iso(base_dir)?;
    }

    qemu::test_iso(base_dir, timeout)?;

    Ok(())
}
