use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ROCKY_DVD_URL: &str =
    "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso";
const ROCKY_DVD_SIZE: &str = "8.6GB";
const SYSLINUX_URL: &str =
    "https://mirrors.edge.kernel.org/pub/linux/utils/boot/syslinux/syslinux-6.03.tar.xz";

pub fn download_rocky(base_dir: &Path) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let iso_path = downloads_dir.join("Rocky-10.1-x86_64-dvd1.iso");

    if iso_path.exists() {
        println!("Rocky DVD ISO already exists at {}", iso_path.display());
        return Ok(());
    }

    fs::create_dir_all(&downloads_dir)?;
    println!("Downloading Rocky Linux 10 DVD ISO ({})...", ROCKY_DVD_SIZE);
    println!("URL: {}", ROCKY_DVD_URL);

    let status = Command::new("curl")
        .args([
            "-L",
            "-o",
            iso_path.to_str().unwrap(),
            "--progress-bar",
            ROCKY_DVD_URL,
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

/// Download Rocky Linux DVD ISO (8.6GB) for binary manifest extraction.
///
/// This is used as the source of truth for "what binaries should a Linux system have"
/// per the prevention-first test design based on Anthropic research.
///
/// The DVD contains the full set of packages that Rocky ships, allowing us to
/// extract a manifest of all binaries that a real distribution includes.
pub fn download_rocky_dvd(base_dir: &Path, skip_confirm: bool) -> Result<PathBuf> {
    let downloads_dir = base_dir.join("downloads");
    let dvd_path = downloads_dir.join("Rocky-10.1-x86_64-dvd1.iso");

    if dvd_path.exists() {
        println!("Rocky DVD ISO already exists at {}", dvd_path.display());
        return Ok(dvd_path);
    }

    // Show warning and require confirmation unless --yes flag is passed
    if !skip_confirm {
        eprintln!();
        eprintln!("{}", "=".repeat(70));
        eprintln!("WARNING: About to download Rocky Linux DVD ISO ({})", ROCKY_DVD_SIZE);
        eprintln!("{}", "=".repeat(70));
        eprintln!();
        eprintln!("URL: {}", ROCKY_DVD_URL);
        eprintln!("Destination: {}", dvd_path.display());
        eprintln!();
        eprintln!("This ISO is used as the source of truth for binary manifest extraction.");
        eprintln!("It enables prevention-first testing by deriving test expectations from");
        eprintln!("what Rocky Linux actually ships, rather than hardcoded lists.");
        eprintln!();
        eprint!("Continue? [y/N]: ");

        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            bail!("Download cancelled by user");
        }
    }

    fs::create_dir_all(&downloads_dir)?;
    println!("Downloading Rocky Linux 10 DVD ISO ({})...", ROCKY_DVD_SIZE);
    println!("URL: {}", ROCKY_DVD_URL);
    println!("This may take a while depending on your connection speed.");

    let status = Command::new("curl")
        .args([
            "-L",
            "-o",
            dvd_path.to_str().unwrap(),
            "--progress-bar",
            ROCKY_DVD_URL,
        ])
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        // Clean up partial download
        let _ = fs::remove_file(&dvd_path);
        bail!("curl failed with status: {}", status);
    }

    println!("Downloaded to {}", dvd_path.display());
    Ok(dvd_path)
}
