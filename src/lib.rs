//! Leviso library exports for testing.
//!
//! This module exposes internal components for integration testing.
//!
//! # STOP. READ. THEN ACT.
//!
//! Before writing any code in this crate:
//! 1. Read the existing modules to understand what exists
//! 2. Unit tests go in `leviso/tests/` - NOT E2E QEMU tests
//! 3. E2E installation tests go in the `install-tests` crate
//!
//! See `leviso/tests/README.md` and `STOP_READ_THEN_ACT.md` for details.

pub mod config;
pub mod initramfs;
