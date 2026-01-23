//! Component builder - orchestrates component installation.
//!
//! This module provides the high-level `build_system()` function that
//! installs all components in the correct order.

use anyhow::Result;

use super::definitions::*;
use super::executor;
use crate::build::context::BuildContext;

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
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("Building complete system for squashfs...");

    // Phase 1: Filesystem
    executor::execute(ctx, &FILESYSTEM)?;

    // Phase 2: Binaries
    executor::execute(ctx, &SHELL)?;
    executor::execute(ctx, &COREUTILS)?;
    executor::execute(ctx, &SBIN_BINARIES)?;
    executor::execute(ctx, &SYSTEMD_BINS)?;

    // Phase 3: Systemd
    executor::execute(ctx, &SYSTEMD_UNITS)?;
    executor::execute(ctx, &GETTY)?;
    executor::execute(ctx, &UDEV)?;
    executor::execute(ctx, &TMPFILES)?;
    executor::execute(ctx, &LIVE_SYSTEMD)?;

    // Phase 4: D-Bus (using Service abstraction)
    executor::execute(ctx, &DBUS_SVC)?;

    // Phase 5: Services (using Service abstraction where applicable)
    executor::execute(ctx, &NETWORK)?;  // Has custom ops, keeping as Component
    executor::execute(ctx, &CHRONY_SVC)?;
    executor::execute(ctx, &OPENSSH_SVC)?;
    executor::execute(ctx, &PAM)?;
    executor::execute(ctx, &MODULES)?;

    // Phase 6: Config
    executor::execute(ctx, &ETC_CONFIG)?;

    // Phase 7: Packages
    executor::execute(ctx, &RECIPE)?;
    executor::execute(ctx, &DRACUT)?;
    executor::execute(ctx, &BOOTLOADER)?;

    // Phase 8: Firmware
    executor::execute(ctx, &FIRMWARE)?;

    // Phase 9: Final
    executor::execute(ctx, &FINAL)?;

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
            (UDEV.name(), UDEV.phase()),
            (TMPFILES.name(), TMPFILES.phase()),
            (LIVE_SYSTEMD.name(), LIVE_SYSTEMD.phase()),
            (DBUS_SVC.name(), DBUS_SVC.phase()),
            (NETWORK.name(), NETWORK.phase()),
            (CHRONY_SVC.name(), CHRONY_SVC.phase()),
            (OPENSSH_SVC.name(), OPENSSH_SVC.phase()),
            (PAM.name(), PAM.phase()),
            (MODULES.name(), MODULES.phase()),
            (ETC_CONFIG.name(), ETC_CONFIG.phase()),
            (RECIPE.name(), RECIPE.phase()),
            (DRACUT.name(), DRACUT.phase()),
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
            assert!(
                names.insert(*name),
                "Duplicate component name: {}",
                name
            );
        }
    }
}
