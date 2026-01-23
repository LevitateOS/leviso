//! Build command - builds LevitateOS artifacts.

use anyhow::Result;
use std::path::Path;

use crate::artifact;
use crate::config::Config;
use crate::extract;
use crate::rebuild;
use leviso_deps::DependencyResolver;

/// Build target for the build command.
pub enum BuildTarget {
    /// Full build (all artifacts)
    Full,
    /// Kernel only
    Kernel { clean: bool },
    /// Squashfs only
    Squashfs,
    /// Initramfs only
    Initramfs,
    /// ISO only
    Iso,
}

/// Execute the build command.
pub fn cmd_build(
    base_dir: &Path,
    target: BuildTarget,
    resolver: &DependencyResolver,
    config: &Config,
) -> Result<()> {
    match target {
        BuildTarget::Full => build_full(base_dir, resolver, config),
        BuildTarget::Kernel { clean } => build_kernel_only(base_dir, resolver, config, clean),
        BuildTarget::Squashfs => build_squashfs_only(base_dir),
        BuildTarget::Initramfs => build_initramfs_only(base_dir),
        BuildTarget::Iso => build_iso_only(base_dir),
    }
}

/// Full build: squashfs + tiny initramfs + ISO.
/// Skips anything already built, rebuilds only on changes.
fn build_full(base_dir: &Path, resolver: &DependencyResolver, config: &Config) -> Result<()> {
    println!("=== Full LevitateOS Build ===\n");

    // 1. Download Rocky if needed
    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
        println!("Resolving Rocky Linux ISO...");
        let rocky = resolver.rocky_iso()?;
        if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
            extract::extract_rocky_iso(base_dir, &rocky.path)?;
        }
    }

    // 2. Resolve Linux source (auto-detects submodule or downloads)
    let linux = resolver.linux()?;

    // 3. Build kernel (skip if built and kconfig unchanged)
    if rebuild::kernel_needs_rebuild(base_dir) {
        println!("\nBuilding kernel...");
        build_kernel(base_dir, &linux.path, config, false)?;
        rebuild::cache_kconfig_hash(base_dir);
    } else {
        println!("\n[SKIP] Kernel already built (kconfig unchanged)");
    }

    // 4. Build squashfs (skip if inputs unchanged)
    if rebuild::squashfs_needs_rebuild(base_dir) {
        println!("\nBuilding squashfs system image...");
        artifact::build_squashfs(base_dir)?;
        rebuild::cache_squashfs_hash(base_dir);
    } else {
        println!("\n[SKIP] Squashfs already built (inputs unchanged)");
    }

    // 5. Build tiny initramfs (skip if inputs unchanged)
    if rebuild::initramfs_needs_rebuild(base_dir) {
        println!("\nBuilding tiny initramfs...");
        artifact::build_tiny_initramfs(base_dir)?;
        rebuild::cache_initramfs_hash(base_dir);
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 6. Build ISO (skip if components unchanged)
    if rebuild::iso_needs_rebuild(base_dir) {
        println!("\nBuilding ISO...");
        artifact::create_squashfs_iso(base_dir)?;
    } else {
        println!("\n[SKIP] ISO already built (components unchanged)");
    }

    println!("\n=== Build Complete ===");
    println!("  ISO: output/levitateos.iso");
    println!("  Squashfs: output/filesystem.squashfs");
    println!("\nNext: leviso run");

    Ok(())
}

/// Build kernel only.
fn build_kernel_only(
    base_dir: &Path,
    resolver: &DependencyResolver,
    config: &Config,
    clean: bool,
) -> Result<()> {
    let linux = resolver.linux()?;
    if clean || rebuild::kernel_needs_rebuild(base_dir) {
        build_kernel(base_dir, &linux.path, config, clean)?;
        rebuild::cache_kconfig_hash(base_dir);
    } else {
        println!("[SKIP] Kernel already built (kconfig unchanged)");
        println!("  Use 'build kernel --clean' to force rebuild");
    }
    Ok(())
}

/// Build squashfs only.
fn build_squashfs_only(base_dir: &Path) -> Result<()> {
    if rebuild::squashfs_needs_rebuild(base_dir) {
        artifact::build_squashfs(base_dir)?;
        rebuild::cache_squashfs_hash(base_dir);
    } else {
        println!("[SKIP] Squashfs already built (inputs unchanged)");
        println!("  Use 'clean squashfs' then rebuild to force");
    }
    Ok(())
}

/// Build initramfs only.
fn build_initramfs_only(base_dir: &Path) -> Result<()> {
    if rebuild::initramfs_needs_rebuild(base_dir) {
        artifact::build_tiny_initramfs(base_dir)?;
        rebuild::cache_initramfs_hash(base_dir);
    } else {
        println!("[SKIP] Initramfs already built (inputs unchanged)");
        println!("  Use 'clean iso' then rebuild to force");
    }
    Ok(())
}

/// Build ISO only.
fn build_iso_only(base_dir: &Path) -> Result<()> {
    let squashfs_path = base_dir.join("output/filesystem.squashfs");
    let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");

    if !squashfs_path.exists() {
        println!("Squashfs not found, building...");
        artifact::build_squashfs(base_dir)?;
    }
    if !initramfs_path.exists() {
        println!("Tiny initramfs not found, building...");
        artifact::build_tiny_initramfs(base_dir)?;
    }
    artifact::create_squashfs_iso(base_dir)?;
    Ok(())
}

/// Build the kernel.
fn build_kernel(
    base_dir: &Path,
    linux_source: &Path,
    _config: &Config,
    clean: bool,
) -> Result<()> {
    use crate::build;

    let output_dir = base_dir.join("output");

    if clean {
        let kernel_build = output_dir.join("kernel-build");
        if kernel_build.exists() {
            println!("Cleaning kernel build directory...");
            std::fs::remove_dir_all(&kernel_build)?;
        }
    }

    let version = build::kernel::build_kernel(linux_source, &output_dir, base_dir)?;

    build::kernel::install_kernel(linux_source, &output_dir, &output_dir.join("staging"))?;

    println!("\n=== Kernel build complete ===");
    println!("  Version: {}", version);
    println!("  Kernel:  output/staging/boot/vmlinuz");
    println!("  Modules: output/staging/usr/lib/modules/{}/", version);

    Ok(())
}
