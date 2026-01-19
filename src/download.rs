use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ROCKY_ISO_URL: &str =
    "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-minimal.iso";
const SYSLINUX_URL: &str =
    "https://mirrors.edge.kernel.org/pub/linux/utils/boot/syslinux/syslinux-6.03.tar.xz";

pub fn download_rocky(base_dir: &Path) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let iso_path = downloads_dir.join("rocky.iso");

    if iso_path.exists() {
        println!("Rocky ISO already exists at {}", iso_path.display());
        return Ok(());
    }

    fs::create_dir_all(&downloads_dir)?;
    println!("Downloading Rocky Linux 10 Minimal ISO...");
    println!("URL: {}", ROCKY_ISO_URL);

    let status = Command::new("curl")
        .args([
            "-L",
            "-o",
            iso_path.to_str().unwrap(),
            "--progress-bar",
            ROCKY_ISO_URL,
        ])
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        bail!("curl failed with status: {}", status);
    }

    println!("Downloaded to {}", iso_path.display());
    Ok(())
}

pub fn download_syslinux(base_dir: &Path) -> Result<PathBuf> {
    let downloads_dir = base_dir.join("downloads");
    let syslinux_tar = downloads_dir.join("syslinux-6.03.tar.xz");
    let syslinux_dir = downloads_dir.join("syslinux-6.03");

    if syslinux_dir.join("bios/core/isolinux.bin").exists() {
        println!("Syslinux already downloaded and extracted.");
        return Ok(syslinux_dir);
    }

    fs::create_dir_all(&downloads_dir)?;

    if !syslinux_tar.exists() {
        println!("Downloading syslinux from kernel.org...");
        let status = Command::new("curl")
            .args([
                "-L",
                "-o",
                syslinux_tar.to_str().unwrap(),
                "--progress-bar",
                SYSLINUX_URL,
            ])
            .status()
            .context("Failed to download syslinux")?;

        if !status.success() {
            bail!("Failed to download syslinux");
        }
    }

    println!("Extracting syslinux...");
    let status = Command::new("tar")
        .args([
            "-xf",
            syslinux_tar.to_str().unwrap(),
            "-C",
            downloads_dir.to_str().unwrap(),
        ])
        .status()
        .context("Failed to extract syslinux")?;

    if !status.success() {
        bail!("Failed to extract syslinux");
    }

    Ok(syslinux_dir)
}
