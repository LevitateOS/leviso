//! Component builder - orchestrates component installation.
//!
//! This module provides the high-level `build_system()` function that
//! installs all components in the correct order.

use anyhow::Result;

use super::definitions::*;
use super::executor;
use super::Component;
use crate::build::context::BuildContext;

/// All system components in dependency order.
///
/// Components are sorted by phase, which ensures proper ordering:
/// 1. Filesystem - directories must exist before files
/// 2. Binaries - shells and tools before services
/// 3. Systemd - unit files before enabling
/// 4. D-Bus - before services that need it
/// 5. Services - network, chrony, ssh, pam
/// 6. Config - /etc files
/// 7. Packages - recipe, dracut
/// 8. Firmware - hardware support
/// 9. Final - welcome message, installer tools
const COMPONENTS: &[&Component] = &[
    // Phase 1: Filesystem
    &FILESYSTEM,
    // Phase 2: Binaries
    &SHELL,
    &COREUTILS,
    &SBIN_BINARIES,
    &SYSTEMD_BINS,
    // Phase 3: Systemd
    &SYSTEMD_UNITS,
    &GETTY,
    &UDEV,
    &TMPFILES,
    &LIVE_SYSTEMD,
    // Phase 4: D-Bus
    &DBUS,
    // Phase 5: Services
    &NETWORK,
    &CHRONY,
    &OPENSSH,
    &PAM,
    &MODULES,
    // Phase 6: Config
    &ETC_CONFIG,
    // Phase 7: Packages
    &RECIPE,
    &DRACUT,
    &BOOTLOADER,
    // Phase 8: Firmware
    &FIRMWARE,
    // Phase 9: Final
    &FINAL,
];

/// Build the complete system into the staging directory.
///
/// This is the main entry point that replaces the old imperative
/// `squashfs::system::build_system()` function.
///
/// # Architecture
///
/// Instead of calling individual `setup_*()` functions imperatively,
/// we iterate over declarative component definitions and execute them.
/// This ensures:
///
/// - Consistent behavior across all components
/// - Single source of truth for what each component needs
/// - Easy to add/remove/reorder components
/// - Clear dependency ordering via phases
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("Building complete system for squashfs...");
    println!("Using declarative component system ({} components)", COMPONENTS.len());

    // Components are already ordered by phase in the COMPONENTS array.
    // We could sort by phase dynamically, but static ordering is clearer
    // and allows fine-tuned control within phases.

    for component in COMPONENTS {
        executor::execute(ctx, component)?;
    }

    println!("System build complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_components_are_ordered_by_phase() {
        let mut prev_phase = None;
        for component in COMPONENTS {
            if let Some(prev) = prev_phase {
                assert!(
                    component.phase >= prev,
                    "Component '{}' (phase {:?}) comes after a component with later phase {:?}",
                    component.name,
                    component.phase,
                    prev
                );
            }
            prev_phase = Some(component.phase);
        }
    }

    #[test]
    fn test_all_components_have_unique_names() {
        let mut names = std::collections::HashSet::new();
        for component in COMPONENTS {
            assert!(
                names.insert(component.name),
                "Duplicate component name: {}",
                component.name
            );
        }
    }
}
