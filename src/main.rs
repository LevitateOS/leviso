//! Leviso - LevitateOS ISO builder.
//!
//! Builds LevitateOS using squashfs architecture:
//! - Squashfs system image (complete live system, ~400MB)
//! - Tiny initramfs (mounts squashfs, ~5MB)
//! - Bootable ISO

mod artifact;
mod build;
mod cache;
mod clean;
mod commands;
mod component;
mod config;
mod extract;
mod preflight;
mod process;
mod qemu;
mod rebuild;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use config::Config;
use leviso_deps::DependencyResolver;

#[derive(Parser)]
#[command(name = "leviso")]
#[command(about = "LevitateOS ISO builder")]
#[command(after_help = "QUICK START:\n  leviso preflight  Check all dependencies\n  leviso build      Build everything\n  leviso run        Boot in QEMU\n  leviso clean      Remove build artifacts")]
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
    let resolver = DependencyResolver::new(&base_dir)?;
    let config = Config::load(); // .env loaded by resolver

    match cli.command {
        Commands::Build { target } => {
            let build_target = match target {
                None => commands::build::BuildTarget::Full,
                Some(BuildTarget::Kernel { clean }) => {
                    commands::build::BuildTarget::Kernel { clean }
                }
                Some(BuildTarget::Squashfs) => commands::build::BuildTarget::Squashfs,
                Some(BuildTarget::Initramfs) => commands::build::BuildTarget::Initramfs,
                Some(BuildTarget::Iso) => commands::build::BuildTarget::Iso,
            };
            commands::cmd_build(&base_dir, build_target, &resolver, &config)?;
        }

        Commands::Run { no_disk, disk_size } => {
            commands::cmd_run(&base_dir, no_disk, disk_size)?;
        }

        Commands::Clean { what } => {
            let clean_target = match what {
                None => commands::clean::CleanTarget::Outputs,
                Some(CleanTarget::Kernel) => commands::clean::CleanTarget::Kernel,
                Some(CleanTarget::Iso) => commands::clean::CleanTarget::Iso,
                Some(CleanTarget::Squashfs) => commands::clean::CleanTarget::Squashfs,
                Some(CleanTarget::Downloads) => commands::clean::CleanTarget::Downloads,
                Some(CleanTarget::Cache) => commands::clean::CleanTarget::Cache,
                Some(CleanTarget::All) => commands::clean::CleanTarget::All,
            };
            commands::cmd_clean(&base_dir, clean_target, &resolver)?;
        }

        Commands::Show { what } => {
            let show_target = match what {
                ShowTarget::Config => commands::show::ShowTarget::Config,
                ShowTarget::Squashfs => commands::show::ShowTarget::Squashfs,
            };
            commands::cmd_show(&base_dir, show_target, &config, &resolver)?;
        }

        Commands::Download { what } => {
            let download_target = match what {
                None => commands::download::DownloadTarget::All,
                Some(DownloadTarget::Linux { full: _ }) => commands::download::DownloadTarget::Linux,
                Some(DownloadTarget::Rocky) => commands::download::DownloadTarget::Rocky,
                Some(DownloadTarget::Tools) => commands::download::DownloadTarget::Tools,
            };
            commands::cmd_download(download_target, &resolver)?;
        }

        Commands::Extract { what } => {
            let extract_target = match what {
                ExtractTarget::Rocky => commands::extract::ExtractTarget::Rocky,
                ExtractTarget::Squashfs { output } => {
                    commands::extract::ExtractTarget::Squashfs { output }
                }
            };
            commands::cmd_extract(&base_dir, extract_target, &resolver)?;
        }

        Commands::Preflight { strict } => {
            commands::cmd_preflight(&base_dir, strict)?;
        }
    }

    Ok(())
}
