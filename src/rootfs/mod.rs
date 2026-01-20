//! Rootfs builder for LevitateOS.
//!
//! This module builds the base system tarball that gets extracted during installation.
//! The tarball contains everything needed for a bootable LevitateOS system
//! (except the kernel, which is installed separately).
//!
//! ## Components
//!
//! - **binaries**: Coreutils, sbin utilities, systemd binaries
//! - **etc**: System configuration files (passwd, shadow, fstab, etc.)
//! - **systemd**: Unit files for installed system boot
//! - **pam**: Real PAM authentication (not permissive like live)
//! - **recipe**: Package manager integration
//!
//! # ⚠️ FALSE POSITIVES KILL PROJECTS ⚠️
//!
//! Every test and verification in this module MUST reflect what USERS need,
//! not what's convenient for developers.
//!
//! **The Cheat Pattern (DO NOT DO THIS):**
//! 1. Binary is missing from source rootfs
//! 2. Move it from CRITICAL to OPTIONAL
//! 3. Tests pass!
//! 4. Ship it!
//! 5. User: `bash: sudo: command not found`
//! 6. Project reputation destroyed
//!
//! **The Correct Pattern:**
//! 1. Binary is missing from source rootfs
//! 2. Build FAILS with clear error message
//! 3. Developer fixes the source (gets complete rootfs)
//! 4. Tests pass because requirements are MET
//! 5. User has working system
//!
//! Read: `.teams/KNOWLEDGE_false-positives-testing.md`

pub mod binary;
pub mod builder;
pub mod context;
pub mod parts;
pub mod rpm;

pub use builder::RootfsBuilder;
pub use context::BuildContext;
pub use rpm::{RpmExtractor, REQUIRED_PACKAGES};
