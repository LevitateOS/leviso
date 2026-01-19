use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn extract_rocky(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_path = extract_dir.join("rocky.iso");
    let iso_contents = extract_dir.join("iso-contents");
    let rootfs_dir = extract_dir.join("rootfs");

    if !iso_path.exists() {
        bail!("Rocky ISO not found. Run 'leviso download' first.");
    }

    // Step 1: Extract ISO contents with 7z
    if !iso_contents.exists() {
        println!("Extracting ISO contents with 7z...");
        fs::create_dir_all(&iso_contents)?;
        let status = Command::new("7z")
            .args([
                "x",
                "-y",
                iso_path.to_str().unwrap(),
                &format!("-o{}", iso_contents.display()),
            ])
            .status()
            .context("Failed to run 7z. Is p7zip installed?")?;

        if !status.success() {
            bail!("7z extraction failed");
        }
    } else {
        println!("ISO already extracted to {}", iso_contents.display());
    }

    // Step 2: Find and extract squashfs
    if !rootfs_dir.exists() {
        println!("Looking for squashfs...");

        // Rocky 10 uses images/install.img which is a squashfs
        let squashfs_candidates = [
            iso_contents.join("images/install.img"),
            iso_contents.join("LiveOS/squashfs.img"),
            iso_contents.join("LiveOS/rootfs.img"),
        ];

        let squashfs_path = squashfs_candidates
            .iter()
            .find(|p| p.exists())
            .context("Could not find squashfs image in ISO")?;

        println!("Found squashfs at: {}", squashfs_path.display());

        fs::create_dir_all(&rootfs_dir)?;
        let status = Command::new("unsquashfs")
            .args([
                "-d",
                rootfs_dir.to_str().unwrap(),
                "-f",
                "-no-xattrs",
                squashfs_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to run unsquashfs. Is squashfs-tools installed?")?;

        // unsquashfs may return non-zero for xattr warnings, check if extraction succeeded
        if !status.success() && !rootfs_dir.join("usr").exists() {
            bail!("unsquashfs failed");
        }
    } else {
        println!("Rootfs already extracted to {}", rootfs_dir.display());
    }

    println!("Extraction complete!");
    Ok(())
}
