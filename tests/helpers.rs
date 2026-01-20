//! Shared test utilities for leviso tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
    pub fn build_context(&self) -> leviso::initramfs::context::BuildContext {
        leviso::initramfs::context::BuildContext::new(
            self.rootfs.clone(),
            self.initramfs.clone(),
            self.base_dir.clone(),
        )
    }
}

/// Create a minimal mock rootfs with basic structure.
pub fn create_mock_rootfs(rootfs: &Path) {
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

/// Create a mock executable binary file.
pub fn create_mock_binary(path: &Path) {
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent dir for binary");
    }

    // Write a minimal ELF-like header (just for testing, not a real executable)
    // For unit tests, we just need the file to exist
    fs::write(path, "#!/bin/bash\necho mock\n").expect("Failed to create mock binary");

    // Make executable
    let mut perms = fs::metadata(path).expect("Failed to get metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("Failed to set permissions");
}

/// Create a mock shared library file.
pub fn create_mock_library(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent dir for library");
    }
    // Just create an empty file for testing
    fs::write(path, b"").expect("Failed to create mock library");
}

/// Assert that a symlink exists and points to the expected target.
pub fn assert_symlink(path: &Path, expected_target: &str) {
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

/// Assert that a file contains expected content.
pub fn assert_file_contains(path: &Path, expected: &str) {
    let content = fs::read_to_string(path).expect(&format!("Failed to read file: {}", path.display()));
    assert!(
        content.contains(expected),
        "File {} does not contain expected content.\nExpected to find: {}\nActual content: {}",
        path.display(),
        expected,
        content
    );
}

/// Assert that a file exists.
pub fn assert_file_exists(path: &Path) {
    assert!(
        path.exists(),
        "Expected file to exist: {}",
        path.display()
    );
}

/// Assert that a directory exists.
pub fn assert_dir_exists(path: &Path) {
    assert!(
        path.is_dir(),
        "Expected directory to exist: {}",
        path.display()
    );
}

/// Get the path to the real initramfs root (for validation tests).
pub fn real_initramfs_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Path::new(&manifest_dir).join("output/initramfs-root")
}

/// Check if the real initramfs has been built.
pub fn initramfs_is_built() -> bool {
    real_initramfs_root().exists()
}
