//! Leviso - LevitateOS ISO builder.
//!
//! Builds LevitateOS with EROFS-based architecture:
//! - EROFS rootfs image (complete live system, ~400MB)
//! - Tiny initramfs (mounts rootfs, ~5MB)
//! - Bootable ISO with UKI (systemd-boot)
#![allow(dead_code, unused_imports)]

mod artifact;
mod build;
mod cache;
mod clean;
mod commands;
mod common;
mod component;
mod config;
mod extract;
mod preflight;
mod qemu;
mod rebuild;
mod recipe;
mod timing;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use config::Config;

#[derive(Parser)]
#[command(name = "leviso")]
#[command(about = "LevitateOS ISO builder")]
#[command(
    after_help = "QUICK START:\n  leviso preflight  Check all dependencies\n  leviso build      Build everything\n  leviso run        Boot in QEMU\n  leviso clean      Remove build artifacts"
)]
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

        /// Build the kernel from source (~1 hour). Requires --dangerously-waste-the-users-time.
        #[arg(long)]
        kernel: bool,

        /// Confirm that you really want to spend ~1 hour building the kernel.
        #[arg(long)]
        dangerously_waste_the_users_time: bool,
    },

    /// Run the ISO in QEMU (UEFI boot, GUI)
    Run {
        /// Don't attach virtual disk
        #[arg(long)]
        no_disk: bool,
        /// Virtual disk size (default: 8G)
        #[arg(long, default_value = "8G")]
        disk_size: String,
    },

    /// Test the ISO boots correctly (headless, automated)
    Test {
        /// Timeout in seconds (default: 120)
        #[arg(short, long, default_value = "120")]
        timeout: u64,
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

    /// Run preflight checks (verify all dependencies before build)
    Preflight {
        /// Fail if any checks fail (exit code 1)
        #[arg(long)]
        strict: bool,
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
    /// Build rootfs image (EROFS, complete live system)
    Rootfs,
    /// Build tiny initramfs (mounts rootfs, ~5MB)
    Initramfs,
    /// Build only the ISO image
    Iso,
    /// Build VM disk image (qcow2)
    Qcow2 {
        /// Disk size in GB (default: 256, sparse allocation)
        #[arg(long, default_value = "256")]
        disk_size: u32,
    },
}

#[derive(Subcommand)]
enum ShowTarget {
    /// Show current configuration
    Config,
    /// Show rootfs contents (EROFS)
    Rootfs,
    /// Show build status (what needs rebuilding)
    Status,
}

#[derive(Subcommand)]
enum CleanTarget {
    /// Clean kernel build artifacts only
    Kernel,
    /// Clean ISO and initramfs only
    Iso,
    /// Clean rootfs only (EROFS image + staging)
    Rootfs,
    /// Clean downloaded sources (Rocky ISO, Linux source)
    Downloads,
    /// Clean cached tool binaries (~/.cache/levitate/)
    Cache,
    /// Clean everything (downloads + outputs + cache)
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
    /// Build or download installation tools (recstrap, recfstab, recchroot)
    Tools,
}

#[derive(Subcommand)]
enum ExtractTarget {
    /// Extract Rocky ISO contents
    Rocky,
    /// Extract rootfs for inspection (extracts EROFS)
    Rootfs {
        /// Output directory (default: output/rootfs-extracted)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Load .env if present
    dotenvy::dotenv().ok();
    let config = Config::load();

    match cli.command {
        Commands::Build {
            target,
            kernel,
            dangerously_waste_the_users_time,
        } => {
            use distro_contract::kernel::{KernelBuildGuard, KernelGuard};
            let build_target = match target {
                Some(BuildTarget::Kernel { clean }) => {
                    KernelGuard::new(
                        true,
                        dangerously_waste_the_users_time,
                        "cargo run -- build kernel --dangerously-waste-the-users-time",
                    )
                    .require_kernel_confirmation();
                    commands::build::BuildTarget::Kernel { clean }
                }
                None if kernel => {
                    KernelGuard::new(
                        true,
                        dangerously_waste_the_users_time,
                        "cargo run -- build --kernel --dangerously-waste-the-users-time",
                    )
                    .require_kernel_confirmation();
                    commands::build::BuildTarget::FullWithKernel
                }
                None => commands::build::BuildTarget::Full,
                Some(BuildTarget::Rootfs) => commands::build::BuildTarget::Rootfs,
                Some(BuildTarget::Initramfs) => commands::build::BuildTarget::Initramfs,
                Some(BuildTarget::Iso) => commands::build::BuildTarget::Iso,
                Some(BuildTarget::Qcow2 { disk_size }) => {
                    commands::build::BuildTarget::Qcow2 { disk_size }
                }
            };
            commands::cmd_build(&base_dir, build_target, &config)?;
        }

        Commands::Run { no_disk, disk_size } => {
            commands::cmd_run(&base_dir, no_disk, disk_size)?;
        }

        Commands::Test { timeout } => {
            commands::cmd_test(&base_dir, timeout)?;
        }

        Commands::Clean { what } => {
            let clean_target = match what {
                None => commands::clean::CleanTarget::Outputs,
                Some(CleanTarget::Kernel) => commands::clean::CleanTarget::Kernel,
                Some(CleanTarget::Iso) => commands::clean::CleanTarget::Iso,
                Some(CleanTarget::Rootfs) => commands::clean::CleanTarget::Rootfs,
                Some(CleanTarget::Downloads) => commands::clean::CleanTarget::Downloads,
                Some(CleanTarget::Cache) => commands::clean::CleanTarget::Cache,
                Some(CleanTarget::All) => commands::clean::CleanTarget::All,
            };
            commands::cmd_clean(&base_dir, clean_target)?;
        }

        Commands::Show { what } => {
            let show_target = match what {
                ShowTarget::Config => commands::show::ShowTarget::Config,
                ShowTarget::Rootfs => commands::show::ShowTarget::Rootfs,
                ShowTarget::Status => commands::show::ShowTarget::Status,
            };
            commands::cmd_show(&base_dir, show_target, &config)?;
        }

        Commands::Download { what } => {
            let download_target = match what {
                None => commands::download::DownloadTarget::All,
                Some(DownloadTarget::Linux { full: _ }) => {
                    commands::download::DownloadTarget::Linux
                }
                Some(DownloadTarget::Rocky) => commands::download::DownloadTarget::Rocky,
                Some(DownloadTarget::Tools) => commands::download::DownloadTarget::Tools,
            };
            commands::cmd_download(&base_dir, download_target)?;
        }

        Commands::Extract { what } => {
            let extract_target = match what {
                ExtractTarget::Rocky => commands::extract::ExtractTarget::Rocky,
                ExtractTarget::Rootfs { output } => {
                    commands::extract::ExtractTarget::Rootfs { output }
                }
            };
            commands::cmd_extract(&base_dir, extract_target)?;
        }

        Commands::Preflight { strict } => {
            commands::cmd_preflight(&base_dir, strict)?;
        }
    }

    Ok(())
}
