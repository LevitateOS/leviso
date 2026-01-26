//! Download command - downloads dependencies.

use anyhow::Result;
use std::path::Path;

use leviso_deps::DependencyResolver;

use crate::recipe;

/// Download target for the download command.
pub enum DownloadTarget {
    /// Download all dependencies
    All,
    /// Download Linux kernel source
    Linux,
    /// Download Rocky ISO (via recipe)
    Rocky,
    /// Download installation tools
    Tools,
}

/// Execute the download command.
pub fn cmd_download(
    base_dir: &Path,
    target: DownloadTarget,
    resolver: &DependencyResolver,
) -> Result<()> {
    match target {
        DownloadTarget::All => {
            println!("Resolving all dependencies...\n");

            // Rocky via recipe
            let rocky = recipe::rocky(base_dir)?;
            println!("Rocky: {} [OK]", rocky.iso.display());

            resolver.linux()?;
            let _ = resolver.all_tools()?;
            println!("\nAll dependencies resolved.");
        }
        DownloadTarget::Linux => {
            let linux = resolver.linux()?;
            println!("Linux source: {}", linux.path.display());
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
            println!("Resolving installation tools...\n");
            let (recstrap, recfstab, recchroot) = resolver.all_tools()?;
            println!("\nTools resolved:");
            for bin in [&recstrap, &recfstab, &recchroot] {
                let source = match bin.source {
                    leviso_deps::ToolSourceType::BuiltFromEnvVar => "built (env)",
                    leviso_deps::ToolSourceType::BuiltFromSubmodule => "built (submodule)",
                    leviso_deps::ToolSourceType::Downloaded => "downloaded",
                };
                let valid = if bin.is_valid() { "OK" } else { "MISSING" };
                println!(
                    "  {:10} {} [{}] ({})",
                    format!("{}:", bin.tool.name()),
                    bin.path.display(),
                    valid,
                    source
                );
            }
        }
    }
    Ok(())
}
