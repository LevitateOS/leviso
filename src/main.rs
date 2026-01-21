mod clean;
mod config;
mod download;
mod extract;
mod initramfs;
mod iso;
mod qemu;
mod rocky_manifest;
mod rootfs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use config::Config;
use rootfs::RootfsBuilder;

#[derive(Parser)]
#[command(name = "leviso")]
#[command(about = "LevitateOS ISO and rootfs builder")]
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

    /// Quick test in terminal (direct kernel boot, for debugging)
    Test {
        /// Command to run after boot, then exit
        #[arg(short, long)]
        cmd: Option<String>,
    },

    /// Clean build artifacts
    Clean,

    /// Show information
    Show {
        #[command(subcommand)]
        what: ShowTarget,
    },

    /// Verify build outputs
    Verify {
        #[command(subcommand)]
        what: Option<VerifyTarget>,
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
    /// Build only the rootfs tarball
    Rootfs {
        /// Path to recipe binary (optional)
        #[arg(long)]
        recipe: Option<PathBuf>,
    },
    /// Build only the ISO image
    Iso,
    /// Build only the initramfs (for live boot)
    Initramfs,
}

#[derive(Subcommand)]
enum ShowTarget {
    /// Show current configuration
    Config,
    /// List tarball contents
    Tarball {
        /// Path to tarball (default: output/levitateos-base.tar.xz)
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum VerifyTarget {
    /// Verify tarball contains all required files
    Tarball {
        /// Path to tarball (default: output/levitateos-base.tar.xz)
        path: Option<PathBuf>,
    },
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
    /// Extract rootfs tarball for inspection
    Tarball {
        /// Path to tarball (default: output/levitateos-base.tar.xz)
        #[arg(short, long)]
        tarball: Option<PathBuf>,
        /// Output directory (default: output/rootfs)
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
                    // Full build: download deps, build kernel (if source available), rootfs, iso
                    println!("=== Full LevitateOS Build ===\n");

                    // 1. Download Rocky if needed
                    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
                        println!("Downloading Rocky Linux...");
                        download::download_rocky(&base_dir)?;
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

                    // 4. Build rootfs
                    println!("\nBuilding rootfs tarball...");
                    build_rootfs(&base_dir, None)?;

                    // 5. Build initramfs and ISO
                    println!("\nBuilding initramfs...");
                    initramfs::build_initramfs(&base_dir)?;

                    println!("\nBuilding ISO...");
                    iso::create_iso(&base_dir)?;

                    println!("\n=== Build Complete ===");
                    println!("  ISO: output/levitateos.iso");
                    println!("  Tarball: output/levitateos-base.tar.xz");
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
                Some(BuildTarget::Rootfs { recipe }) => {
                    build_rootfs(&base_dir, recipe)?;
                }
                Some(BuildTarget::Iso) => {
                    initramfs::build_initramfs(&base_dir)?;
                    iso::create_iso(&base_dir)?;
                }
                Some(BuildTarget::Initramfs) => {
                    initramfs::build_initramfs(&base_dir)?;
                }
            }
        }

        // ===== RUN =====
        Commands::Run { no_disk, disk_size } => {
            // Auto-build if ISO doesn't exist
            let iso_path = base_dir.join("output/levitateos.iso");
            if !iso_path.exists() {
                println!("ISO not found, building...\n");
                initramfs::build_initramfs(&base_dir)?;
                iso::create_iso(&base_dir)?;
            }
            let disk = if no_disk { None } else { Some(disk_size) };
            qemu::run_iso(&base_dir, disk)?;
        }

        Commands::Test { cmd } => {
            initramfs::build_initramfs(&base_dir)?;
            qemu::test_direct(&base_dir, cmd)?;
        }

        // ===== CLEAN =====
        Commands::Clean => {
            clean::clean(&base_dir)?;
        }

        // ===== SHOW =====
        Commands::Show { what } => {
            match what {
                ShowTarget::Config => {
                    config.print();
                }
                ShowTarget::Tarball { path } => {
                    let tarball = path.unwrap_or_else(|| base_dir.join("output/levitateos-base.tar.xz"));
                    rootfs::builder::list_tarball(&tarball)?;
                }
            }
        }

        // ===== VERIFY =====
        Commands::Verify { what } => {
            match what {
                None | Some(VerifyTarget::Tarball { path: None }) => {
                    let tarball = base_dir.join("output/levitateos-base.tar.xz");
                    rootfs::builder::verify_tarball(&tarball)?;
                }
                Some(VerifyTarget::Tarball { path: Some(p) }) => {
                    rootfs::builder::verify_tarball(&p)?;
                }
            }
        }

        // ===== DOWNLOAD =====
        Commands::Download { what } => {
            match what {
                None => {
                    // Download everything
                    println!("Downloading all dependencies...\n");
                    download::download_rocky(&base_dir)?;
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
                    download::download_rocky(&base_dir)?;
                }
            }
        }

        // ===== EXTRACT =====
        Commands::Extract { what } => {
            match what {
                ExtractTarget::Rocky => {
                    extract::extract_rocky(&base_dir)?;
                }
                ExtractTarget::Tarball { tarball, output } => {
                    let tarball_path = tarball.unwrap_or_else(|| {
                        base_dir.join("output/levitateos-base.tar.xz")
                    });
                    let output_dir = output.unwrap_or_else(|| {
                        base_dir.join("output/rootfs")
                    });
                    rootfs::builder::extract_tarball(&tarball_path, &output_dir)?;
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

    // Ensure parent directory exists
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
fn build_kernel(base_dir: &PathBuf, config: &Config, clean: bool) -> Result<()> {
    let output_dir = base_dir.join("output");

    if clean {
        let kernel_build = output_dir.join("kernel-build");
        if kernel_build.exists() {
            println!("Cleaning kernel build directory...");
            std::fs::remove_dir_all(&kernel_build)?;
        }
    }

    let version = rootfs::parts::kernel::build_kernel(
        &config.linux_source,
        &output_dir,
    )?;

    rootfs::parts::kernel::install_kernel(
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

/// Build the rootfs tarball.
fn build_rootfs(base_dir: &PathBuf, recipe: Option<PathBuf>) -> Result<()> {
    let iso_contents = base_dir.join("downloads/iso-contents");
    let rocky_rootfs = base_dir.join("downloads/rootfs");
    let output = base_dir.join("output");

    let mut builder = RootfsBuilder::new(&rocky_rootfs, &output);

    if iso_contents.join("BaseOS/Packages").exists() {
        builder = builder.with_iso_contents(&iso_contents);
    } else if rocky_rootfs.exists() {
        println!("Warning: Using extracted rootfs (may be incomplete)");
    } else {
        anyhow::bail!(
            "Rocky packages not found.\n\
             Run 'leviso download rocky' and 'leviso extract rocky' first."
        );
    }

    if let Some(recipe_path) = recipe {
        builder = builder.with_recipe(recipe_path);
    }

    let tarball = builder.build()?;
    println!("\nTarball created: {}", tarball.display());

    Ok(())
}
