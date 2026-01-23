//! Show command - displays information.

use anyhow::Result;
use std::path::Path;

use crate::config::Config;
use crate::process::Cmd;
use leviso_deps::DependencyResolver;

/// Show target for the show command.
pub enum ShowTarget {
    /// Show configuration
    Config,
    /// Show squashfs contents
    Squashfs,
}

/// Execute the show command.
pub fn cmd_show(
    base_dir: &Path,
    target: ShowTarget,
    config: &Config,
    resolver: &DependencyResolver,
) -> Result<()> {
    match target {
        ShowTarget::Config => {
            config.print();
            println!();
            resolver.print_status();
        }
        ShowTarget::Squashfs => {
            let squashfs = base_dir.join("output/filesystem.squashfs");
            if !squashfs.exists() {
                anyhow::bail!("Squashfs not found. Run 'leviso build squashfs' first.");
            }
            // Use unsquashfs -l to list contents
            Cmd::new("unsquashfs")
                .args(["-l"])
                .arg_path(&squashfs)
                .error_msg("unsquashfs failed. Install: sudo dnf install squashfs-tools")
                .run_interactive()?;
        }
    }
    Ok(())
}
