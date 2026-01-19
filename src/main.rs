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
    /// Test the ISO in QEMU
    Test {
        /// Use QEMU GUI instead of serial console
        #[arg(long)]
        gui: bool,
        /// Force BIOS boot instead of UEFI (for legacy testing)
        #[arg(long)]
        bios: bool,
    },
    /// Direct kernel boot (faster debugging, no ISO needed)
    Run {
        /// Use QEMU GUI instead of serial console
        #[arg(long)]
        gui: bool,
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
        Commands::Test { gui, bios } => qemu::test_qemu(&base_dir, gui, bios)?,
        Commands::Run { gui } => qemu::test_direct(&base_dir, gui)?,
        Commands::Clean => clean::clean(&base_dir)?,
    }

    Ok(())
}
