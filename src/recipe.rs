//! Recipe binary resolution and execution.
//!
//! Recipe is the general-purpose package manager used by leviso to manage
//! build dependencies like the Rocky Linux ISO.
//!
//! Recipe returns structured JSON to stdout (logs go to stderr), so leviso
//! can parse the ctx to get paths instead of hardcoding them.
//!
//! Resolution order:
//! 1. System PATH (`which recipe`)
//! 2. Monorepo submodule (`../tools/recipe`)
//! 3. `RECIPE_BIN` env var (path to binary)
//! 4. `RECIPE_SRC` env var (path to source, will build)

use anyhow::{bail, Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How the recipe binary was built from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecipeSource {
    /// Built from monorepo submodule.
    Monorepo,
    /// Built from source via RECIPE_SRC.
    EnvSrc,
}

/// Resolved recipe binary.
#[derive(Debug, Clone)]
pub struct RecipeBinary {
    /// Path to the binary.
    pub path: PathBuf,
}

impl RecipeBinary {
    /// Check if the binary exists and is executable.
    pub fn is_valid(&self) -> bool {
        if !self.path.exists() {
            return false;
        }

        match std::fs::metadata(&self.path) {
            Ok(meta) => {
                if !meta.is_file() {
                    return false;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    if mode & 0o111 == 0 {
                        return false;
                    }
                }
                true
            }
            Err(_) => false,
        }
    }

    /// Run a recipe file with this binary.
    pub fn run(&self, recipe_path: &Path, build_dir: &Path) -> Result<()> {
        run_recipe(&self.path, recipe_path, build_dir)
    }
}

/// Find the recipe binary using the resolution order.
///
/// Resolution order:
/// 1. System PATH (`which recipe`)
/// 2. Monorepo submodule (`../tools/recipe`)
/// 3. `RECIPE_BIN` env var (path to binary)
/// 4. `RECIPE_SRC` env var (path to source, will build)
pub fn find_recipe(monorepo_dir: &Path) -> Result<RecipeBinary> {
    // 1. Check system PATH
    if let Ok(path) = which::which("recipe") {
        return Ok(RecipeBinary { path });
    }

    // 2. Check monorepo submodule
    let submodule = monorepo_dir.join("tools/recipe");
    if submodule.join("Cargo.toml").exists() {
        return build_from_source(&submodule, monorepo_dir, RecipeSource::Monorepo);
    }

    // 3. Check RECIPE_BIN env var
    if let Ok(bin_path) = env::var("RECIPE_BIN") {
        let path = PathBuf::from(&bin_path);
        if path.exists() {
            let binary = RecipeBinary { path };
            if binary.is_valid() {
                return Ok(binary);
            }
            bail!(
                "RECIPE_BIN points to invalid binary: {}\n\
                 File exists but is not executable.",
                bin_path
            );
        }
        bail!(
            "RECIPE_BIN points to non-existent path: {}",
            bin_path
        );
    }

    // 4. Check RECIPE_SRC env var
    if let Ok(src_path) = env::var("RECIPE_SRC") {
        let src = PathBuf::from(&src_path);
        if src.join("Cargo.toml").exists() {
            // For RECIPE_SRC, use its parent as potential workspace root
            let workspace_root = src.parent().unwrap_or(&src);
            return build_from_source(&src, workspace_root, RecipeSource::EnvSrc);
        }
        bail!(
            "RECIPE_SRC is not a valid Cargo crate: {}\n\
             Expected Cargo.toml at that path.",
            src_path
        );
    }

    bail!(
        "Could not find recipe binary.\n\n\
         Resolution order tried:\n\
         1. System PATH - not found\n\
         2. Monorepo at {} - not found\n\
         3. RECIPE_BIN env var - not set\n\
         4. RECIPE_SRC env var - not set\n\n\
         Solutions:\n\
         - Install recipe to PATH\n\
         - Set RECIPE_BIN=/path/to/recipe\n\
         - Set RECIPE_SRC=/path/to/recipe/source",
        submodule.display()
    )
}

/// Build recipe from source.
fn build_from_source(crate_path: &Path, monorepo_dir: &Path, source: RecipeSource) -> Result<RecipeBinary> {
    let release_build = env::var("RECIPE_BUILD_RELEASE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    let source_desc = match source {
        RecipeSource::Monorepo => "monorepo",
        RecipeSource::EnvSrc => "RECIPE_SRC",
    };

    println!("  Building recipe ({})...", source_desc);
    println!("    Source: {}", crate_path.display());

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--package")
        .arg("levitate-recipe")
        .current_dir(crate_path);

    if release_build {
        cmd.arg("--release");
        println!("    Profile: release");
    } else {
        println!("    Profile: debug");
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to execute cargo build for recipe"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "cargo build failed for recipe\n  Exit code: {}\n  stderr: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    let profile = if release_build { "release" } else { "debug" };

    // In a workspace, binary goes to workspace root's target directory
    let binary = monorepo_dir.join("target").join(profile).join("recipe");

    if !binary.exists() {
        // Fallback: check crate's local target (non-workspace case)
        let local_binary = crate_path.join("target").join(profile).join("recipe");
        if local_binary.exists() {
            println!("    Built: {}", local_binary.display());
            return Ok(RecipeBinary { path: local_binary });
        }
        bail!(
            "Built binary not found at:\n  - {}\n  - {}",
            binary.display(),
            local_binary.display()
        );
    }

    println!("    Built: {}", binary.display());

    Ok(RecipeBinary { path: binary })
}

/// Run a recipe using the recipe binary, returning the ctx as JSON.
///
/// Recipe outputs:
/// - stderr: Progress/logs (inherited, shown to user)
/// - stdout: JSON ctx (parsed and returned)
pub fn run_recipe_json(recipe_bin: &Path, recipe_path: &Path, build_dir: &Path) -> Result<serde_json::Value> {
    eprintln!("  Running recipe: {}", recipe_path.display());
    eprintln!("    Build dir: {}", build_dir.display());

    let output = Command::new(recipe_bin)
        .arg("install")
        .arg(recipe_path)
        .arg("--build-dir")
        .arg(build_dir)
        .stderr(Stdio::inherit())  // Show progress to user
        .output()
        .with_context(|| format!("Failed to execute recipe: {}", recipe_bin.display()))?;

    if !output.status.success() {
        bail!(
            "Recipe failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );
    }

    let ctx: serde_json::Value = serde_json::from_slice(&output.stdout)
        .with_context(|| "Failed to parse recipe JSON output")?;

    Ok(ctx)
}

/// Run a recipe using the recipe binary (legacy, no JSON parsing).
pub fn run_recipe(recipe_bin: &Path, recipe_path: &Path, build_dir: &Path) -> Result<()> {
    run_recipe_json(recipe_bin, recipe_path, build_dir)?;
    Ok(())
}

// ============================================================================
// Rocky Linux dependency via recipe
// ============================================================================

/// Paths produced by the rocky.rhai recipe after execution.
#[derive(Debug, Clone)]
pub struct RockyPaths {
    /// Path to the downloaded ISO.
    pub iso: PathBuf,
    /// Path to the extracted rootfs.
    pub rootfs: PathBuf,
    /// Path to the extracted ISO contents (packages).
    pub iso_contents: PathBuf,
}

impl RockyPaths {
    /// Check if all paths exist.
    pub fn exists(&self) -> bool {
        self.iso.exists() && self.rootfs.exists() && self.iso_contents.exists()
    }
}

/// Run the rocky.rhai recipe and return the output paths.
///
/// This is the entry point for leviso to use recipe for Rocky dependency.
/// The recipe returns a ctx with paths, so we don't need to hardcode them.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
///
/// # Returns
/// The paths to the Rocky artifacts (ISO, rootfs, iso-contents).
pub fn rocky(base_dir: &Path) -> Result<RockyPaths> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/rocky.rhai");

    if !recipe_path.exists() {
        bail!(
            "Rocky recipe not found at: {}\n\
             Expected rocky.rhai in leviso/deps/",
            recipe_path.display()
        );
    }

    // Find and run recipe, parse JSON output
    let recipe_bin = find_recipe(&monorepo_dir)?;
    let ctx = run_recipe_json(&recipe_bin.path, &recipe_path, &downloads_dir)?;

    // Extract paths from ctx (recipe sets these)
    let iso = ctx["iso_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("Rocky-10.1-x86_64-dvd1.iso"));

    let rootfs = ctx["rootfs_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("rootfs"));

    let iso_contents = ctx["iso_contents_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("iso-contents"));

    let paths = RockyPaths {
        iso,
        rootfs,
        iso_contents,
    };

    if !paths.exists() {
        bail!(
            "Recipe completed but expected paths are missing:\n\
             - ISO: {} ({})\n\
             - rootfs: {} ({})\n\
             - iso-contents: {} ({})",
            paths.iso.display(),
            if paths.iso.exists() { "OK" } else { "MISSING" },
            paths.rootfs.display(),
            if paths.rootfs.exists() { "OK" } else { "MISSING" },
            paths.iso_contents.display(),
            if paths.iso_contents.exists() { "OK" } else { "MISSING" },
        );
    }

    Ok(paths)
}

// ============================================================================
// Installation tools via recipes (recstrap, recfstab, recchroot)
// ============================================================================

/// Run the tool recipes to install recstrap, recfstab, recchroot to staging.
///
/// These tools are required for the live ISO to be able to install itself.
/// The recipes install binaries to output/staging/usr/bin/.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn install_tools(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let staging_bin = base_dir.join("output/staging/usr/bin");

    // Find recipe binary once
    let recipe_bin = find_recipe(&monorepo_dir)?;

    // Run each tool recipe
    for tool in ["recstrap", "recfstab", "recchroot"] {
        let recipe_path = base_dir.join(format!("deps/{}.rhai", tool));
        let installed_path = staging_bin.join(tool);

        // Skip if already installed
        if installed_path.exists() {
            println!("  {} already installed", tool);
            continue;
        }

        if !recipe_path.exists() {
            bail!(
                "{} recipe not found at: {}\n\
                 Expected {}.rhai in leviso/deps/",
                tool,
                recipe_path.display(),
                tool
            );
        }

        recipe_bin.run(&recipe_path, &downloads_dir)?;

        // Verify installation
        if !installed_path.exists() {
            bail!(
                "Recipe completed but {} not found at: {}",
                tool,
                installed_path.display()
            );
        }
    }

    Ok(())
}

// ============================================================================
// Supplementary packages via recipe
// ============================================================================

/// Run the packages.rhai recipe to extract supplementary RPMs into rootfs.
///
/// This must be called after `rocky()` since it depends on the rootfs and
/// iso-contents directories created by rocky.rhai.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn packages(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/packages.rhai");

    if !recipe_path.exists() {
        bail!(
            "Packages recipe not found at: {}\n\
             Expected packages.rhai in leviso/deps/",
            recipe_path.display()
        );
    }

    // Verify rocky.rhai has been run first
    let rootfs = downloads_dir.join("rootfs");
    let iso_contents = downloads_dir.join("iso-contents");

    if !rootfs.join("usr").exists() {
        bail!(
            "rootfs not found at: {}\n\
             Run rocky.rhai first (via rocky() function).",
            rootfs.display()
        );
    }

    if !iso_contents.join("BaseOS/Packages").exists() {
        bail!(
            "iso-contents not found at: {}\n\
             Run rocky.rhai first (via rocky() function).",
            iso_contents.display()
        );
    }

    // Find and run recipe
    let recipe_bin = find_recipe(&monorepo_dir)?;
    recipe_bin.run(&recipe_path, &downloads_dir)?;

    Ok(())
}

/// Run the epel.rhai recipe to download and extract EPEL packages into rootfs.
///
/// This must be called after `packages()` since it depends on the rootfs.
/// Downloads packages not available in Rocky 10 DVD: btrfs-progs, ntfs-3g,
/// screen, pv, ddrescue, testdisk.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn epel(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/epel.rhai");

    if !recipe_path.exists() {
        bail!(
            "EPEL recipe not found at: {}\n\
             Expected epel.rhai in leviso/deps/",
            recipe_path.display()
        );
    }

    // Verify rootfs exists
    let rootfs = downloads_dir.join("rootfs");
    if !rootfs.join("usr").exists() {
        bail!(
            "rootfs not found at: {}\n\
             Run rocky.rhai and packages.rhai first.",
            rootfs.display()
        );
    }

    // Find and run recipe
    let recipe_bin = find_recipe(&monorepo_dir)?;
    recipe_bin.run(&recipe_path, &downloads_dir)?;

    Ok(())
}

// ============================================================================
// Linux kernel via recipe
// ============================================================================

/// Paths produced by the linux.rhai recipe after execution.
#[derive(Debug, Clone)]
pub struct LinuxPaths {
    /// Path to the kernel source tree.
    pub source: PathBuf,
    /// Path to vmlinuz in staging.
    pub vmlinuz: PathBuf,
    /// Kernel version string.
    pub version: String,
}

impl LinuxPaths {
    /// Check if kernel is built and installed.
    pub fn is_installed(&self) -> bool {
        self.vmlinuz.exists()
    }
}

/// Run the linux.rhai recipe and return the output paths.
///
/// This handles the full kernel workflow: acquire source, build, install to staging.
/// The recipe returns a ctx with all paths and the kernel version.
///
/// # Arguments
/// * `base_dir` - leviso crate root (e.g., `/path/to/leviso`)
pub fn linux(base_dir: &Path) -> Result<LinuxPaths> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/linux.rhai");

    if !recipe_path.exists() {
        bail!(
            "Linux recipe not found at: {}\n\
             Expected linux.rhai in leviso/deps/",
            recipe_path.display()
        );
    }

    // Find and run recipe, parse JSON output
    let recipe_bin = find_recipe(&monorepo_dir)?;
    let ctx = run_recipe_json(&recipe_bin.path, &recipe_path, &downloads_dir)?;

    // Extract paths from ctx (recipe sets these)
    let output_dir = base_dir.join("output");

    let source = ctx["source_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // Fallback: check submodule first, then downloads
            let submodule = monorepo_dir.join("linux");
            if submodule.join("Makefile").exists() {
                submodule
            } else {
                downloads_dir.join("linux")
            }
        });

    let version = ctx["kernel_version"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let vmlinuz = output_dir.join("staging/boot/vmlinuz");

    Ok(LinuxPaths {
        source,
        vmlinuz,
        version,
    })
}

/// Check if Linux source is available (without running the full recipe).
pub fn has_linux_source(base_dir: &Path) -> bool {
    let monorepo_dir = base_dir.parent().unwrap_or(base_dir);

    // Check submodule
    if monorepo_dir.join("linux/Makefile").exists() {
        return true;
    }

    // Check downloads
    if base_dir.join("downloads/linux/Makefile").exists() {
        return true;
    }

    false
}

/// Clear the recipe cache directory (~/.cache/levitate/).
pub fn clear_cache() -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("levitate");

    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir)?;
        std::fs::create_dir_all(&cache_dir)?;
    }
    Ok(())
}

