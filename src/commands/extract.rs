//! Extract command - extracts archives for inspection.

use anyhow::Result;
use std::path::{Path, PathBuf};

use distro_spec::levitate::ROOTFS_NAME;

use crate::extract;
use distro_builder::process::Cmd;

/// Extract target for the extract command.
pub enum ExtractTarget {
    /// Extract Rocky ISO
    Rocky,
    /// Extract rootfs (EROFS)
    Rootfs { output: Option<PathBuf> },
}

/// Execute the extract command.
pub fn cmd_extract(base_dir: &Path, target: ExtractTarget) -> Result<()> {
    match target {
        ExtractTarget::Rocky => {
            // Use recipe to ensure Rocky is available, then extract
            let rocky = crate::recipe::rocky(base_dir)?;
            extract::extract_rocky_iso(base_dir, &rocky.iso)?;
        }
        ExtractTarget::Rootfs { output } => {
            let rootfs = base_dir.join("output").join(ROOTFS_NAME);
            if !rootfs.exists() {
                anyhow::bail!("Rootfs not found. Run 'leviso build rootfs' first.");
            }
            let output_dir = output.unwrap_or_else(|| base_dir.join("output/rootfs-extracted"));
            println!("Extracting EROFS rootfs to {}...", output_dir.display());
            // EROFS extraction requires mounting or using fsck.erofs --extract
            // For inspection, dump the image info instead
            Cmd::new("fsck.erofs")
                .args(["--extract", &output_dir.to_string_lossy()])
                .arg_path(&rootfs)
                .error_msg("fsck.erofs failed. Install: sudo dnf install erofs-utils")
                .run_interactive()?;
            println!("Extracted to: {}", output_dir.display());
        }
    }
    Ok(())
}
