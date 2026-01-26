//! Extract command - extracts archives for inspection.

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::extract;
use distro_builder::process::Cmd;

/// Extract target for the extract command.
pub enum ExtractTarget {
    /// Extract Rocky ISO
    Rocky,
    /// Extract squashfs
    Squashfs { output: Option<PathBuf> },
}

/// Execute the extract command.
pub fn cmd_extract(base_dir: &Path, target: ExtractTarget) -> Result<()> {
    match target {
        ExtractTarget::Rocky => {
            // Use recipe to ensure Rocky is available, then extract
            let rocky = crate::recipe::rocky(base_dir)?;
            extract::extract_rocky_iso(base_dir, &rocky.iso)?;
        }
        ExtractTarget::Squashfs { output } => {
            let squashfs = base_dir.join("output/filesystem.squashfs");
            if !squashfs.exists() {
                anyhow::bail!("Squashfs not found. Run 'leviso build squashfs' first.");
            }
            let output_dir = output.unwrap_or_else(|| base_dir.join("output/squashfs-extracted"));
            println!("Extracting squashfs to {}...", output_dir.display());
            Cmd::new("unsquashfs")
                .args(["-d"])
                .arg_path(&output_dir)
                .arg("-f")
                .arg_path(&squashfs)
                .error_msg("unsquashfs failed. Install: sudo dnf install squashfs-tools")
                .run_interactive()?;
            println!("Extracted to: {}", output_dir.display());
        }
    }
    Ok(())
}
