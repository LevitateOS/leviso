//! Installation tool resolution (recstrap, recfstab, recchroot).

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;

use super::download::{self, DownloadOptions};
use crate::process::Cmd;
use super::DependencyResolver;

/// Get GitHub org for tool downloads from environment or use default.
fn github_org() -> String {
    env::var("GITHUB_ORG").unwrap_or_else(|_| "LevitateOS".to_string())
}

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
    /// Check if the binary exists and is a valid executable file.
    pub fn is_valid(&self) -> bool {
        if !self.path.exists() {
            return false;
        }

        // Check it's a file, not a directory
        match std::fs::metadata(&self.path) {
            Ok(meta) => {
                if !meta.is_file() {
                    return false;
                }
                // On Unix, check executable bit
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    if mode & 0o111 == 0 {
                        return false; // Not executable
                    }
                }
                true
            }
            Err(_) => false,
        }
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
    download_tool(resolver, tool)
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

    let mut cmd = Cmd::new("cargo").arg("build").dir(crate_path);
    if release_build {
        cmd = cmd.arg("--release");
        println!("    Profile: release");
    } else {
        println!("    Profile: debug");
    }

    cmd.error_msg(&format!("cargo build failed for {}", tool.name()))
        .run()?;

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
fn download_tool(resolver: &DependencyResolver, tool: Tool) -> Result<ToolBinary> {
    let binary_path = resolver.cache_dir().join(tool.name());

    // Check if already cached and valid
    if binary_path.exists() {
        let cached = ToolBinary {
            path: binary_path.clone(),
            source: ToolSourceType::Downloaded,
            tool,
        };
        if cached.is_valid() {
            println!("  {} (cached): {}", tool.name(), binary_path.display());
            return Ok(cached);
        } else {
            // Cached binary is invalid (not executable, corrupted, etc.) - remove and re-download
            println!("  {} cached binary invalid, re-downloading...", tool.name());
            std::fs::remove_file(&binary_path).ok();
        }
    }

    println!("  Downloading {} from GitHub...", tool.name());

    // Get latest release URL
    let release_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        github_org(),
        tool.repo()
    );

    // Use centralized HTTP download for API call
    let rt = tokio::runtime::Runtime::new()?;
    let release_json = rt.block_on(fetch_github_release(&release_url))?;

    // Parse to find tarball URL
    let tarball_url = extract_asset_url(&release_json, &tool.tarball_name())
        .with_context(|| {
            format!(
                "Failed to find {} in release assets for {}",
                tool.tarball_name(),
                tool.name()
            )
        })?;

    println!("    URL: {}", tarball_url);

    // Download tarball using centralized download
    let tarball_path = resolver.cache_dir().join(tool.tarball_name());
    rt.block_on(download::http(
        &tarball_url,
        &tarball_path,
        &DownloadOptions::default(),
    ))
    .with_context(|| format!("Failed to download {} from {}", tool.tarball_name(), tarball_url))?;

    // Extract binary using centralized extraction
    rt.block_on(download::extract_file_from_tarball(
        &tarball_path,
        resolver.cache_dir(),
        tool.name(),
    ))
    .with_context(|| {
        format!(
            "Failed to extract {} from {}",
            tool.name(),
            tarball_path.display()
        )
    })?;

    // Clean up tarball
    let _ = std::fs::remove_file(&tarball_path);

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)
            .with_context(|| format!("Failed to get metadata for {}", binary_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)
            .with_context(|| format!("Failed to set permissions on {}", binary_path.display()))?;
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

/// Fetch GitHub release info via API.
async fn fetch_github_release(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("leviso/0.1")
        .build()
        .context("Failed to create HTTP client")?;

    // Check for GitHub token in environment
    let github_token = env::var("GITHUB_TOKEN").ok();

    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(30));

    // Add auth header if token is available
    if let Some(token) = &github_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to fetch GitHub release: {}", url))?;

    let status = response.status();
    if !status.is_success() {
        // Provide helpful error messages for common issues
        let error_msg = match status.as_u16() {
            403 => {
                if github_token.is_some() {
                    format!(
                        "GitHub API rate limit exceeded or token invalid (HTTP 403)\n\
                         Check your GITHUB_TOKEN permissions or wait for rate limit reset."
                    )
                } else {
                    format!(
                        "GitHub API rate limit exceeded (HTTP 403)\n\
                         Set GITHUB_TOKEN environment variable to increase rate limit:\n\
                         export GITHUB_TOKEN=ghp_your_personal_access_token"
                    )
                }
            }
            429 => format!(
                "GitHub API rate limit exceeded (HTTP 429)\n\
                 Set GITHUB_TOKEN environment variable or wait for rate limit reset."
            ),
            404 => format!(
                "GitHub release not found (HTTP 404)\n\
                 Check that {} exists and has published releases.",
                url
            ),
            _ => format!("GitHub API error: HTTP {} for {}", status, url),
        };
        bail!("{}", error_msg);
    }

    response
        .text()
        .await
        .with_context(|| format!("Failed to read response from {}", url))
}

/// Extract asset download URL from GitHub release JSON.
fn extract_asset_url(json: &str, asset_name: &str) -> Result<String> {
    // Find the asset with our name
    let asset_marker = format!("\"name\":\"{}\"", asset_name);
    let asset_pos = json.find(&asset_marker).with_context(|| {
        format!(
            "Asset '{}' not found in GitHub release. Available assets may not include this platform.",
            asset_name
        )
    })?;

    // Find browser_download_url near this position
    let search_region =
        &json[asset_pos.saturating_sub(500)..asset_pos.saturating_add(500).min(json.len())];

    let url_marker = "\"browser_download_url\":\"";
    let url_start = search_region
        .find(url_marker)
        .context("browser_download_url not found in release JSON")?;

    let url_content = &search_region[url_start + url_marker.len()..];
    let url_end = url_content
        .find('"')
        .context("Malformed browser_download_url in release JSON")?;

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
