//! Download command - downloads dependencies.

use anyhow::Result;
use distro_spec::shared::LEVITATE_CARGO_TOOLS;
use std::path::Path;

use crate::recipe;

/// Download target for the download command.
pub enum DownloadTarget {
    /// Download all dependencies
    All,
    /// Download Linux kernel source
    Linux,
    /// Download Rocky ISO (via recipe)
    Rocky,
    /// Download installation tools (via recipe)
    Tools,
}

/// Execute the download command.
pub fn cmd_download(base_dir: &Path, target: DownloadTarget) -> Result<()> {
    match target {
        DownloadTarget::All => {
            println!("Resolving all dependencies...\n");

            // Rocky via recipe
            let rocky = recipe::rocky(base_dir)?;
            println!("Rocky: {} [OK]", rocky.iso.display());

            // Linux via recipe
            let linux = recipe::linux(base_dir)?;
            println!("Linux: {} [OK]", linux.source.display());

            // Tools via recipe
            recipe::install_tools(base_dir)?;

            println!("\nAll dependencies resolved.");
        }
        DownloadTarget::Linux => {
            let linux = recipe::linux(base_dir)?;
            println!("Linux source: {}", linux.source.display());
            if linux.is_installed() {
                println!("Kernel: {} (installed)", linux.version);
            }
        }
        DownloadTarget::Rocky => {
            // Use recipe for Rocky
            let rocky = recipe::rocky(base_dir)?;
            let status = if rocky.exists() { "OK" } else { "MISSING" };
            println!("Rocky (via recipe):");
            println!("  ISO:          {} [{}]", rocky.iso.display(), status);
            println!("  rootfs:       {}", rocky.rootfs.display());
            println!("  iso-contents: {}", rocky.iso_contents.display());
        }
        DownloadTarget::Tools => {
            println!("Installing tools via recipes...\n");
            recipe::install_tools(base_dir)?;

            // Show what was installed
            let staging_bin = base_dir.join("output/staging/usr/bin");
            println!("\nTools installed:");
            for tool in LEVITATE_CARGO_TOOLS {
                let path = staging_bin.join(tool);
                let status = if path.exists() { "OK" } else { "MISSING" };
                println!(
                    "  {:10} {} [{}]",
                    format!("{}:", tool),
                    path.display(),
                    status
                );
            }
        }
    }
    Ok(())
}
