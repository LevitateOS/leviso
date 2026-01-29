//! Test helpers for executor module tests.
//!
//! Shared utilities for all executor operation tests.

#![cfg(test)]

use crate::build::context::BuildContext;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test environment with temporary directories for rootfs and initramfs.
pub struct TestEnv {
    /// Temporary directory (kept alive for lifetime of TestEnv)
    pub _temp_dir: TempDir,
    /// Mock rootfs directory (source of binaries)
    pub rootfs: PathBuf,
    /// Initramfs directory (build destination)
    pub initramfs: PathBuf,
    /// Base directory (project root simulation)
    pub base_dir: PathBuf,
}

impl TestEnv {
    /// Create a new test environment with temporary directories.
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base = temp_dir.path();

        let rootfs = base.join("rootfs");
        let initramfs = base.join("initramfs");
        let base_dir = base.to_path_buf();

        fs::create_dir_all(&rootfs).expect("Failed to create rootfs dir");
        fs::create_dir_all(&initramfs).expect("Failed to create initramfs dir");

        Self {
            _temp_dir: temp_dir,
            rootfs,
            initramfs,
            base_dir,
        }
    }

    /// Create the build context for testing.
    pub fn build_context(&self) -> BuildContext {
        BuildContext::for_testing(&self.rootfs, &self.initramfs, &self.base_dir)
    }
}

/// Create a minimal mock rootfs with basic structure.
pub fn create_mock_rootfs(rootfs: &std::path::Path) {
    let dirs = [
        "usr/bin",
        "usr/sbin",
        "bin",
        "sbin",
        "usr/lib64",
        "lib64",
        "usr/lib/systemd/system",
        "usr/lib/systemd",
        "usr/lib64/security",
        "usr/share/dbus-1/system.d",
        "usr/share/dbus-1/system-services",
        "etc",
        "usr/lib/kbd/keymaps",
    ];

    for dir in dirs {
        fs::create_dir_all(rootfs.join(dir)).expect("Failed to create mock rootfs dir");
    }

    // Create passwd and group files
    fs::write(
        rootfs.join("etc/passwd"),
        "root:x:0:0:root:/root:/bin/bash\ndbus:x:81:81:System message bus:/:/sbin/nologin\n",
    )
    .expect("Failed to create passwd");

    fs::write(
        rootfs.join("etc/group"),
        "root:x:0:\ndbus:x:81:\n",
    )
    .expect("Failed to create group");
}

/// Assert that a file exists.
pub fn assert_file_exists(path: &std::path::Path) {
    assert!(
        path.exists(),
        "Expected file to exist: {}",
        path.display()
    );
}

/// Assert that a file contains expected content.
pub fn assert_file_contains(path: &std::path::Path, expected: &str) {
    let content =
        fs::read_to_string(path).expect(&format!("Failed to read file: {}", path.display()));
    assert!(
        content.contains(expected),
        "File {} does not contain expected content.\nExpected to find: {}\nActual content: {}",
        path.display(),
        expected,
        content
    );
}

/// Assert that a symlink exists and points to the expected target.
pub fn assert_symlink(path: &std::path::Path, expected_target: &str) {
    assert!(
        path.is_symlink(),
        "Expected symlink at {}, but it's not a symlink",
        path.display()
    );

    let target = fs::read_link(path).expect("Failed to read symlink");
    assert_eq!(
        target.to_string_lossy(),
        expected_target,
        "Symlink {} points to {:?}, expected {}",
        path.display(),
        target,
        expected_target
    );
}

/// Macro to reduce boilerplate in executor tests.
///
/// Sets up a complete test environment in a single call:
/// - Creates TestEnv with temporary directories
/// - Creates mock rootfs structure
/// - Builds BuildContext
/// - Creates LicenseTracker
///
/// # Example
/// ```ignore
/// #[test]
/// fn test_something() {
///     let (env, ctx, tracker) = setup_test_env!();
///     // Ready to use env, ctx, tracker
/// }
/// ```
#[macro_export]
macro_rules! setup_test_env {
    () => {{
        use crate::build::licenses::LicenseTracker;
        let env = $crate::component::executor::helpers::TestEnv::new();
        $crate::component::executor::helpers::create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();
        (env, ctx, tracker)
    }};
}
