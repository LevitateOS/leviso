//! Rootfs construction modules.
//!
//! This module contains all the components needed to build a complete
//! installed system rootfs for LevitateOS.

pub mod binaries;
pub mod etc;
pub mod filesystem;
pub mod pam;
pub mod recipe;
pub mod systemd;
