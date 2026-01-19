mod clean;
mod download;
mod extract;
mod initramfs;
mod iso;
mod qemu;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "leviso", about = "Build minimal bootable Linux ISO")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the ISO from scratch
    Build,
    /// Download Rocky ISO only
    Download,
    /// Extract files from Rocky ISO
    Extract,
    /// Build initramfs from extracted files
    Initramfs,
    /// Create bootable ISO
    Iso,
    /// Quick test: direct kernel boot in terminal (for debugging)
    Test {
        /// Command to run after boot (then exit)
        #[arg(short, long)]
        cmd: Option<String>,
    },
    /// Run the ISO in QEMU GUI (closest to bare metal)
    Run {
        /// Force BIOS boot instead of UEFI
        #[arg(long)]
        bios: bool,
    },
    /// Clean build artifacts
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    match cli.command {
        Commands::Build => {
            download::download_rocky(&base_dir)?;
            extract::extract_rocky(&base_dir)?;
            initramfs::build_initramfs(&base_dir)?;
            iso::create_iso(&base_dir)?;
        }
        Commands::Download => download::download_rocky(&base_dir)?,
        Commands::Extract => extract::extract_rocky(&base_dir)?,
        Commands::Initramfs => initramfs::build_initramfs(&base_dir)?,
        Commands::Iso => iso::create_iso(&base_dir)?,
        Commands::Test { cmd } => {
            initramfs::build_initramfs(&base_dir)?;
            qemu::test_direct(&base_dir, cmd)?;
        }
        Commands::Run { bios } => {
            initramfs::build_initramfs(&base_dir)?;
            iso::create_iso(&base_dir)?;
            qemu::run_iso(&base_dir, bios)?;
        }
        Commands::Clean => clean::clean(&base_dir)?,
    }

    Ok(())
}
