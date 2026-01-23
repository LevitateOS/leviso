//! Download command - downloads dependencies.

use anyhow::Result;

use leviso_deps::DependencyResolver;

/// Download target for the download command.
pub enum DownloadTarget {
    /// Download all dependencies
    All,
    /// Download Linux kernel source
    Linux,
    /// Download Rocky ISO
    Rocky,
    /// Download installation tools
    Tools,
}

/// Execute the download command.
pub fn cmd_download(target: DownloadTarget, resolver: &DependencyResolver) -> Result<()> {
    match target {
        DownloadTarget::All => {
            println!("Resolving all dependencies...\n");
            resolver.rocky_iso()?;
            resolver.linux()?;
            let _ = resolver.all_tools()?;
            println!("\nAll dependencies resolved.");
        }
        DownloadTarget::Linux => {
            let linux = resolver.linux()?;
            println!("Linux source: {}", linux.path.display());
        }
        DownloadTarget::Rocky => {
            let rocky = resolver.rocky_iso()?;
            let status = if rocky.is_valid() { "OK" } else { "MISSING" };
            println!("Rocky ISO: {} [{}]", rocky.path.display(), status);
            println!("  Version: {} ({})", rocky.config.version, rocky.config.arch);
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
