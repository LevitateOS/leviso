//! File operation handlers: Op::CopyFile, Op::CopyTree, Op::WriteFile, Op::WriteFileMode, Op::Symlink
//!
//! Hybrid approach:
//! - CopyFile/CopyTree: Keep leviso-specific logic (library deps, Rocky rootfs handling)
//! - WriteFile/WriteFileMode/Symlink: Delegate to distro-builder (generic implementations)

use anyhow::bail;
use anyhow::Result;
use std::path::Path;

use crate::build::context::BuildContext;
use crate::build::libdeps::{copy_dir_tree, copy_file};
use crate::common::ensure_parent_exists;
use leviso_elf::create_symlink_if_missing;

/// Handle Op::CopyFile: Copy a file from rootfs
///
/// Uses leviso-specific library dependency handling from libdeps
pub fn handle_copyfile(ctx: &BuildContext, path: &str) -> Result<()> {
    let found = copy_file(ctx, path)?;
    if !found {
        bail!("{} not found", path);
    }
    Ok(())
}

/// Handle Op::CopyTree: Copy an entire directory tree
///
/// Uses leviso-specific handling from libdeps for Rocky rootfs layout
pub fn handle_copytree(ctx: &BuildContext, path: &str) -> Result<()> {
    copy_dir_tree(ctx, path)?;
    Ok(())
}

/// Handle Op::WriteFile: Write a file with content
///
/// Delegates to distro-builder's generic implementation
pub fn handle_writefile(ctx: &BuildContext, path: &str, content: &str) -> Result<()> {
    distro_builder::executor::files::handle_writefile(&ctx.staging, path, content)
}

/// Handle Op::WriteFileMode: Write a file with specific permissions
///
/// Delegates to distro-builder's generic implementation
pub fn handle_writefilemode(
    ctx: &BuildContext,
    path: &str,
    content: &str,
    mode: u32,
) -> Result<()> {
    distro_builder::executor::files::handle_writefilemode(&ctx.staging, path, content, mode)
}

/// Handle Op::Symlink: Create a symlink
///
/// Delegates to distro-builder's generic implementation
pub fn handle_symlink(ctx: &BuildContext, link: &str, target: &str) -> Result<()> {
    distro_builder::executor::files::handle_symlink(&ctx.staging, link, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Op, Phase};
    use distro_builder::LicenseTracker;
    use distro_contract::PackageManager;
    use leviso_cheat_test::cheat_aware;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    // Import test helpers from parent module
    use super::super::helpers::*;

    #[cheat_aware(
        protects = "Op::WriteFile creates file with correct content",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Write empty file",
            "Write to wrong path",
            "Truncate content"
        ],
        consequence = "Config files have wrong content, services fail to start"
    )]
    #[test]
    fn test_component_writefile_creates_content() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let content = "test-content-12345\nline two\n";
        let write_component = Component {
            name: "TestWriteFile",
            phase: Phase::Config,
            ops: &[Op::WriteFile(
                "etc/test-config.conf",
                "test-content-12345\nline two\n",
            )],
        };

        let result = super::super::execute(&ctx, &write_component, &tracker);
        assert!(result.is_ok(), "Op::WriteFile should succeed: {:?}", result);

        let file_path = env.initramfs.join("etc/test-config.conf");
        assert!(file_path.exists(), "File should be created");

        let written = fs::read_to_string(&file_path).expect("Should read file");
        assert_eq!(written, content, "Content should match exactly");
    }

    #[cheat_aware(
        protects = "Op::Symlink creates working symlink",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Create regular file instead of symlink",
            "Point symlink to wrong target",
            "Skip if symlink exists (wrong target)"
        ],
        consequence = "Merged-usr symlinks broken, binaries not found at expected paths"
    )]
    #[test]
    fn test_component_symlink_creates_link() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let symlink_component = Component {
            name: "TestSymlink",
            phase: Phase::Filesystem,
            ops: &[Op::Symlink("bin", "usr/bin")],
        };

        let result = super::super::execute(&ctx, &symlink_component, &tracker);
        assert!(result.is_ok(), "Op::Symlink should succeed: {:?}", result);

        let link_path = env.initramfs.join("bin");
        assert!(link_path.is_symlink(), "Should be a symlink");

        let target = fs::read_link(&link_path).expect("Should read symlink");
        assert_eq!(
            target.to_string_lossy(),
            "usr/bin",
            "Symlink should point to usr/bin"
        );
    }

    #[cheat_aware(
        protects = "Op::WriteFileMode sets correct file permissions",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Ignore mode parameter",
            "Use default permissions",
            "Skip chmod call"
        ],
        consequence = "Sensitive files world-readable, security vulnerability"
    )]
    #[test]
    fn test_component_writefilemode_sets_permissions() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let write_component = Component {
            name: "TestWriteFileMode",
            phase: Phase::Config,
            ops: &[Op::WriteFileMode(
                "etc/shadow-test",
                "root:!:19000::::::",
                0o600,
            )],
        };

        let result = super::super::execute(&ctx, &write_component, &tracker);
        assert!(
            result.is_ok(),
            "Op::WriteFileMode should succeed: {:?}",
            result
        );

        let file_path = env.initramfs.join("etc/shadow-test");
        assert!(file_path.exists(), "File should be created");

        let metadata = fs::metadata(&file_path).expect("Should get metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "File should have mode 0600, got {:o}", mode);
    }

    #[cheat_aware(
        protects = "Op::CopyFile fails loudly when source file missing",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Return Ok(()) when file missing",
            "Create empty file instead",
            "Log warning and continue"
        ],
        consequence = "Critical config files missing from system, services fail to start"
    )]
    #[test]
    fn test_component_copyfile_missing_fails() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let copy_component = Component {
            name: "TestCopyFileMissing",
            phase: Phase::Config,
            ops: &[Op::CopyFile("etc/nonexistent-config-file.conf")],
        };

        let result = super::super::execute(&ctx, &copy_component, &tracker);
        assert!(
            result.is_err(),
            "Op::CopyFile should fail when source file doesn't exist"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found") || err_msg.contains("nonexistent"),
            "Error should indicate file not found: {}",
            err_msg
        );
    }

    #[cheat_aware(
        protects = "Op::CopyFile succeeds when source file exists",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Always report file not found",
            "Copy empty file",
            "Truncate content"
        ],
        consequence = "Config files not copied, services misconfigured"
    )]
    #[test]
    fn test_component_copyfile_copies_existing() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);

        // Create a config file in rootfs
        let src_config = env.rootfs.join("etc/test-app.conf");
        fs::create_dir_all(src_config.parent().unwrap()).unwrap();
        fs::write(&src_config, "setting=value\nother=123\n").unwrap();

        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let copyfile_component = Component {
            name: "TestCopyFileSuccess",
            phase: Phase::Config,
            ops: &[Op::CopyFile("etc/test-app.conf")],
        };

        let result = super::super::execute(&ctx, &copyfile_component, &tracker);
        assert!(result.is_ok(), "Op::CopyFile should succeed: {:?}", result);

        // Verify file was copied with correct content
        let dst_config = env.initramfs.join("etc/test-app.conf");
        assert_file_exists(&dst_config);
        assert_file_contains(&dst_config, "setting=value");
        assert_file_contains(&dst_config, "other=123");
    }

    #[cheat_aware(
        protects = "Op::CopyTree copies entire directory tree from rootfs",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Only copy top-level directory",
            "Skip files, only copy dirs",
            "Ignore symlinks in tree"
        ],
        consequence = "Missing nested files - config directories incomplete"
    )]
    #[test]
    fn test_component_copytree_copies_structure() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);

        // Create a directory tree in the mock rootfs to copy
        let src_tree = env.rootfs.join("usr/share/test-config");
        fs::create_dir_all(src_tree.join("subdir")).unwrap();
        fs::write(src_tree.join("main.conf"), "main config").unwrap();
        fs::write(src_tree.join("subdir/nested.conf"), "nested config").unwrap();

        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let copytree_component = Component {
            name: "TestCopyTree",
            phase: Phase::Config,
            ops: &[Op::CopyTree("usr/share/test-config")],
        };

        let result = super::super::execute(&ctx, &copytree_component, &tracker);
        assert!(result.is_ok(), "Op::CopyTree should succeed: {:?}", result);

        // Verify entire tree was copied
        let dst_tree = env.initramfs.join("usr/share/test-config");
        assert!(dst_tree.is_dir(), "Root directory should exist");
        assert!(
            dst_tree.join("subdir").is_dir(),
            "Subdirectory should exist"
        );
        assert_file_exists(&dst_tree.join("main.conf"));
        assert_file_exists(&dst_tree.join("subdir/nested.conf"));
        assert_file_contains(&dst_tree.join("main.conf"), "main config");
        assert_file_contains(&dst_tree.join("subdir/nested.conf"), "nested config");
    }

    #[cheat_aware(
        protects = "Op::CopyTree preserves symlinks in directory tree",
        severity = "MEDIUM",
        ease = "MEDIUM",
        cheats = [
            "Convert symlinks to regular files",
            "Skip symlinks entirely",
            "Resolve symlinks and copy target"
        ],
        consequence = "Symlink-based configs broken, wrong files used at runtime"
    )]
    #[test]
    fn test_component_copytree_preserves_symlinks() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);

        // Create a directory tree with a symlink
        let src_tree = env.rootfs.join("etc/test-service");
        fs::create_dir_all(&src_tree).unwrap();
        fs::write(src_tree.join("real.conf"), "real content").unwrap();
        std::os::unix::fs::symlink("real.conf", src_tree.join("link.conf")).unwrap();

        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let copytree_component = Component {
            name: "TestCopyTreeSymlink",
            phase: Phase::Config,
            ops: &[Op::CopyTree("etc/test-service")],
        };

        let result = super::super::execute(&ctx, &copytree_component, &tracker);
        assert!(result.is_ok(), "Op::CopyTree should succeed: {:?}", result);

        // Verify symlink was preserved
        let dst_link = env.initramfs.join("etc/test-service/link.conf");
        assert!(
            dst_link.is_symlink(),
            "Symlink should be preserved, not converted to file"
        );
        assert_symlink(&dst_link, "real.conf");
    }

    #[cheat_aware(
        protects = "All operations in a component execute in order",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Execute operations out of order",
            "Skip some operations",
            "Stop on first operation"
        ],
        consequence = "Incomplete component installation, missing files or directories"
    )]
    #[test]
    fn test_component_all_operations_execute() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let multi_op_component = Component {
            name: "TestMultiOp",
            phase: Phase::Config,
            ops: &[
                Op::Dir("var/lib/multitest"),
                Op::WriteFile("var/lib/multitest/file1.txt", "content1"),
                Op::WriteFile("var/lib/multitest/file2.txt", "content2"),
                Op::Symlink("var/lib/multitest/link", "file1.txt"),
            ],
        };

        let result = super::super::execute(&ctx, &multi_op_component, &tracker);
        assert!(
            result.is_ok(),
            "All operations should succeed: {:?}",
            result
        );

        // Verify all operations executed
        assert!(
            env.initramfs.join("var/lib/multitest").is_dir(),
            "Directory should exist"
        );
        assert!(
            env.initramfs.join("var/lib/multitest/file1.txt").exists(),
            "file1.txt should exist"
        );
        assert!(
            env.initramfs.join("var/lib/multitest/file2.txt").exists(),
            "file2.txt should exist"
        );
        assert!(
            env.initramfs.join("var/lib/multitest/link").is_symlink(),
            "symlink should exist"
        );
    }
}
