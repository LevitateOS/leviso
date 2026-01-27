//! Build modules for creating the LevitateOS system image.
//!
//! This module contains core utilities for the build process.
//! Most build logic has been moved to the declarative component system
//! in `crate::component`.
//!
//! # Remaining modules
//!
//! - `context`: BuildContext for paths during build
//! - `filesystem`: Filesystem structure creation utilities
//! - `libdeps`: Library dependency resolution utilities
//! - `users`: User/group file manipulation utilities
//!
//! Note: Kernel building is now handled by `crate::recipe::linux()`.

pub mod context;
pub mod filesystem;
pub mod libdeps;
pub mod licenses;
pub mod users;

// Re-export commonly used items
pub use context::BuildContext;
pub use licenses::LicenseTracker;
