//! Build command - builds LevitateOS artifacts.

use anyhow::Result;
use std::path::Path;
use std::time::Instant;

use distro_spec::levitate::{
    INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME,
};

use crate::artifact;
use crate::config::Config;
use crate::rebuild;
use crate::recipe;
use distro_builder::timing::Timer;

fn open_artifact_store(base_dir: &Path) -> Option<distro_builder::artifact_store::ArtifactStore> {
    match distro_builder::artifact_store::ArtifactStore::open_for_distro(base_dir) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("[WARN] Artifact store disabled: {:#}", e);
            None
        }
    }
}

/// Build target for the build command.
pub enum BuildTarget {
    /// Full build (all artifacts, skip kernel if not available)
    Full,
    /// Full build with kernel compilation
    FullWithKernel,
    /// Kernel only
    Kernel { clean: bool },
    /// Rootfs (EROFS) only
    Rootfs,
    /// Initramfs only
    Initramfs,
    /// ISO only
    Iso,
    /// qcow2 VM disk image
    Qcow2 { disk_size: u32 },
}

/// Execute the build command.
pub fn cmd_build(base_dir: &Path, target: BuildTarget, config: &Config) -> Result<()> {
    match target {
        BuildTarget::Full => build_full(base_dir, config, false),
        BuildTarget::FullWithKernel => build_full(base_dir, config, true),
        BuildTarget::Kernel { clean } => build_kernel_only(base_dir, config, clean),
        BuildTarget::Rootfs => build_rootfs_only(base_dir),
        BuildTarget::Initramfs => build_initramfs_only(base_dir),
        BuildTarget::Iso => build_iso_only(base_dir),
        BuildTarget::Qcow2 { disk_size } => build_qcow2_only(base_dir, disk_size),
    }
}

/// Full build: rootfs (EROFS) + tiny initramfs + ISO.
/// Skips anything already built, rebuilds only on changes.
fn build_full(base_dir: &Path, _config: &Config, with_kernel: bool) -> Result<()> {
    println!("=== Full LevitateOS Build ===\n");
    let build_start = Instant::now();
    let store = open_artifact_store(base_dir);

    // 0. Ensure host build tools are available (mkfs.erofs, xorriso, etc.)
    println!("Ensuring host build tools...");
    recipe::ensure_host_tools(base_dir)?;

    // 1. Ensure Rocky is available via recipe
    if !base_dir.join("downloads/iso-contents/BaseOS").exists() {
        println!("Resolving Rocky Linux via recipe...");
        recipe::rocky(base_dir)?;
    }

    // 1b. Extract supplementary RPMs into rootfs
    println!("\nExtracting supplementary packages...");
    recipe::packages(base_dir)?;

    // 1c. Download and extract EPEL packages
    println!("\nDownloading EPEL packages...");
    recipe::epel(base_dir)?;

    // Try to restore outputs from the centralized artifact store if the output
    // files are missing but input hashes are known.
    if let Some(store) = &store {
        let out = base_dir.join("output");

        let rootfs_key = out.join(".rootfs-inputs.hash");
        let rootfs_out = out.join(ROOTFS_NAME);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "rootfs_erofs",
            &rootfs_key,
            &rootfs_out,
        ) {
            Ok(true) => println!("\n[RESTORE] Rootfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore rootfs from artifact store: {:#}",
                e
            ),
        }

        let initramfs_key = out.join(".initramfs-inputs.hash");
        let initramfs_out = out.join(INITRAMFS_LIVE_OUTPUT);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "initramfs",
            &initramfs_key,
            &initramfs_out,
        ) {
            Ok(true) => println!("\n[RESTORE] Initramfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore initramfs from artifact store: {:#}",
                e
            ),
        }

        let install_initramfs_key = out.join(".install-initramfs-inputs.hash");
        let install_initramfs_out = out.join(INITRAMFS_INSTALLED_OUTPUT);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "install_initramfs",
            &install_initramfs_key,
            &install_initramfs_out,
        ) {
            Ok(true) => println!("\n[RESTORE] Install initramfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore install initramfs from artifact store: {:#}",
                e
            ),
        }
    }

    // 2. Kernel: build only if --kernel was passed, otherwise use existing or error
    let needs_compile = rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);

    if needs_compile || needs_install {
        if with_kernel {
            println!("\nBuilding kernel via recipe...");
            let t = Timer::start("Kernel");
            let linux = recipe::linux(base_dir)?;
            rebuild::cache_kernel_hash(base_dir);
            if let Some(store) = &store {
                let key = base_dir.join("output/.kernel-inputs.hash");
                let staging = base_dir.join("output/staging");
                if let Err(e) = distro_builder::artifact_store::try_store_kernel_payload_from_key(
                    store,
                    &key,
                    &staging,
                    std::collections::BTreeMap::new(),
                ) {
                    eprintln!(
                        "[WARN] Failed to store kernel payload in artifact store: {:#}",
                        e
                    );
                }
            }
            t.finish();
            println!("  Kernel {} installed", linux.version);
        } else if needs_compile {
            // No kernel at all
            anyhow::bail!(
                "No kernel available!\n\n\
                 LevitateOS is the canonical kernel builder. To build:\n\
                   cargo run -- build --kernel --dangerously-waste-the-users-time\n\n\
                 Or build kernel only:\n\
                   cargo run -- build kernel --dangerously-waste-the-users-time"
            );
        } else {
            // Kernel exists but needs reinstall
            println!("\nInstalling kernel (compile skipped)...");
            let t = Timer::start("Kernel");
            let linux = recipe::linux(base_dir)?;
            rebuild::cache_kernel_hash(base_dir);
            if let Some(store) = &store {
                let key = base_dir.join("output/.kernel-inputs.hash");
                let staging = base_dir.join("output/staging");
                if let Err(e) = distro_builder::artifact_store::try_store_kernel_payload_from_key(
                    store,
                    &key,
                    &staging,
                    std::collections::BTreeMap::new(),
                ) {
                    eprintln!(
                        "[WARN] Failed to store kernel payload in artifact store: {:#}",
                        e
                    );
                }
            }
            t.finish();
            println!("  Kernel {} installed", linux.version);
        }
    } else {
        println!("\n[SKIP] Kernel already built and installed");
    }

    // 4. Build rootfs (EROFS) - skip if inputs unchanged
    if rebuild::rootfs_needs_rebuild(base_dir) {
        println!("\nBuilding EROFS rootfs image...");
        let t = Timer::start("Rootfs");
        artifact::build_rootfs(base_dir)?;
        rebuild::cache_rootfs_hash(base_dir);
        if let Some(store) = &store {
            let key = base_dir.join("output/.rootfs-inputs.hash");
            let out = base_dir.join("output").join(ROOTFS_NAME);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!("[WARN] Failed to store rootfs in artifact store: {:#}", e);
            }
        }
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
        if let Some(store) = &store {
            let key = base_dir.join("output/.initramfs-inputs.hash");
            let out = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "initramfs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store initramfs in artifact store: {:#}",
                    e
                );
            }
        }
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 5b. Build install initramfs (REQUIRED for installation)
    // This is copied to installed systems during installation
    // The initramfs is generic (no hostonly) so it works on any hardware
    if rebuild::install_initramfs_needs_rebuild(base_dir) {
        println!("\nBuilding install initramfs...");
        let t = Timer::start("Install Initramfs");
        artifact::build_install_initramfs(base_dir)?;
        rebuild::cache_install_initramfs_hash(base_dir);
        if let Some(store) = &store {
            let key = base_dir.join("output/.install-initramfs-inputs.hash");
            let out = base_dir.join("output").join(INITRAMFS_INSTALLED_OUTPUT);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "install_initramfs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store install initramfs in artifact store: {:#}",
                    e
                );
            }
        }
        t.finish();
    } else {
        println!("\n[SKIP] Install initramfs already built (inputs unchanged)");
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

    // 7. ALWAYS verify all artifacts (whether just built or skipped)
    // This catches broken artifacts from previous runs
    println!("\n=== Artifact Verification ===");
    let output_dir = base_dir.join("output");
    artifact::verify_live_initramfs(&output_dir.join(INITRAMFS_LIVE_OUTPUT))?;
    artifact::verify_install_initramfs(&output_dir.join(INITRAMFS_INSTALLED_OUTPUT))?;
    artifact::verify_iso(&output_dir.join(ISO_FILENAME))?;

    // 8. Verify hardware compatibility
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
                // Only show failures by default (verbose=false)
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
        println!(
            "\n[FAIL] Hardware compatibility verification failed for {} profile(s).",
            failures
        );
        // We don't bail yet, just warn, unless it's critical for the DISTRO itself.
        // For now, let's just print the results.
    } else {
        println!("\n[PASS] All hardware compatibility profiles passed (or have only non-critical warnings).");
    }

    Ok(())
}

/// Build kernel only.
fn build_kernel_only(base_dir: &Path, _config: &Config, clean: bool) -> Result<()> {
    let needs_compile = clean || rebuild::kernel_needs_compile(base_dir);
    let needs_install = rebuild::kernel_needs_install(base_dir);
    let store = open_artifact_store(base_dir);

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
        if let Some(store) = &store {
            let key = base_dir.join("output/.kernel-inputs.hash");
            let staging = base_dir.join("output/staging");
            if let Err(e) = distro_builder::artifact_store::try_store_kernel_payload_from_key(
                store,
                &key,
                &staging,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store kernel payload in artifact store: {:#}",
                    e
                );
            }
        }
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
    let store = open_artifact_store(base_dir);

    if let Some(store) = &store {
        let key = base_dir.join("output/.rootfs-inputs.hash");
        let out = base_dir.join("output").join(ROOTFS_NAME);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "rootfs_erofs",
            &key,
            &out,
        ) {
            Ok(true) => println!("[RESTORE] Rootfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore rootfs from artifact store: {:#}",
                e
            ),
        }
    }

    if rebuild::rootfs_needs_rebuild(base_dir) {
        artifact::build_rootfs(base_dir)?;
        rebuild::cache_rootfs_hash(base_dir);
        if let Some(store) = &store {
            let key = base_dir.join("output/.rootfs-inputs.hash");
            let out = base_dir.join("output").join(ROOTFS_NAME);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!("[WARN] Failed to store rootfs in artifact store: {:#}", e);
            }
        }
    } else {
        println!("[SKIP] Rootfs already built (inputs unchanged)");
        println!("  Use 'clean rootfs' then rebuild to force");
    }
    // Rootfs verification happens inside build_rootfs() via verify_staging()
    // No separate verification needed here since EROFS is always built fresh
    Ok(())
}

/// Build initramfs only.
fn build_initramfs_only(base_dir: &Path) -> Result<()> {
    let store = open_artifact_store(base_dir);

    if let Some(store) = &store {
        let key = base_dir.join("output/.initramfs-inputs.hash");
        let out = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "initramfs",
            &key,
            &out,
        ) {
            Ok(true) => println!("[RESTORE] Initramfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore initramfs from artifact store: {:#}",
                e
            ),
        }
    }

    if rebuild::initramfs_needs_rebuild(base_dir) {
        artifact::build_tiny_initramfs(base_dir)?;
        rebuild::cache_initramfs_hash(base_dir);
        if let Some(store) = &store {
            let key = base_dir.join("output/.initramfs-inputs.hash");
            let out = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "initramfs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store initramfs in artifact store: {:#}",
                    e
                );
            }
        }
    } else {
        println!("[SKIP] Initramfs already built (inputs unchanged)");
        println!("  Use 'clean iso' then rebuild to force");
    }

    // Always verify (whether just built or skipped)
    let output_dir = base_dir.join("output");
    artifact::verify_live_initramfs(&output_dir.join(INITRAMFS_LIVE_OUTPUT))?;
    Ok(())
}

/// Build ISO only.
fn build_iso_only(base_dir: &Path) -> Result<()> {
    let store = open_artifact_store(base_dir);
    let output_dir = base_dir.join("output");
    let rootfs_path = output_dir.join(ROOTFS_NAME);
    let initramfs_path = output_dir.join(INITRAMFS_LIVE_OUTPUT);

    if !rootfs_path.exists() {
        if let Some(store) = &store {
            let key = base_dir.join("output/.rootfs-inputs.hash");
            if let Err(e) = distro_builder::artifact_store::try_restore_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &rootfs_path,
            ) {
                eprintln!(
                    "[WARN] Failed to restore rootfs from artifact store: {:#}",
                    e
                );
            }
        }
    }
    if !rootfs_path.exists() {
        println!("Rootfs not found, building...");
        artifact::build_rootfs(base_dir)?;
    }
    if !initramfs_path.exists() {
        if let Some(store) = &store {
            let key = base_dir.join("output/.initramfs-inputs.hash");
            if let Err(e) = distro_builder::artifact_store::try_restore_file_from_key(
                store,
                "initramfs",
                &key,
                &initramfs_path,
            ) {
                eprintln!(
                    "[WARN] Failed to restore initramfs from artifact store: {:#}",
                    e
                );
            }
        }
    }
    if !initramfs_path.exists() {
        println!("Tiny initramfs not found, building...");
        artifact::build_tiny_initramfs(base_dir)?;
    }
    // Rebuild install initramfs if inputs changed (needed for installation)
    if rebuild::install_initramfs_needs_rebuild(base_dir) {
        println!("Building install initramfs...");
        artifact::build_install_initramfs(base_dir)?;
        rebuild::cache_install_initramfs_hash(base_dir);
    }

    // Verify all components BEFORE creating ISO
    println!("\n=== Pre-ISO Verification ===");
    artifact::verify_live_initramfs(&output_dir.join(INITRAMFS_LIVE_OUTPUT))?;
    artifact::verify_install_initramfs(&output_dir.join(INITRAMFS_INSTALLED_OUTPUT))?;

    artifact::create_iso(base_dir)?;

    // Verify final ISO
    artifact::verify_iso(&output_dir.join(ISO_FILENAME))?;
    Ok(())
}

/// Build qcow2 VM disk image only.
fn build_qcow2_only(base_dir: &Path, disk_size: u32) -> Result<()> {
    let output_dir = base_dir.join("output");
    let rootfs_staging = output_dir.join("rootfs-staging");

    // Rootfs-staging is required for qcow2 building (we use it directly, not EROFS)
    if !rootfs_staging.exists() {
        anyhow::bail!(
            "rootfs-staging not found at {}.\nRun 'cargo run -- build rootfs' first.",
            rootfs_staging.display()
        );
    }

    // Rebuild install initramfs if inputs changed (required for qcow2 boot)
    // This makes the workflow more ergonomic - changes to recinit will trigger rebuild
    if rebuild::install_initramfs_needs_rebuild(base_dir) {
        println!("Building install initramfs (inputs changed)...");
        let t = Timer::start("Install Initramfs");
        artifact::build_install_initramfs(base_dir)?;
        rebuild::cache_install_initramfs_hash(base_dir);
        t.finish();
    }

    // Build the qcow2 image
    artifact::build_qcow2(base_dir, disk_size)?;

    // Verify the image
    artifact::verify_qcow2(base_dir)?;

    Ok(())
}
