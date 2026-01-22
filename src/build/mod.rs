//! Build modules for creating the LevitateOS system image.
//!
//! This module contains all the components needed to build a complete
//! LevitateOS system from a Rocky Linux rootfs.
//!
//! # Architecture
//!
//! The squashfs serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! DESIGN: Live = Installed (same content, zero duplication)

pub mod binary;
pub mod binaries;
pub mod chrony;
pub mod context;
pub mod dbus;
pub mod etc;
pub mod filesystem;
pub mod kernel;
pub mod modules;
pub mod network;
pub mod pam;
pub mod recipe;
pub mod systemd;
pub mod users;

// Re-export commonly used items
pub use context::BuildContext;
