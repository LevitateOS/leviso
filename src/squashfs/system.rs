//! Complete system builder for squashfs.
//!
//! Builds the complete system by merging:
//! - Binaries, PAM, systemd, sudo, recipe (complete user-facing tools)
//! - Networking (NetworkManager, wpa_supplicant, WiFi firmware)
//! - D-Bus (required for systemctl, timedatectl, etc.)
//! - Chrony (NTP time synchronization)
//! - Kernel modules (for hardware support)
//!
//! The result is a single image that serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! DESIGN: Live = Installed (same content, zero duplication)
//!
//! # Architecture
//!
//! This module delegates to the declarative component system in
//! `crate::component`. All build logic is defined there.

use anyhow::Result;

use crate::build::BuildContext;

/// Build the complete system into the staging directory.
///
/// This function delegates to the declarative component system.
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    crate::component::build_system(ctx)
}
