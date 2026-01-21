//! Rootfs detection and validation.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Find the actual rootfs path, handling nested structures.
/// Rocky Linux uses LiveOS/rootfs.img inside install.img.
pub fn find_rootfs(extract_dir: &Path) -> Result<PathBuf> {
    let rootfs_dir = extract_dir.join("rootfs");

    // Check if rootfs exists - try both direct and nested paths
    if rootfs_dir.join("bin").exists() {
        return Ok(rootfs_dir);
    }

    if rootfs_dir.join("squashfs-root").exists() {
        return Ok(rootfs_dir.join("squashfs-root"));
    }

    if rootfs_dir.join("LiveOS").exists() {
        // Rocky uses LiveOS/rootfs.img inside install.img
        let liveos = rootfs_dir.join("LiveOS");
        if liveos.join("rootfs.img").exists() {
            println!("Found nested rootfs.img, extracting...");
            let inner_rootfs = extract_dir.join("inner-rootfs");
            if !inner_rootfs.exists() {
                fs::create_dir_all(&inner_rootfs)?;

                let rootfs_img = liveos.join("rootfs.img");
                let rootfs_img_str = rootfs_img
                    .to_str()
                    .context("rootfs.img path is not valid UTF-8")?;
                let inner_rootfs_str = inner_rootfs
                    .to_str()
                    .context("inner-rootfs path is not valid UTF-8")?;

                // Try unsquashfs first
                let status = Command::new("unsquashfs")
                    .args(["-d", inner_rootfs_str, "-f", rootfs_img_str])
                    .status();

                if status.is_err() || !status.as_ref().map(|s| s.success()).unwrap_or(false) {
                    // It might be ext4, try 7z
                    println!("Not a squashfs, trying to extract as ext4...");
                    let status = Command::new("7z")
                        .args(["x", "-y", rootfs_img_str, &format!("-o{}", inner_rootfs_str)])
                        .status()?;
                    if !status.success() {
                        bail!("Could not extract inner rootfs.img");
                    }
                }
            }
            return Ok(inner_rootfs);
        } else {
            bail!("Rootfs not found. Run 'leviso extract' first.");
        }
    }

    bail!(
        "Rootfs not found at {}. Run 'leviso extract' first.",
        rootfs_dir.display()
    );
}

/// Validate that essential binaries exist in rootfs.
pub fn validate_rootfs(rootfs: &Path) -> Result<()> {
    let essential = ["bin", "lib64", "usr/bin", "usr/lib64"];

    for dir in essential {
        let path = rootfs.join(dir);
        if !path.exists() {
            bail!("Essential directory missing from rootfs: {}", dir);
        }
    }

    // Check for bash specifically
    let bash_exists = rootfs.join("usr/bin/bash").exists() || rootfs.join("bin/bash").exists();
    if !bash_exists {
        bail!("bash not found in rootfs");
    }

    Ok(())
}

/// Check that required host tools are available.
pub fn check_host_tools() -> Result<()> {
    // readelf instead of ldd - works for cross-compilation
    let tools = ["readelf", "cpio", "gzip"];

    for tool in tools {
        let status = Command::new("which").arg(tool).output();
        match status {
            Ok(output) if output.status.success() => {}
            _ => bail!("Required host tool not found: {}", tool),
        }
    }

    Ok(())
}
