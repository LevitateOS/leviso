//! Build command - builds LevitateOS artifacts.

use anyhow::Result;
use std::path::Path;
use std::time::Instant;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, INITRAMFS_INSTALLED_OUTPUT, ISO_FILENAME, ROOTFS_NAME};

use crate::artifact;
use crate::config::Config;
use crate::rebuild;
use crate::recipe;
use crate::timing::Timer;

/// Build target for the build command.
pub enum BuildTarget {
    /// Full build (all artifacts)
    Full,
    /// Kernel only
    Kernel { clean: bool },
    /// Rootfs (EROFS) only
    Rootfs,
    /// Initramfs only
    Initramfs,
    /// ISO only
    Iso,
}

/// Execute the build command.
pub fn cmd_build(
    base_dir: &Path,
    target: BuildTarget,
    config: &Config,
) -> Result<()> {
    match target {
        BuildTarget::Full => build_full(base_dir, config),
        BuildTarget::Kernel { clean } => build_kernel_only(base_dir, config, clean),
        BuildTarget::Rootfs => build_rootfs_only(base_dir),
        BuildTarget::Initramfs => build_initramfs_only(base_dir),
        BuildTarget::Iso => build_iso_only(base_dir),
    }
}

/// Full build: rootfs (EROFS) + tiny initramfs + ISO.
/// Skips anything already built, rebuilds only on changes.
fn build_full(base_dir: &Path, _config: &Config) -> Result<()> {
    println!("=== Full LevitateOS Build ===\n");
    let build_start = Instant::now();

    // 1. Ensure Rocky is available via recipe
    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
        println!("Resolving Rocky Linux via recipe...");
        recipe::rocky(base_dir)?;
    }

    // 1b. Extract supplementary RPMs into rootfs
    // (This is separate from rocky.rhai so changing the package list doesn't re-extract the 2GB install.img)
    println!("\nExtracting supplementary packages...");
    recipe::packages(base_dir)?;

    // 2. Build kernel via recipe (acquire + build + install)
    // The recipe has is_acquired/is_built/is_installed checks for incremental builds
    let needs_compile = rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);

    if needs_compile || needs_install {
        println!("\nBuilding kernel via recipe...");
        let t = Timer::start("Kernel");
        let linux = recipe::linux(base_dir)?;
        rebuild::cache_kernel_hash(base_dir);
        t.finish();
        println!("  Kernel {} installed", linux.version);
    } else {
        println!("\n[SKIP] Kernel already built and installed");
    }

    // 4. Build rootfs (EROFS) - skip if inputs unchanged
    if rebuild::rootfs_needs_rebuild(base_dir) {
        println!("\nBuilding EROFS rootfs image...");
        let t = Timer::start("Rootfs");
        artifact::build_rootfs(base_dir)?;
        rebuild::cache_rootfs_hash(base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Rootfs already built (inputs unchanged)");
    }

    // 5. Build tiny initramfs (skip if inputs unchanged)
    if rebuild::initramfs_needs_rebuild(base_dir) {
        println!("\nBuilding tiny initramfs...");
        let t = Timer::start("Initramfs");
        artifact::build_tiny_initramfs(base_dir)?;
        rebuild::cache_initramfs_hash(base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 5b. Build install initramfs (REQUIRED for installation)
    // This is copied to installed systems instead of running dracut (saves 2-3 min)
    // The initramfs is generic (no hostonly) so it works on any hardware
    let install_initramfs = base_dir.join("output").join(INITRAMFS_INSTALLED_OUTPUT);
    if !install_initramfs.exists() {
        println!("\nBuilding install initramfs...");
        let t = Timer::start("Install Initramfs");
        artifact::build_install_initramfs(base_dir)?;
        t.finish();
    } else {
        println!("\n[SKIP] Install initramfs already built");
    }

    // 6. Build ISO (skip if components unchanged)
    if rebuild::iso_needs_rebuild(base_dir) {
        println!("\nBuilding ISO...");
        let t = Timer::start("ISO");
        artifact::create_iso(base_dir)?;
        t.finish();
    } else {
        println!("\n[SKIP] ISO already built (components unchanged)");
    }

    // 7. Verify hardware compatibility
    verify_hardware_compat(base_dir)?;

    let total = build_start.elapsed().as_secs_f64();
    if total >= 60.0 {
        println!("\n=== Build Complete ({:.1}m) ===", total / 60.0);
    } else {
        println!("\n=== Build Complete ({:.1}s) ===", total);
    }
    println!("  ISO: output/{}", ISO_FILENAME);
    println!("  Rootfs: output/{}", ROOTFS_NAME);
    println!("\nNext: leviso run");

    Ok(())
}

/// Verify hardware compatibility against all profiles.
fn verify_hardware_compat(base_dir: &Path) -> Result<()> {
    println!("\n=== Hardware Compatibility Verification ===");

    let output_dir = base_dir.join("output");
    // Firmware is installed to rootfs-staging during rootfs build, not staging
    let checker = hardware_compat::HardwareCompatChecker::new(
        output_dir.join("kernel-build/.config"),
        output_dir.join("rootfs-staging/usr/lib/firmware"),
    );

    let all_profiles = hardware_compat::profiles::get_all_profiles();
    let mut failures = 0;

    for p in all_profiles {
        match checker.verify_profile(p.as_ref()) {
            Ok(report) => {
                report.print_summary();
                if report.has_critical_failures() {
                    failures += 1;
                }
            }
            Err(e) => {
                println!("  [ERROR] Failed to verify profile '{}': {}", p.name(), e);
                failures += 1;
            }
        }
    }

    if failures > 0 {
        println!("\n[FAIL] Hardware compatibility verification failed for {} profile(s).", failures);
        // We don't bail yet, just warn, unless it's critical for the DISTRO itself.
        // For now, let's just print the results.
    } else {
        println!("\n[PASS] All hardware compatibility profiles passed (or have only non-critical warnings).");
    }

    Ok(())
}

/// Build kernel only.
fn build_kernel_only(
    base_dir: &Path,
    _config: &Config,
    clean: bool,
) -> Result<()> {
    let needs_compile = clean || rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);

    if clean {
        // Clean kernel build directory before recipe runs
        let kernel_build = base_dir.join("output/kernel-build");
        if kernel_build.exists() {
            println!("Cleaning kernel build directory...");
            std::fs::remove_dir_all(&kernel_build)?;
        }
    }

    if needs_compile || needs_install || clean {
        println!("Building kernel via recipe...");
        let linux = recipe::linux(base_dir)?;
        rebuild::cache_kernel_hash(base_dir);
        println!("\n=== Kernel build complete ===");
        println!("  Version: {}", linux.version);
        println!("  Kernel:  output/staging/boot/vmlinuz");
    } else {
        println!("[SKIP] Kernel already built and installed");
        println!("  Use 'build kernel --clean' to force rebuild");
    }
    Ok(())
}

/// Build rootfs (EROFS) only.
fn build_rootfs_only(base_dir: &Path) -> Result<()> {
    if rebuild::rootfs_needs_rebuild(base_dir) {
        artifact::build_rootfs(base_dir)?;
        rebuild::cache_rootfs_hash(base_dir);
    } else {
        println!("[SKIP] Rootfs already built (inputs unchanged)");
        println!("  Use 'clean rootfs' then rebuild to force");
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
    let rootfs_path = base_dir.join("output").join(ROOTFS_NAME);
    let initramfs_path = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);

    if !rootfs_path.exists() {
        println!("Rootfs not found, building...");
        artifact::build_rootfs(base_dir)?;
    }
    if !initramfs_path.exists() {
        println!("Tiny initramfs not found, building...");
        artifact::build_tiny_initramfs(base_dir)?;
    }
    artifact::create_iso(base_dir)?;
    Ok(())
}

