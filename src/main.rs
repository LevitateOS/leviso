//! Leviso - LevitateOS ISO builder.
//!
//! Builds LevitateOS using squashfs architecture:
//! - Squashfs system image (complete live system, ~400MB)
//! - Tiny initramfs (mounts squashfs, ~5MB)
//! - Bootable ISO

mod build;
mod clean;
mod common;
mod config;
mod download;
mod extract;
mod initramfs;
mod iso;
mod qemu;
mod squashfs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use config::Config;

#[derive(Parser)]
#[command(name = "leviso")]
#[command(about = "LevitateOS ISO builder")]
#[command(after_help = "QUICK START:\n  leviso build      Build everything\n  leviso run        Boot in QEMU\n  leviso clean      Remove build artifacts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build LevitateOS (downloads dependencies automatically)
    Build {
        #[command(subcommand)]
        target: Option<BuildTarget>,
    },

    /// Run the ISO in QEMU (UEFI boot)
    Run {
        /// Don't attach virtual disk
        #[arg(long)]
        no_disk: bool,
        /// Virtual disk size (default: 8G)
        #[arg(long, default_value = "8G")]
        disk_size: String,
    },

    /// Quick test in terminal (direct kernel boot with squashfs)
    Test {
        /// Command to run after boot, then exit
        #[arg(short, long)]
        cmd: Option<String>,
    },

    /// Clean build artifacts (default: preserves downloads)
    Clean {
        #[command(subcommand)]
        what: Option<CleanTarget>,
    },

    /// Show information
    Show {
        #[command(subcommand)]
        what: ShowTarget,
    },

    /// Download dependencies (usually automatic)
    Download {
        #[command(subcommand)]
        what: Option<DownloadTarget>,
    },

    /// Extract archives for inspection
    Extract {
        #[command(subcommand)]
        what: ExtractTarget,
    },
}

#[derive(Subcommand)]
enum BuildTarget {
    /// Build only the Linux kernel
    Kernel {
        /// Clean kernel build directory first
        #[arg(long)]
        clean: bool,
    },
    /// Build squashfs system image (complete live system)
    Squashfs,
    /// Build tiny initramfs (mounts squashfs, ~5MB)
    Initramfs,
    /// Build only the ISO image
    Iso,
}

#[derive(Subcommand)]
enum ShowTarget {
    /// Show current configuration
    Config,
    /// Show squashfs contents
    Squashfs,
}

#[derive(Subcommand)]
enum CleanTarget {
    /// Clean kernel build artifacts only
    Kernel,
    /// Clean ISO and initramfs only
    Iso,
    /// Clean squashfs only
    Squashfs,
    /// Clean downloaded sources (Rocky ISO, Linux source)
    Downloads,
    /// Clean everything (downloads + outputs)
    All,
}

#[derive(Subcommand)]
enum DownloadTarget {
    /// Download Linux kernel source
    Linux {
        /// Full clone instead of shallow (slower but complete history)
        #[arg(long)]
        full: bool,
    },
    /// Download Rocky Linux ISO
    Rocky,
}

#[derive(Subcommand)]
enum ExtractTarget {
    /// Extract Rocky ISO contents
    Rocky,
    /// Extract squashfs for inspection
    Squashfs {
        /// Output directory (default: output/squashfs-extracted)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = Config::load(&base_dir);

    match cli.command {
        // ===== BUILD =====
        Commands::Build { target } => {
            match target {
                None => {
                    // Full build: squashfs + tiny initramfs + ISO
                    println!("=== Full LevitateOS Build ===\n");

                    // 1. Download Rocky if needed
                    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
                        println!("Downloading Rocky Linux...");
                        download::download_rocky(&base_dir, &config.rocky)?;
                        extract::extract_rocky(&base_dir)?;
                    }

                    // 2. Download Linux source if needed
                    if !config.has_linux_source() {
                        println!("\nDownloading Linux kernel source...");
                        download_linux(&config, true)?;
                    }

                    // 3. Build kernel
                    println!("\nBuilding kernel...");
                    build_kernel(&base_dir, &config, false)?;

                    // 4. Build squashfs (complete system image)
                    println!("\nBuilding squashfs system image...");
                    squashfs::build_squashfs(&base_dir)?;

                    // 5. Build tiny initramfs
                    println!("\nBuilding tiny initramfs...");
                    initramfs::build_tiny_initramfs(&base_dir)?;

                    // 6. Build ISO
                    println!("\nBuilding ISO...");
                    iso::create_squashfs_iso(&base_dir)?;

                    println!("\n=== Build Complete ===");
                    println!("  ISO: output/levitateos.iso");
                    println!("  Squashfs: output/filesystem.squashfs");
                    println!("\nNext: leviso run");
                }
                Some(BuildTarget::Kernel { clean }) => {
                    if !config.has_linux_source() {
                        anyhow::bail!(
                            "Linux source not found at: {}\n\n\
                             Run 'leviso download linux' first.",
                            config.linux_source.display()
                        );
                    }
                    build_kernel(&base_dir, &config, clean)?;
                }
                Some(BuildTarget::Squashfs) => {
                    squashfs::build_squashfs(&base_dir)?;
                }
                Some(BuildTarget::Initramfs) => {
                    initramfs::build_tiny_initramfs(&base_dir)?;
                }
                Some(BuildTarget::Iso) => {
                    let squashfs_path = base_dir.join("output/filesystem.squashfs");
                    let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");

                    if !squashfs_path.exists() {
                        println!("Squashfs not found, building...");
                        squashfs::build_squashfs(&base_dir)?;
                    }
                    if !initramfs_path.exists() {
                        println!("Tiny initramfs not found, building...");
                        initramfs::build_tiny_initramfs(&base_dir)?;
                    }
                    iso::create_squashfs_iso(&base_dir)?;
                }
            }
        }

        // ===== RUN =====
        Commands::Run { no_disk, disk_size } => {
            // Auto-build if ISO doesn't exist
            let iso_path = base_dir.join("output/levitateos.iso");
            if !iso_path.exists() {
                println!("ISO not found, building...\n");
                let squashfs_path = base_dir.join("output/filesystem.squashfs");
                let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");

                if !squashfs_path.exists() {
                    squashfs::build_squashfs(&base_dir)?;
                }
                if !initramfs_path.exists() {
                    initramfs::build_tiny_initramfs(&base_dir)?;
                }
                iso::create_squashfs_iso(&base_dir)?;
            }
            let disk = if no_disk { None } else { Some(disk_size) };
            qemu::run_iso(&base_dir, disk)?;
        }

        Commands::Test { cmd } => {
            // Quick test: direct kernel boot with squashfs
            let squashfs_path = base_dir.join("output/filesystem.squashfs");
            let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");

            if !squashfs_path.exists() {
                println!("Squashfs not found, building...");
                squashfs::build_squashfs(&base_dir)?;
            }
            if !initramfs_path.exists() {
                println!("Tiny initramfs not found, building...");
                initramfs::build_tiny_initramfs(&base_dir)?;
            }
            qemu::test_direct(&base_dir, cmd)?;
        }

        // ===== CLEAN =====
        Commands::Clean { what } => {
            match what {
                None => {
                    // Default: clean outputs but preserve downloads
                    clean::clean_outputs(&base_dir)?;
                }
                Some(CleanTarget::Kernel) => {
                    clean::clean_kernel(&base_dir)?;
                }
                Some(CleanTarget::Iso) => {
                    clean::clean_iso(&base_dir)?;
                }
                Some(CleanTarget::Squashfs) => {
                    clean::clean_squashfs(&base_dir)?;
                }
                Some(CleanTarget::Downloads) => {
                    clean::clean_downloads(&base_dir)?;
                }
                Some(CleanTarget::All) => {
                    clean::clean_all(&base_dir)?;
                }
            }
        }

        // ===== SHOW =====
        Commands::Show { what } => {
            match what {
                ShowTarget::Config => {
                    config.print();
                }
                ShowTarget::Squashfs => {
                    let squashfs = base_dir.join("output/filesystem.squashfs");
                    if !squashfs.exists() {
                        anyhow::bail!("Squashfs not found. Run 'leviso build squashfs' first.");
                    }
                    // Use unsquashfs -l to list contents
                    let status = std::process::Command::new("unsquashfs")
                        .args(["-l", squashfs.to_str().unwrap()])
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("unsquashfs failed");
                    }
                }
            }
        }

        // ===== DOWNLOAD =====
        Commands::Download { what } => {
            match what {
                None => {
                    // Download everything
                    println!("Downloading all dependencies...\n");
                    download::download_rocky(&base_dir, &config.rocky)?;
                    if !config.has_linux_source() {
                        download_linux(&config, true)?;
                    } else {
                        println!("Linux source already exists at: {}", config.linux_source.display());
                    }
                }
                Some(DownloadTarget::Linux { full }) => {
                    if config.has_linux_source() {
                        println!("Linux source already exists at: {}", config.linux_source.display());
                        println!("To re-download, remove the directory first.");
                        return Ok(());
                    }
                    download_linux(&config, !full)?;
                }
                Some(DownloadTarget::Rocky) => {
                    download::download_rocky(&base_dir, &config.rocky)?;
                }
            }
        }

        // ===== EXTRACT =====
        Commands::Extract { what } => {
            match what {
                ExtractTarget::Rocky => {
                    extract::extract_rocky(&base_dir)?;
                }
                ExtractTarget::Squashfs { output } => {
                    let squashfs = base_dir.join("output/filesystem.squashfs");
                    if !squashfs.exists() {
                        anyhow::bail!("Squashfs not found. Run 'leviso build squashfs' first.");
                    }
                    let output_dir = output.unwrap_or_else(|| base_dir.join("output/squashfs-extracted"));
                    println!("Extracting squashfs to {}...", output_dir.display());
                    let status = std::process::Command::new("unsquashfs")
                        .args(["-d", output_dir.to_str().unwrap(), "-f", squashfs.to_str().unwrap()])
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("unsquashfs failed");
                    }
                    println!("Extracted to: {}", output_dir.display());
                }
            }
        }
    }

    Ok(())
}

/// Download Linux kernel source.
fn download_linux(config: &Config, shallow: bool) -> Result<()> {
    println!("Downloading Linux kernel source...");
    println!("  URL: {}", config.linux_git_url);
    println!("  Destination: {}", config.linux_source.display());

    if let Some(parent) = config.linux_source.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone");
    if shallow {
        cmd.args(["--depth", "1"]);
    }
    cmd.arg(&config.linux_git_url);
    cmd.arg(&config.linux_source);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("git clone failed");
    }

    println!("Linux source downloaded successfully.");
    Ok(())
}

/// Build the kernel.
fn build_kernel(base_dir: &Path, config: &Config, clean: bool) -> Result<()> {
    let output_dir = base_dir.join("output");

    if clean {
        let kernel_build = output_dir.join("kernel-build");
        if kernel_build.exists() {
            println!("Cleaning kernel build directory...");
            std::fs::remove_dir_all(&kernel_build)?;
        }
    }

    let version = build::kernel::build_kernel(&config.linux_source, &output_dir)?;

    build::kernel::install_kernel(
        &config.linux_source,
        &output_dir,
        &output_dir.join("staging"),
    )?;

    println!("\n=== Kernel build complete ===");
    println!("  Version: {}", version);
    println!("  Kernel:  output/staging/boot/vmlinuz");
    println!("  Modules: output/staging/usr/lib/modules/{}/", version);

    Ok(())
}
