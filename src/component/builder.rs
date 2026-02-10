//! Component builder - orchestrates component installation.
//!
//! This module provides the high-level `build_system()` function that
//! installs all components in the correct order.

use anyhow::Result;

use super::definitions::*;
use super::executor;
use crate::build::context::BuildContext;
use crate::timing::Timer;
use distro_contract::PackageManager;
use distro_builder::LicenseTracker;

/// Build the complete system into the staging directory.
///
/// Components and Services are installed in phase order:
/// 1. Filesystem - directories must exist before files
/// 2. Binaries - shells and tools before services
/// 3. Systemd - unit files before enabling
/// 4. D-Bus - before services that need it
/// 5. Services - network, chrony, ssh, pam
/// 6. Config - /etc files
/// 7. Packages - recipe, dracut
/// 8. Firmware - hardware support
/// 9. Final - welcome message, installer tools
/// 10. Licenses - copy license files for all redistributed packages
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("Building complete system for rootfs (EROFS)...");

    // Track licenses for all binaries we copy
    let tracker = LicenseTracker::new(ctx.source.clone(), PackageManager::Rpm);

    // Phase 1: Filesystem
    let t = Timer::start("Filesystem");
    executor::execute(ctx, &FILESYSTEM, &tracker)?;
    t.finish();

    // Phase 2: Binaries
    let t = Timer::start("Binaries");
    executor::execute(ctx, &SHELL, &tracker)?;
    executor::execute(ctx, &COREUTILS, &tracker)?;
    executor::execute(ctx, &SBIN_BINARIES, &tracker)?;
    executor::execute(ctx, &SYSTEMD_BINS, &tracker)?;
    t.finish();

    // Phase 3: Systemd
    let t = Timer::start("Systemd");
    executor::execute(ctx, &SYSTEMD_UNITS, &tracker)?;
    executor::execute(ctx, &GETTY, &tracker)?;
    executor::execute(ctx, &EFIVARS, &tracker)?; // EFI variable filesystem for efibootmgr
    executor::execute(ctx, &UDEV, &tracker)?;
    executor::execute(ctx, &TMPFILES, &tracker)?;
    executor::execute(ctx, &LIVE_SYSTEMD, &tracker)?;
    t.finish();

    // Phase 4: D-Bus (using Service abstraction)
    let t = Timer::start("D-Bus");
    executor::execute(ctx, &DBUS_SVC, &tracker)?;
    t.finish();

    // Phase 5: Services (using Service abstraction where applicable)
    let t = Timer::start("Services");
    executor::execute(ctx, &NETWORK, &tracker)?; // Has custom ops, keeping as Component
    executor::execute(ctx, &CHRONY_SVC, &tracker)?;
    executor::execute(ctx, &OPENSSH_SVC, &tracker)?;
    executor::execute(ctx, &PAM, &tracker)?;
    executor::execute(ctx, &MODULES, &tracker)?;
    // Desktop services
    executor::execute(ctx, &BLUETOOTH_SVC, &tracker)?;
    executor::execute(ctx, &PIPEWIRE_SVC, &tracker)?;
    executor::execute(ctx, &POLKIT_SVC, &tracker)?;
    executor::execute(ctx, &UDISKS_SVC, &tracker)?;
    executor::execute(ctx, &UPOWER_SVC, &tracker)?;
    t.finish();

    // Phase 6: Config
    let t = Timer::start("Config");
    executor::execute(ctx, &ETC_CONFIG, &tracker)?;
    t.finish();

    // Phase 7: Packages
    // NOTE: DRACUT removed - initramfs built using custom rootless builder
    let t = Timer::start("Packages");
    executor::execute(ctx, &RECIPE, &tracker)?;
    executor::execute(ctx, &BOOTLOADER, &tracker)?;
    t.finish();

    // Phase 8: Firmware
    let t = Timer::start("Firmware");
    executor::execute(ctx, &FIRMWARE, &tracker)?;
    t.finish();

    // Phase 9: Final
    let t = Timer::start("Final");
    executor::execute(ctx, &FINAL, &tracker)?;
    t.finish();

    // Phase 10: Licenses - copy license files for all redistributed packages
    let t = Timer::start("Licenses");
    let license_count = tracker.copy_licenses(&ctx.source, &ctx.staging)?;
    println!("  Copied licenses for {} packages", license_count);
    t.finish();

    println!("System build complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Installable, Phase};

    /// Get all installables in the order they're executed.
    fn all_installables() -> Vec<(&'static str, Phase)> {
        vec![
            (FILESYSTEM.name(), FILESYSTEM.phase()),
            (SHELL.name(), SHELL.phase()),
            (COREUTILS.name(), COREUTILS.phase()),
            (SBIN_BINARIES.name(), SBIN_BINARIES.phase()),
            (SYSTEMD_BINS.name(), SYSTEMD_BINS.phase()),
            (SYSTEMD_UNITS.name(), SYSTEMD_UNITS.phase()),
            (GETTY.name(), GETTY.phase()),
            (EFIVARS.name(), EFIVARS.phase()),
            (UDEV.name(), UDEV.phase()),
            (TMPFILES.name(), TMPFILES.phase()),
            (LIVE_SYSTEMD.name(), LIVE_SYSTEMD.phase()),
            (DBUS_SVC.name(), DBUS_SVC.phase()),
            (NETWORK.name(), NETWORK.phase()),
            (CHRONY_SVC.name(), CHRONY_SVC.phase()),
            (OPENSSH_SVC.name(), OPENSSH_SVC.phase()),
            (PAM.name(), PAM.phase()),
            (MODULES.name(), MODULES.phase()),
            // Desktop services
            (BLUETOOTH_SVC.name(), BLUETOOTH_SVC.phase()),
            (PIPEWIRE_SVC.name(), PIPEWIRE_SVC.phase()),
            (POLKIT_SVC.name(), POLKIT_SVC.phase()),
            (UDISKS_SVC.name(), UDISKS_SVC.phase()),
            (UPOWER_SVC.name(), UPOWER_SVC.phase()),
            (ETC_CONFIG.name(), ETC_CONFIG.phase()),
            (RECIPE.name(), RECIPE.phase()),
            (BOOTLOADER.name(), BOOTLOADER.phase()),
            (FIRMWARE.name(), FIRMWARE.phase()),
            (FINAL.name(), FINAL.phase()),
        ]
    }

    #[test]
    fn test_components_are_ordered_by_phase() {
        let installables = all_installables();
        let mut prev_phase = None;
        for (name, phase) in &installables {
            if let Some(prev) = prev_phase {
                assert!(
                    *phase >= prev,
                    "Component '{}' (phase {:?}) comes after a component with later phase {:?}",
                    name,
                    phase,
                    prev
                );
            }
            prev_phase = Some(*phase);
        }
    }

    #[test]
    fn test_all_components_have_unique_names() {
        let installables = all_installables();
        let mut names = std::collections::HashSet::new();
        for (name, _) in &installables {
            assert!(names.insert(*name), "Duplicate component name: {}", name);
        }
    }
}
