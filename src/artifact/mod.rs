//! Build artifacts - initramfs, rootfs (EROFS), UKI, ISO, and qcow2 creation.
//!
//! This module contains all artifact creation logic:
//! - `initramfs` - Tiny initramfs builder (~5MB)
//! - `rootfs` - EROFS system image builder (~350MB)
//! - `uki` - Unified Kernel Image builder
//! - `iso` - Bootable ISO creation
//! - `qcow2` - Bootable VM disk image

pub mod initramfs;
pub mod iso;
pub mod qcow2;
pub mod rootfs;
pub mod uki;

pub use initramfs::{
    build_install_initramfs, build_tiny_initramfs, verify_install_initramfs, verify_live_initramfs,
};
pub use iso::{create_iso, verify_iso};
pub use qcow2::{build_qcow2, verify_qcow2};
pub use rootfs::build_rootfs;
