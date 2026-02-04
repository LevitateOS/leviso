//! License tracking and copying for redistributed binaries.
//!
//! Tracks which packages are used during the build and copies their license
//! files from `/usr/share/licenses/<package>/` to the staging directory.

use anyhow::{Context, Result};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Tracks packages used during the build for license compliance.
///
/// When binaries or libraries are copied, register them with this tracker.
/// After the build completes, call `copy_licenses()` to copy all license files.
pub struct LicenseTracker {
    packages: RefCell<HashSet<String>>,
}

impl LicenseTracker {
    /// Create a new license tracker.
    pub fn new() -> Self {
        Self {
            packages: RefCell::new(HashSet::new()),
        }
    }

    /// Register a binary that was copied.
    ///
    /// Looks up the package name from the binary mapping and adds it to the set.
    pub fn register_binary(&self, binary: &str) {
        if let Some(pkg) = distro_spec::shared::licenses::package_for_binary(binary) {
            self.packages.borrow_mut().insert(pkg.to_string());
        }
    }

    /// Register a library that was copied.
    ///
    /// Looks up the package name from the library mapping and adds it to the set.
    pub fn register_library(&self, lib: &str) {
        if let Some(pkg) = distro_spec::shared::licenses::package_for_library(lib) {
            self.packages.borrow_mut().insert(pkg.to_string());
        }
    }

    /// Register a package directly by name.
    ///
    /// Use this for content that doesn't go through the binary/library mappings,
    /// such as firmware, kernel modules, or data files.
    pub fn register_package(&self, package: &str) {
        self.packages.borrow_mut().insert(package.to_string());
    }

    /// Get the number of packages tracked.
    pub fn package_count(&self) -> usize {
        self.packages.borrow().len()
    }

    /// Copy license directories for all used packages.
    ///
    /// Copies from `source/usr/share/licenses/<pkg>/` to `staging/usr/share/licenses/<pkg>/`.
    /// Returns the number of license directories copied.
    pub fn copy_licenses(&self, source: &Path, staging: &Path) -> Result<usize> {
        let license_src = source.join("usr/share/licenses");
        let license_dst = staging.join("usr/share/licenses");
        fs::create_dir_all(&license_dst)?;

        let packages = self.packages.borrow();
        let mut copied = 0;
        let mut missing = Vec::new();

        for pkg in packages.iter() {
            let src = license_src.join(pkg);
            let dst = license_dst.join(pkg);

            if src.is_dir() {
                copy_dir_recursive(&src, &dst)
                    .with_context(|| format!("copying licenses for {}", pkg))?;
                copied += 1;
            } else {
                missing.push(pkg.as_str());
            }
        }

        if !missing.is_empty() {
            println!(
                "  Note: {} packages have no license dir: {}",
                missing.len(),
                missing.join(", ")
            );
        }

        Ok(copied)
    }
}

impl Default for LicenseTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if src_path.is_symlink() {
            let target = fs::read_link(&src_path)?;
            if !dst_path.exists() && !dst_path.is_symlink() {
                std::os::unix::fs::symlink(&target, &dst_path)?;
            }
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_registers_binaries() {
        let tracker = LicenseTracker::new();
        tracker.register_binary("bash");
        tracker.register_binary("ls");
        tracker.register_binary("cat"); // Also coreutils, deduped

        // bash + coreutils = 2 unique packages
        assert_eq!(tracker.package_count(), 2);
    }

    #[test]
    fn test_tracker_registers_libraries() {
        let tracker = LicenseTracker::new();
        tracker.register_library("libc.so.6");
        tracker.register_library("libpam.so.0");

        // glibc + pam = 2 packages
        assert_eq!(tracker.package_count(), 2);
    }

    #[test]
    fn test_tracker_deduplicates() {
        let tracker = LicenseTracker::new();
        tracker.register_binary("ls");
        tracker.register_binary("cat");
        tracker.register_binary("cp");
        tracker.register_binary("mv");

        // All coreutils = 1 package
        assert_eq!(tracker.package_count(), 1);
    }

    #[test]
    fn test_unknown_binaries_ignored() {
        let tracker = LicenseTracker::new();
        tracker.register_binary("nonexistent-binary");

        assert_eq!(tracker.package_count(), 0);
    }

    #[test]
    fn test_register_package_directly() {
        let tracker = LicenseTracker::new();
        tracker.register_package("linux-firmware");
        tracker.register_package("tzdata");
        tracker.register_package("kbd");

        assert_eq!(tracker.package_count(), 3);
    }

    #[test]
    fn test_mixed_registration() {
        let tracker = LicenseTracker::new();
        // Via binary mapping
        tracker.register_binary("bash");
        // Via library mapping
        tracker.register_library("libc.so.6");
        // Direct registration
        tracker.register_package("linux-firmware");

        // bash + glibc + linux-firmware = 3 packages
        assert_eq!(tracker.package_count(), 3);
    }
}
