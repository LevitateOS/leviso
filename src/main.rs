mod clean;
mod download;
mod extract;
mod initramfs;
mod iso;
mod qemu;
mod rootfs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use rootfs::RootfsBuilder;

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
    /// Build base system tarball for installation
    Rootfs {
        /// Path to recipe binary (optional)
        #[arg(long)]
        recipe: Option<PathBuf>,
    },
    /// List contents of a base tarball
    RootfsList {
        /// Path to tarball
        path: PathBuf,
    },
    /// Verify base tarball contains essential files
    RootfsVerify {
        /// Path to tarball
        path: PathBuf,
    },
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
        /// Don't attach the virtual disk
        #[arg(long)]
        no_disk: bool,
        /// Virtual disk size (default: 8G)
        #[arg(long, default_value = "8G")]
        disk_size: String,
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
        Commands::Rootfs { recipe } => {
            // Build base system tarball
            let rocky_rootfs = base_dir.join("downloads/rootfs");
            let output = base_dir.join("output");

            if !rocky_rootfs.exists() {
                anyhow::bail!(
                    "Rocky rootfs not found at {}. Run 'leviso extract' first.",
                    rocky_rootfs.display()
                );
            }

            let mut builder = RootfsBuilder::new(&rocky_rootfs, &output);
            if let Some(recipe_path) = recipe {
                builder = builder.with_recipe(recipe_path);
            }

            let tarball = builder.build()?;
            println!("\nBase tarball created: {}", tarball.display());
        }
        Commands::RootfsList { path } => {
            rootfs::builder::list_tarball(&path)?;
        }
        Commands::RootfsVerify { path } => {
            rootfs::builder::verify_tarball(&path)?;
        }
        Commands::Test { cmd } => {
            initramfs::build_initramfs(&base_dir)?;
            qemu::test_direct(&base_dir, cmd)?;
        }
        Commands::Run { bios, no_disk, disk_size } => {
            initramfs::build_initramfs(&base_dir)?;
            iso::create_iso(&base_dir)?;
            let disk = if no_disk { None } else { Some(disk_size) };
            qemu::run_iso(&base_dir, bios, disk)?;
        }
        Commands::Clean => clean::clean(&base_dir)?,
    }

    Ok(())
}
