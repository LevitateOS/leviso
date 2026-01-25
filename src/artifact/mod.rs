//! Build artifacts - initramfs, squashfs, and ISO creation.
//!
//! This module contains all artifact creation logic:
//! - `initramfs` - Tiny initramfs builder (~5MB)
//! - `squashfs` - Squashfs system image builder (~350MB)
//! - `iso` - Bootable ISO creation

pub mod initramfs;
pub mod iso;
pub mod squashfs;

pub use initramfs::{build_tiny_initramfs, build_install_initramfs};
pub use iso::create_squashfs_iso;
pub use squashfs::build_squashfs;
