//! Build command - builds LevitateOS artifacts.

use anyhow::Result;
use std::path::Path;
use std::time::Instant;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, INITRAMFS_INSTALLED_OUTPUT, ISO_FILENAME, SQUASHFS_NAME};

use crate::artifact;
use crate::config::Config;
use crate::rebuild;
use crate::timing::Timer;
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
    let build_start = Instant::now();

    // 1. Ensure Rocky is available via recipe
    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
        println!("Resolving Rocky Linux via recipe...");
        crate::recipe::rocky(base_dir)?;
    }

    // 2. Resolve Linux source (auto-detects submodule or downloads)
    let linux = resolver.linux()?;

    // 3. Build kernel (compile + install, skip what's already done)
    let needs_compile = rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);

    if needs_compile {
        println!("\nBuilding kernel...");
        let t = Timer::start("Kernel");
        build_kernel(base_dir, &linux.path, config, false)?;
        rebuild::cache_kernel_hash(base_dir);
        t.finish();
    } else if needs_install {
        // bzImage exists but vmlinuz doesn't - just install
        println!("\nInstalling kernel (compile skipped)...");
        let t = Timer::start("Kernel install");
        install_kernel_only(base_dir, &linux.path)?;
        t.finish();
    } else {
        println!("\n[SKIP] Kernel already built and installed");
    }

    // 4. Build squashfs (skip if inputs unchanged)
    if rebuild::squashfs_needs_rebuild(base_dir) {
        println!("\nBuilding squashfs system image...");
        let t = Timer::start("Squashfs");
        artifact::build_squashfs(base_dir)?;
        rebuild::cache_squashfs_hash(base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Squashfs already built (inputs unchanged)");
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
        artifact::create_squashfs_iso(base_dir)?;
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
    println!("  Squashfs: output/{}", SQUASHFS_NAME);
    println!("\nNext: leviso run");

    Ok(())
}

/// Verify hardware compatibility against all profiles.
fn verify_hardware_compat(base_dir: &Path) -> Result<()> {
    println!("\n=== Hardware Compatibility Verification ===");

    let output_dir = base_dir.join("output");
    // Firmware is installed to squashfs-root during squashfs build, not staging
    let checker = hardware_compat::HardwareCompatChecker::new(
        output_dir.join("kernel-build/.config"),
        output_dir.join("squashfs-root/usr/lib/firmware"),
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
    resolver: &DependencyResolver,
    config: &Config,
    clean: bool,
) -> Result<()> {
    let linux = resolver.linux()?;
    let needs_compile = clean || rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);

    if needs_compile {
        build_kernel(base_dir, &linux.path, config, clean)?;
        rebuild::cache_kernel_hash(base_dir);
    } else if needs_install {
        println!("Installing kernel (compile skipped)...");
        install_kernel_only(base_dir, &linux.path)?;
    } else {
        println!("[SKIP] Kernel already built and installed");
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
    let squashfs_path = base_dir.join("output").join(SQUASHFS_NAME);
    let initramfs_path = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);

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

/// Install kernel only (when bzImage exists but vmlinuz doesn't).
fn install_kernel_only(base_dir: &Path, linux_source: &Path) -> Result<()> {
    use crate::build;

    let output_dir = base_dir.join("output");

    let version = build::kernel::install_kernel(linux_source, &output_dir, &output_dir.join("staging"))?;

    println!("\n=== Kernel install complete ===");
    println!("  Version: {}", version);
    println!("  Kernel:  output/staging/boot/vmlinuz");
    println!("  Modules: output/staging/usr/lib/modules/{}/", version);

    Ok(())
}
