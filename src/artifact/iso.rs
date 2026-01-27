//! ISO creation - builds bootable LevitateOS ISO.
//!
//! Creates an ISO with EROFS-based architecture and UKI boot:
//! - UKIs (~50MB each) - kernel + initramfs + cmdline in signed PE binary
//! - EROFS image (~350MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)
//!
//! Delegates to the standalone `reciso` crate for core ISO building.

use anyhow::{bail, Result};
use leviso_cheat_guard::cheat_bail;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use distro_spec::levitate::{
    // Identity
    ISO_LABEL, ISO_FILENAME,
    // Rootfs (EROFS)
    ROOTFS_NAME,
    // Boot files
    INITRAMFS_LIVE_OUTPUT, INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_INSTALLED_ISO_PATH,
    // UKI entries
    UKI_ENTRIES, UKI_INSTALLED_ENTRIES,
    // Installed UKIs
    UKI_INSTALLED_ISO_DIR,
    // OS identity
    OS_NAME, OS_ID, OS_VERSION,
    // Checksum
    ISO_CHECKSUM_SUFFIX,
};
use reciso::{IsoConfig, UkiSource};
use crate::component::custom::create_live_overlay_at;

/// Get ISO volume label from environment or use default.
/// Used for boot device detection (root=LABEL=X).
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string())
}

/// Paths used during ISO creation.
struct IsoPaths {
    output_dir: PathBuf,
    rootfs: PathBuf,
    initramfs_live: PathBuf,
    initramfs_installed: PathBuf,
    iso_output: PathBuf,
}

impl IsoPaths {
    fn new(base_dir: &Path) -> Self {
        let output_dir = base_dir.join("output");
        Self {
            output_dir: output_dir.clone(),
            rootfs: output_dir.join(ROOTFS_NAME),
            initramfs_live: output_dir.join(INITRAMFS_LIVE_OUTPUT),
            initramfs_installed: output_dir.join(INITRAMFS_INSTALLED_OUTPUT),
            iso_output: output_dir.join(ISO_FILENAME),
        }
    }
}

/// Create ISO using EROFS-based architecture.
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts EROFS + overlay
/// - EROFS image (~350MB) - complete base system
/// - Live overlay - live-specific configs (autologin, serial console, empty root password)
///
/// Boot flow:
/// 1. kernel -> tiny initramfs
/// 2. init_tiny mounts EROFS as lower layer
/// 3. init_tiny mounts /live/overlay from ISO as middle layer
/// 4. init_tiny mounts tmpfs as upper layer (for writes)
/// 5. switch_root -> systemd
///
/// This architecture ensures:
/// - Live ISO has autologin and empty root password (via overlay)
/// - Installed systems (via recstrap) have proper security (EROFS only)
pub fn create_iso(base_dir: &Path) -> Result<()> {
    let paths = IsoPaths::new(base_dir);

    println!("=== Building LevitateOS ISO (Atomic) ===\n");

    // Stage 1: Validate inputs
    validate_iso_inputs(&paths)?;
    let kernel_path = find_kernel(&paths)?;

    // Stage 2: Create live overlay (autologin, serial console, empty root password)
    // This is ONLY applied during live boot, NOT extracted to installed systems
    create_live_overlay_at(&paths.output_dir)?;

    // Stage 3: Build installed UKIs (for users to copy during installation)
    // These need to be created before the ISO since they go into boot/uki/
    let installed_uki_dir = paths.output_dir.join("installed-ukis");
    fs::create_dir_all(&installed_uki_dir)?;
    crate::artifact::uki::build_installed_ukis(
        &kernel_path,
        &paths.initramfs_installed,
        &installed_uki_dir,
    )?;

    // Stage 4: Build reciso config
    let label = iso_label();
    let mut config = IsoConfig::new(
        &kernel_path,
        &paths.initramfs_live,
        &paths.rootfs,
        &label,
        paths.output_dir.join(format!("{}.tmp", ISO_FILENAME)),
    )
    .with_os_release(OS_NAME, OS_ID, OS_VERSION)
    .with_overlay(&paths.output_dir.join("live-overlay"));

    // Add LevitateOS-specific UKI entries
    for entry in UKI_ENTRIES {
        config.ukis.push(UkiSource::Build {
            name: entry.name.to_string(),
            extra_cmdline: entry.extra_cmdline.to_string(),
            filename: entry.filename.to_string(),
        });
    }

    // Add installed initramfs as extra file
    config.extra_files.push((
        paths.initramfs_installed.clone(),
        INITRAMFS_INSTALLED_ISO_PATH.to_string(),
    ));

    // Add installed UKIs as extra files
    fs::create_dir_all(paths.output_dir.join("iso-staging").join(UKI_INSTALLED_ISO_DIR))?;
    for entry in UKI_INSTALLED_ENTRIES {
        let src = installed_uki_dir.join(entry.filename);
        if src.exists() {
            config.extra_files.push((
                src,
                format!("{}/{}", UKI_INSTALLED_ISO_DIR, entry.filename),
            ));
        }
    }

    // Stage 5: Create the ISO using reciso (to temp file for atomicity)
    println!("Creating ISO via reciso...");
    reciso::create_iso(&config)?;

    // Stage 6: Verify hardware compatibility (WARN only, don't block ISO creation)
    println!("\nVerifying hardware compatibility...");
    let has_critical = verify_hardware_compat(base_dir)?;
    if has_critical {
        println!("[WARN] Hardware compatibility verification has critical errors, but continuing ISO build.");
    }

    // Stage 7: Atomic rename to final destination
    let temp_iso = paths.output_dir.join(format!("{}.tmp", ISO_FILENAME));
    fs::rename(&temp_iso, &paths.iso_output)?;

    // Also move the checksum
    let temp_checksum = temp_iso.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let final_checksum = paths.iso_output.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    if temp_checksum.exists() {
        fs::rename(&temp_checksum, &final_checksum)?;
    }

    // Cleanup
    let _ = fs::remove_dir_all(&installed_uki_dir);

    // Stage 8: Verify ISO contents - MUST pass before declaring success
    verify_iso(&paths.iso_output)?;

    print_iso_summary(&paths.iso_output);
    Ok(())
}

/// Helper to run hardware compat verification.
fn verify_hardware_compat(base_dir: &Path) -> Result<bool> {
    let output_dir = base_dir.join("output");
    // Firmware is installed to rootfs-staging during rootfs build, not staging
    let checker = hardware_compat::HardwareCompatChecker::new(
        output_dir.join("kernel-build/.config"),
        output_dir.join("rootfs-staging/usr/lib/firmware"),
    );

    let all_profiles = hardware_compat::profiles::get_all_profiles();
    let mut has_critical = false;

    for p in all_profiles {
        match checker.verify_profile(p.as_ref()) {
            Ok(report) => {
                report.print_summary();
                if report.has_critical_failures() {
                    has_critical = true;
                }
            }
            Err(e) => {
                println!("  [ERROR] Failed to verify profile '{}': {}", p.name(), e);
                has_critical = true;
            }
        }
    }

    Ok(has_critical)
}

/// Stage 1: Validate that required input files exist and are consistent.
fn validate_iso_inputs(paths: &IsoPaths) -> Result<()> {
    if !paths.rootfs.exists() {
        bail!(
            "EROFS rootfs not found at {}.\n\
             Run 'leviso build rootfs' first.",
            paths.rootfs.display()
        );
    }

    if !paths.initramfs_live.exists() {
        bail!(
            "Live initramfs not found at {}.\n\
             Run 'leviso build initramfs' first.",
            paths.initramfs_live.display()
        );
    }

    // Integrity Check: Verify staging kernel vs modules
    let staging_boot = paths.output_dir.join("staging/boot/vmlinuz");
    let staging_modules = paths.output_dir.join("staging/usr/lib/modules");

    if staging_boot.exists() && staging_modules.exists() {
        // Find module directory version
        let mut modules_version = None;
        for entry in fs::read_dir(&staging_modules)? {
            match entry {
                Ok(e) if e.path().is_dir() => {
                    modules_version = Some(e.file_name().to_string_lossy().to_string());
                    break;
                }
                Ok(_) => {} // Not a directory, skip
                Err(e) => {
                    eprintln!(
                        "  [WARN] Error reading staging modules directory entry: {}",
                        e
                    );
                }
            }
        }

        if let Some(version) = modules_version {
            println!("  [CHECK] Staging kernel modules version: {}", version);
        } else {
            bail!("Staging directory exists but contains no kernel modules!");
        }
    }

    Ok(())
}

/// Find the LevitateOS kernel. No fallbacks - fail fast if not built.
fn find_kernel(paths: &IsoPaths) -> Result<PathBuf> {
    let kernel = paths.output_dir.join("staging/boot/vmlinuz");
    if !kernel.exists() {
        bail!(
            "LevitateOS kernel not found at: {}\n\
             Run 'leviso build kernel' first.",
            kernel.display()
        );
    }
    Ok(kernel)
}

/// Verify ISO contents using fsdbg checklist.
///
/// Fails fast if the ISO is missing critical components or if verification tools are missing.
pub fn verify_iso(path: &Path) -> Result<()> {
    use fsdbg::checklist::iso::verify;
    use fsdbg::iso::IsoReader;

    print!("  Verifying ISO... ");

    let reader = match IsoReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("isoinfo not found") {
                println!("FAILED");
                bail!(
                    "ISO verification requires 'isoinfo' tool.\n\n\
                     Install it:\n\
                     \x20 Fedora/RHEL:   sudo dnf install genisoimage\n\
                     \x20 Debian/Ubuntu: sudo apt install genisoimage\n\
                     \x20 Arch Linux:    sudo pacman -S cdrtools\n\n\
                     Then re-run the build."
                );
            }
            println!("FAILED");
            bail!("Failed to open ISO: {}", e);
        }
    };

    let report = verify(&reader);

    if !report.is_success() {
        println!("FAILED");
        let failed: Vec<_> = report.results.iter().filter(|r| !r.passed).collect();
        let missing_items: Vec<_> = failed.iter().map(|r| r.item.as_str()).collect();
        cheat_bail!(
            protects = "Users can boot LevitateOS live ISO and install the system",
            severity = "CRITICAL",
            cheats = [
                "Move missing items to an OPTIONAL list",
                "Remove items from REQUIRED list",
                "Skip ISO verification entirely",
                "Accept partial ISO as complete",
                "Return Ok() without checking report.is_success()"
            ],
            consequence = "ISO fails to boot. Users cannot install LevitateOS.",
            "ISO verification failed. {} items missing:\n{}\n\n\
             Missing: {}\n\n\
             The ISO is incomplete and will not boot correctly.\n\
             Fix the ISO build process to include ALL required files.",
            failed.len(),
            failed
                .iter()
                .map(|r| format!(
                    "  - {} ({})",
                    r.item,
                    r.message.as_deref().unwrap_or("Missing")
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            missing_items.join(", ")
        );
    }

    println!("OK ({} items checked)", report.total());
    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== LevitateOS ISO Created (UKI Boot) ===");
    println!("  Output: {}", iso_output.display());
    match fs::metadata(iso_output) {
        Ok(meta) => {
            println!("  Size: {} MB", meta.len() / 1024 / 1024);
        }
        Err(e) => {
            eprintln!("  [WARN] Could not read ISO size: {}", e);
        }
    }
    println!("  Label: {}", iso_label());
    println!("\nContents:");
    println!("  - Live UKIs: EFI/Linux/ (boot from ISO)");
    println!("  - Installed UKIs: boot/uki/ (copy to /boot/EFI/Linux/ during install)");
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}
