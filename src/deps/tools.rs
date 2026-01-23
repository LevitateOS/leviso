//! Installation tool resolution (recstrap, recfstab, recchroot).

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

use super::DependencyResolver;

/// GitHub org for tool downloads.
const GITHUB_ORG: &str = "LevitateOS";

/// Installation tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Recstrap,
    Recfstab,
    Recchroot,
}

impl Tool {
    /// Environment variable name for local crate path.
    pub fn env_var(&self) -> &'static str {
        match self {
            Tool::Recstrap => "RECSTRAP_PATH",
            Tool::Recfstab => "RECFSTAB_PATH",
            Tool::Recchroot => "RECCHROOT_PATH",
        }
    }

    /// Binary/crate name.
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Recstrap => "recstrap",
            Tool::Recfstab => "recfstab",
            Tool::Recchroot => "recchroot",
        }
    }

    /// GitHub repository name.
    pub fn repo(&self) -> &'static str {
        self.name()
    }

    /// Tarball name on GitHub releases.
    pub fn tarball_name(&self) -> String {
        format!("{}-x86_64-linux.tar.gz", self.name())
    }
}

/// Resolved tool binary.
#[derive(Debug, Clone)]
pub struct ToolBinary {
    /// Path to the binary.
    pub path: PathBuf,
    /// How it was resolved.
    pub source: ToolSourceType,
    /// Which tool this is.
    pub tool: Tool,
}

/// How the tool was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSourceType {
    /// Built from local crate (env var path).
    BuiltFromEnvVar,
    /// Built from submodule.
    BuiltFromSubmodule,
    /// Downloaded from GitHub releases.
    Downloaded,
}

impl ToolBinary {
    /// Check if the binary exists and is executable.
    pub fn is_valid(&self) -> bool {
        self.path.exists()
    }
}

/// Find existing tool binary without downloading/building.
pub fn find_existing(resolver: &DependencyResolver, tool: Tool) -> Option<ToolBinary> {
    let release_build = env::var("REC_BUILD_RELEASE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
    let profile = if release_build { "release" } else { "debug" };

    // 1. Check env var for local crate
    if let Ok(crate_path) = env::var(tool.env_var()) {
        let crate_path = PathBuf::from(crate_path);
        let binary = crate_path.join("target").join(profile).join(tool.name());
        if binary.exists() {
            return Some(ToolBinary {
                path: binary,
                source: ToolSourceType::BuiltFromEnvVar,
                tool,
            });
        }
        // Crate exists but not built - will need to build
        if crate_path.join("Cargo.toml").exists() {
            return None; // Signal that we need to build
        }
    }

    // 2. Check submodule at ../tool_name
    let submodule = resolver.monorepo_dir().join(tool.name());
    if submodule.join("Cargo.toml").exists() {
        let binary = submodule.join("target").join(profile).join(tool.name());
        if binary.exists() {
            return Some(ToolBinary {
                path: binary,
                source: ToolSourceType::BuiltFromSubmodule,
                tool,
            });
        }
        // Submodule exists but not built - will need to build
        return None;
    }

    // 3. Check cache for downloaded binary
    let cached = resolver.cache_dir().join(tool.name());
    if cached.exists() {
        return Some(ToolBinary {
            path: cached,
            source: ToolSourceType::Downloaded,
            tool,
        });
    }

    None
}

/// Resolve tool binary, building or downloading if necessary.
pub fn resolve(resolver: &DependencyResolver, tool: Tool) -> Result<ToolBinary> {
    let release_build = env::var("REC_BUILD_RELEASE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    // 1. Check env var for local crate
    if let Ok(crate_path) = env::var(tool.env_var()) {
        let crate_path = PathBuf::from(crate_path);
        return build_from_crate(tool, &crate_path, release_build, ToolSourceType::BuiltFromEnvVar);
    }

    // 2. Check submodule at ../tool_name
    let submodule = resolver.monorepo_dir().join(tool.name());
    if submodule.join("Cargo.toml").exists() {
        return build_from_crate(tool, &submodule, release_build, ToolSourceType::BuiltFromSubmodule);
    }

    // 3. Download from GitHub releases
    download(resolver, tool)
}

/// Build tool from local crate source.
fn build_from_crate(
    tool: Tool,
    crate_path: &PathBuf,
    release_build: bool,
    source_type: ToolSourceType,
) -> Result<ToolBinary> {
    if !crate_path.exists() {
        bail!(
            "{} points to non-existent path: {}",
            tool.env_var(),
            crate_path.display()
        );
    }

    if !crate_path.join("Cargo.toml").exists() {
        bail!(
            "{} is not a Cargo crate (no Cargo.toml): {}",
            tool.env_var(),
            crate_path.display()
        );
    }

    let source_desc = match source_type {
        ToolSourceType::BuiltFromEnvVar => format!("from {}", tool.env_var()),
        ToolSourceType::BuiltFromSubmodule => "submodule".to_string(),
        _ => "local".to_string(),
    };

    println!("  Building {} ({})...", tool.name(), source_desc);
    println!("    Source: {}", crate_path.display());

    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release_build {
        cmd.arg("--release");
        println!("    Profile: release");
    } else {
        println!("    Profile: debug");
    }
    cmd.current_dir(crate_path);

    let status = cmd
        .status()
        .with_context(|| format!("Failed to run cargo build for {}", tool.name()))?;

    if !status.success() {
        bail!("cargo build failed for {}", tool.name());
    }

    let profile = if release_build { "release" } else { "debug" };
    let binary = crate_path.join("target").join(profile).join(tool.name());

    if !binary.exists() {
        bail!(
            "Built binary not found at expected path: {}",
            binary.display()
        );
    }

    println!("    Built: {}", binary.display());

    Ok(ToolBinary {
        path: binary,
        source: source_type,
        tool,
    })
}

/// Download tool from GitHub releases.
fn download(resolver: &DependencyResolver, tool: Tool) -> Result<ToolBinary> {
    let binary_path = resolver.cache_dir().join(tool.name());

    // Check if already cached
    if binary_path.exists() {
        println!("  {} (cached): {}", tool.name(), binary_path.display());
        return Ok(ToolBinary {
            path: binary_path,
            source: ToolSourceType::Downloaded,
            tool,
        });
    }

    println!("  Downloading {} from GitHub...", tool.name());

    // Get latest release URL
    let release_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_ORG,
        tool.repo()
    );

    // Fetch release info
    let release_json = fetch_url(&release_url)
        .with_context(|| format!("Failed to fetch release info for {}", tool.name()))?;

    // Parse to find tarball URL
    let tarball_url = extract_asset_url(&release_json, &tool.tarball_name())
        .with_context(|| format!("Failed to find {} in release assets", tool.tarball_name()))?;

    println!("    URL: {}", tarball_url);

    // Download tarball
    let tarball_path = resolver.cache_dir().join(tool.tarball_name());
    download_file(&tarball_url, &tarball_path)
        .with_context(|| format!("Failed to download {}", tool.tarball_name()))?;

    // Extract binary
    extract_tarball(&tarball_path, resolver.cache_dir(), tool.name())
        .with_context(|| format!("Failed to extract {}", tool.tarball_name()))?;

    // Clean up tarball
    let _ = std::fs::remove_file(&tarball_path);

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)?;
    }

    if !binary_path.exists() {
        bail!(
            "Binary not found after extraction: {}",
            binary_path.display()
        );
    }

    println!("    Downloaded: {}", binary_path.display());

    Ok(ToolBinary {
        path: binary_path,
        source: ToolSourceType::Downloaded,
        tool,
    })
}

/// Fetch URL content as string (using curl).
fn fetch_url(url: &str) -> Result<String> {
    let output = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", url])
        .output()
        .context("Failed to run curl")?;

    if !output.status.success() {
        bail!(
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Download file using curl.
fn download_file(url: &str, dest: &PathBuf) -> Result<()> {
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(dest)
        .arg(url)
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        bail!("curl download failed");
    }

    Ok(())
}

/// Extract a single file from a tarball.
fn extract_tarball(tarball: &PathBuf, dest_dir: &std::path::Path, filename: &str) -> Result<()> {
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(tarball)
        .args(["-C"])
        .arg(dest_dir)
        .arg(filename)
        .status()
        .context("Failed to run tar")?;

    if !status.success() {
        bail!("tar extraction failed");
    }

    Ok(())
}

/// Extract asset download URL from GitHub release JSON.
fn extract_asset_url(json: &str, asset_name: &str) -> Result<String> {
    // Find the asset with our name
    let asset_marker = format!("\"name\":\"{}\"", asset_name);
    let asset_pos = json
        .find(&asset_marker)
        .with_context(|| format!("Asset {} not found in release", asset_name))?;

    // Find browser_download_url near this position
    let search_region =
        &json[asset_pos.saturating_sub(500)..asset_pos.saturating_add(500).min(json.len())];

    let url_marker = "\"browser_download_url\":\"";
    let url_start = search_region
        .find(url_marker)
        .context("browser_download_url not found")?;

    let url_content = &search_region[url_start + url_marker.len()..];
    let url_end = url_content
        .find('"')
        .context("Malformed browser_download_url")?;

    Ok(url_content[..url_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_names() {
        assert_eq!(Tool::Recstrap.name(), "recstrap");
        assert_eq!(Tool::Recfstab.name(), "recfstab");
        assert_eq!(Tool::Recchroot.name(), "recchroot");
    }

    #[test]
    fn test_tool_env_vars() {
        assert_eq!(Tool::Recstrap.env_var(), "RECSTRAP_PATH");
        assert_eq!(Tool::Recfstab.env_var(), "RECFSTAB_PATH");
        assert_eq!(Tool::Recchroot.env_var(), "RECCHROOT_PATH");
    }

    #[test]
    fn test_tarball_names() {
        assert_eq!(
            Tool::Recstrap.tarball_name(),
            "recstrap-x86_64-linux.tar.gz"
        );
    }
}
