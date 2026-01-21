use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// RPMs to extract and merge into the rootfs.
/// The install.img (Anaconda installer) is missing utilities that users expect.
/// These RPMs supplement the installer rootfs with essential utilities.
const SUPPLEMENTARY_RPMS: &[&str] = &[
    // procps-ng: free, vmstat, uptime, w, watch (installer only has ps, top, pidof)
    "procps-ng",
];

pub fn extract_rocky(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_path = extract_dir.join("Rocky-10.1-x86_64-dvd1.iso");
    let iso_contents = extract_dir.join("iso-contents");
    let rootfs_dir = extract_dir.join("rootfs");

    if !iso_path.exists() {
        bail!(
            "Rocky DVD ISO not found at {}. Run 'leviso download' first.",
            iso_path.display()
        );
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

        // Fix permissions: unsquashfs preserves root ownership which prevents further writes
        // Make the rootfs writable so we can merge in supplementary RPMs
        println!("Fixing permissions on extracted rootfs...");
        let chmod_status = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&rootfs_dir)
            .status()
            .context("Failed to fix permissions")?;

        if !chmod_status.success() {
            println!("Warning: Could not fix all permissions (may need sudo for some files)");
        }
    } else {
        println!("Rootfs already extracted to {}", rootfs_dir.display());
    }

    // Step 3: Extract supplementary RPMs into rootfs
    // The install.img is the Anaconda installer which lacks some utilities users expect
    extract_supplementary_rpms(&iso_contents, &rootfs_dir)?;

    println!("Extraction complete!");
    Ok(())
}

/// Extract supplementary RPMs and merge them into the rootfs.
fn extract_supplementary_rpms(iso_contents: &Path, rootfs_dir: &Path) -> Result<()> {
    // Search both BaseOS and AppStream for packages
    let package_dirs = [
        iso_contents.join("BaseOS/Packages"),
        iso_contents.join("AppStream/Packages"),
    ];

    for rpm_prefix in SUPPLEMENTARY_RPMS {
        let first_char = rpm_prefix.chars().next().unwrap();

        // Search in both BaseOS and AppStream
        let mut rpm_path: Option<std::path::PathBuf> = None;
        for packages_dir in &package_dirs {
            if !packages_dir.exists() {
                continue;
            }
            let rpm_subdir = packages_dir.join(first_char.to_string());
            if rpm_subdir.exists() {
                if let Some(found) = find_rpm(&rpm_subdir, rpm_prefix)? {
                    rpm_path = Some(found);
                    break;
                }
            }
        }

        // Find the matching RPM
        let rpm_path = rpm_path;

        if let Some(rpm) = rpm_path {
            println!("Extracting supplementary RPM: {}", rpm.file_name().unwrap().to_string_lossy());

            // Extract RPM contents directly into rootfs
            // rpm2cpio outputs cpio archive, which we extract in rootfs
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "rpm2cpio '{}' | cpio -idmu --quiet",
                    rpm.display()
                ))
                .current_dir(rootfs_dir)
                .output()
                .context("Failed to extract RPM")?;

            if !output.status.success() {
                println!("Warning: Failed to extract {}: {}",
                    rpm.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        } else {
            println!("Warning: RPM not found for prefix: {}", rpm_prefix);
        }
    }

    Ok(())
}

/// Find an RPM file by prefix in a directory.
fn find_rpm(dir: &Path, prefix: &str) -> Result<Option<std::path::PathBuf>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            // Match RPM files that start with the prefix and end with .rpm
            // e.g., "procps-ng" matches "procps-ng-4.0.4-8.el10.x86_64.rpm"
            if name_str.starts_with(prefix)
                && name_str.ends_with(".rpm")
                && !name_str.contains("-devel")
                && !name_str.contains("-i18n")
                && name_str.contains("x86_64")
            {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}
