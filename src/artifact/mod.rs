//! Build artifacts - initramfs, rootfs (EROFS), UKI, and ISO creation.
//!
//! This module contains all artifact creation logic:
//! - `initramfs` - Tiny initramfs builder (~5MB)
//! - `rootfs` - EROFS system image builder (~350MB)
//! - `uki` - Unified Kernel Image builder
//! - `iso` - Bootable ISO creation

pub mod initramfs;
pub mod iso;
pub mod rootfs;
pub mod uki;

pub use initramfs::{build_tiny_initramfs, build_install_initramfs};
pub use iso::create_iso;
pub use rootfs::build_rootfs;
