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
//! - `kernel`: Interactive kernel compilation (stays imperative)
//! - `libdeps`: Library dependency resolution utilities
//! - `users`: User/group file manipulation utilities

pub mod context;
pub mod filesystem;
pub mod kernel;
pub mod libdeps;
pub mod users;

// Re-export commonly used items
pub use context::BuildContext;
