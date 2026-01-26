use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_builder::process::Cmd;

// NOTE: Supplementary RPM extraction has been moved to packages.rhai recipe.
// See leviso/deps/packages.rhai for the package list and extraction logic.
// This separation allows changing the package list without re-extracting the 2GB squashfs.

/// Extract Rocky ISO from a specific path.
///
/// This is the main extraction function used by the dependency resolver.
pub fn extract_rocky_iso(base_dir: &Path, iso_path: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_contents = extract_dir.join("iso-contents");
    let rootfs_dir = extract_dir.join("rootfs");

    if !iso_path.exists() {
        bail!(
            "Rocky DVD ISO not found at {}.",
            iso_path.display()
        );
    }

    // Step 1: Extract ISO contents with 7z
    if !iso_contents.exists() {
        println!("Extracting ISO contents with 7z...");
        fs::create_dir_all(&iso_contents)?;

        Cmd::new("7z")
            .args(["x", "-y"])
            .arg_path(iso_path)
            .arg(format!("-o{}", iso_contents.display()))
            .error_msg("7z extraction failed. Install: sudo dnf install p7zip-plugins")
            .run_interactive()?;
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

        // unsquashfs may return non-zero for xattr warnings, so allow_fail and check manually
        let result = Cmd::new("unsquashfs")
            .args(["-d"])
            .arg_path(&rootfs_dir)
            .args(["-f", "-no-xattrs"])
            .arg_path(squashfs_path)
            .allow_fail()
            .run()?;

        // Check if extraction actually succeeded (regardless of exit code)
        if !result.success() && !rootfs_dir.join("usr").exists() {
            bail!(
                "unsquashfs failed. Install: sudo dnf install squashfs-tools\n{}",
                result.stderr_trimmed()
            );
        }

        // Fix permissions: unsquashfs preserves root ownership which prevents further writes
        // Make the rootfs writable so packages.rhai can merge supplementary RPMs later
        println!("Fixing permissions on extracted rootfs...");
        let chmod_result = Cmd::new("chmod")
            .args(["-R", "u+rwX"])
            .arg_path(&rootfs_dir)
            .allow_fail()
            .run()?;

        if !chmod_result.success() {
            // FAIL FAST - if we can't fix permissions, the build will fail later
            // Better to fail now with a clear message
            bail!(
                "Could not fix permissions on extracted rootfs.\n\
                 \n\
                 The rootfs contains files owned by root that prevent writing.\n\
                 Without write access, we cannot merge supplementary RPMs.\n\
                 \n\
                 Run with sudo, or run 'sudo chown -R $USER {}' first.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                rootfs_dir.display()
            );
        }
    } else {
        println!("Rootfs already extracted to {}", rootfs_dir.display());
    }

    // NOTE: Supplementary RPM extraction is now done by packages.rhai recipe.
    // This function only handles ISO extraction for manual inspection.
    // Run `leviso build` to get the full rootfs with supplementary packages.

    println!("Extraction complete!");
    println!("  ISO contents: {}", iso_contents.display());
    println!("  Rootfs: {}", rootfs_dir.display());
    println!("\nNote: Supplementary packages are extracted during build via packages.rhai.");
    Ok(())
}
