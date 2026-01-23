//! Unified dependency resolution for LevitateOS build.
//!
//! All external dependencies follow the same pattern:
//! 1. Check env var override (e.g., `LINUX_SOURCE`, `RECSTRAP_PATH`)
//! 2. Check for submodule at `../name` (auto-detect monorepo)
//! 3. Fall back to download (git clone, tarball, or curl)
//!
//! # Example
//!
//! ```no_run
//! use leviso::deps::DependencyResolver;
//!
//! let resolver = DependencyResolver::new("/path/to/leviso")?;
//!
//! // Get paths to dependencies (downloads if needed)
//! let linux = resolver.linux()?;           // Kernel source tree
//! let recstrap = resolver.recstrap()?;     // Binary path
//! let rocky_iso = resolver.rocky_iso()?;   // ISO path
//! ```

mod linux;
mod rocky;
mod tools;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub use linux::LinuxSource;
pub use rocky::RockyIso;
pub use tools::{Tool, ToolBinary};

/// Unified resolver for all LevitateOS build dependencies.
pub struct DependencyResolver {
    /// Base directory (leviso crate root)
    base_dir: PathBuf,
    /// Parent directory (monorepo root, for submodule detection)
    monorepo_dir: PathBuf,
    /// Cache directory for downloads (~/.cache/levitate/)
    cache_dir: PathBuf,
    /// Downloads directory (leviso/downloads/)
    downloads_dir: PathBuf,
}

impl DependencyResolver {
    /// Create a new dependency resolver.
    ///
    /// `base_dir` should be the leviso crate root.
    pub fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
        // Load .env file if present
        dotenvy::dotenv().ok();

        let base_dir = base_dir.as_ref().to_path_buf();
        let monorepo_dir = base_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| base_dir.clone());

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("levitate");

        let downloads_dir = base_dir.join("downloads");

        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache dir: {}", cache_dir.display()))?;
        std::fs::create_dir_all(&downloads_dir)
            .with_context(|| format!("Failed to create downloads dir: {}", downloads_dir.display()))?;

        Ok(Self {
            base_dir,
            monorepo_dir,
            cache_dir,
            downloads_dir,
        })
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the monorepo directory.
    pub fn monorepo_dir(&self) -> &Path {
        &self.monorepo_dir
    }

    /// Get the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the downloads directory.
    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    // =========================================================================
    // Linux Kernel
    // =========================================================================

    /// Resolve Linux kernel source.
    ///
    /// Resolution order:
    /// 1. `LINUX_SOURCE` env var
    /// 2. `../linux` submodule (monorepo)
    /// 3. Download via git clone to `downloads/linux`
    pub fn linux(&self) -> Result<LinuxSource> {
        linux::resolve(self)
    }

    /// Check if Linux source is available without downloading.
    pub fn has_linux(&self) -> bool {
        linux::find_existing(self).is_some()
    }

    // =========================================================================
    // Installation Tools (recstrap, recfstab, recchroot)
    // =========================================================================

    /// Resolve recstrap binary.
    ///
    /// Resolution order:
    /// 1. `RECSTRAP_PATH` env var (crate path, builds from source)
    /// 2. `../recstrap` submodule (monorepo, builds from source)
    /// 3. Download pre-built binary from GitHub releases
    pub fn recstrap(&self) -> Result<ToolBinary> {
        tools::resolve(self, Tool::Recstrap)
    }

    /// Resolve recfstab binary.
    pub fn recfstab(&self) -> Result<ToolBinary> {
        tools::resolve(self, Tool::Recfstab)
    }

    /// Resolve recchroot binary.
    pub fn recchroot(&self) -> Result<ToolBinary> {
        tools::resolve(self, Tool::Recchroot)
    }

    /// Resolve all installation tools.
    pub fn all_tools(&self) -> Result<(ToolBinary, ToolBinary, ToolBinary)> {
        Ok((self.recstrap()?, self.recfstab()?, self.recchroot()?))
    }

    // =========================================================================
    // Rocky Linux ISO
    // =========================================================================

    /// Resolve Rocky Linux ISO.
    ///
    /// Resolution order:
    /// 1. `ROCKY_ISO_PATH` env var (existing ISO file)
    /// 2. Check `downloads/` for existing ISO
    /// 3. Download from Rocky mirrors
    pub fn rocky_iso(&self) -> Result<RockyIso> {
        rocky::resolve(self)
    }

    /// Check if Rocky ISO is available without downloading.
    pub fn has_rocky_iso(&self) -> bool {
        rocky::find_existing(self).is_some()
    }

    // =========================================================================
    // Cache Management
    // =========================================================================

    /// Clear the download cache.
    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
            std::fs::create_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    /// Print resolved dependency status.
    pub fn print_status(&self) {
        println!("Dependency Status:");
        println!("  Base dir:     {}", self.base_dir.display());
        println!("  Monorepo dir: {}", self.monorepo_dir.display());
        println!("  Cache dir:    {}", self.cache_dir.display());
        println!("  Downloads:    {}", self.downloads_dir.display());
        println!();

        // Linux
        match linux::find_existing(self) {
            Some(src) => println!("  Linux:    FOUND at {}", src.path.display()),
            None => println!("  Linux:    NOT FOUND (will download)"),
        }

        // Tools
        for tool in [Tool::Recstrap, Tool::Recfstab, Tool::Recchroot] {
            match tools::find_existing(self, tool) {
                Some(bin) => println!("  {}:  FOUND at {}", tool.name(), bin.path.display()),
                None => println!("  {}:  NOT FOUND (will download)", tool.name()),
            }
        }

        // Rocky ISO
        match rocky::find_existing(self) {
            Some(iso) => println!("  Rocky ISO: FOUND at {}", iso.path.display()),
            None => println!("  Rocky ISO: NOT FOUND (will download)"),
        }
    }
}
