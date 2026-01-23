//! Run command - boots ISO in QEMU.

use anyhow::Result;
use std::path::Path;

use crate::artifact;
use crate::qemu;

/// Execute the run command.
pub fn cmd_run(base_dir: &Path, no_disk: bool, disk_size: String) -> Result<()> {
    // Auto-build if ISO doesn't exist
    let iso_path = base_dir.join("output/levitateos.iso");
    if !iso_path.exists() {
        println!("ISO not found, building...\n");
        let squashfs_path = base_dir.join("output/filesystem.squashfs");
        let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");

        if !squashfs_path.exists() {
            artifact::build_squashfs(base_dir)?;
        }
        if !initramfs_path.exists() {
            artifact::build_tiny_initramfs(base_dir)?;
        }
        artifact::create_squashfs_iso(base_dir)?;
    }

    let disk = if no_disk { None } else { Some(disk_size) };
    qemu::run_iso(base_dir, disk)?;

    Ok(())
}
