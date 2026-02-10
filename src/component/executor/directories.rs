//! Directory operation handlers: Op::Dir, Op::DirMode, Op::Dirs

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::build::context::BuildContext;

/// Handle Op::Dir: Create a directory
pub fn handle_dir(ctx: &BuildContext, path: &str) -> Result<()> {
    fs::create_dir_all(ctx.staging.join(path))?;
    Ok(())
}

/// Handle Op::DirMode: Create a directory with specific permissions
pub fn handle_dirmode(ctx: &BuildContext, path: &str, mode: u32) -> Result<()> {
    let full_path = ctx.staging.join(path);
    fs::create_dir_all(&full_path)?;
    fs::set_permissions(&full_path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Handle Op::Dirs: Create multiple directories
pub fn handle_dirs(ctx: &BuildContext, paths: &[&str]) -> Result<()> {
    for path in paths {
        fs::create_dir_all(ctx.staging.join(path))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Op, Phase};
    use distro_builder::LicenseTracker;
    use distro_contract::PackageManager;
    use leviso_cheat_test::cheat_aware;

    // Import test helpers from parent module
    use super::super::helpers::*;

    #[cheat_aware(
        protects = "Op::Dir creates directory structure",
        severity = "MEDIUM",
        ease = "EASY",
        cheats = [
            "Skip directory creation",
            "Create wrong path",
            "Don't use create_dir_all"
        ],
        consequence = "Later file copies fail because parent directory doesn't exist"
    )]
    #[test]
    fn test_component_dir_creates_nested_structure() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let dir_component = Component {
            name: "TestDir",
            phase: Phase::Filesystem,
            ops: &[Op::Dir("var/lib/deeply/nested/directory")],
        };

        let result = super::super::execute(&ctx, &dir_component, &tracker);
        assert!(result.is_ok(), "Op::Dir should succeed: {:?}", result);

        let created_dir = env.initramfs.join("var/lib/deeply/nested/directory");
        assert!(
            created_dir.is_dir(),
            "Directory should be created at {}",
            created_dir.display()
        );
    }

    #[cheat_aware(
        protects = "Op::DirMode sets correct directory permissions",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Ignore mode parameter",
            "Use default 0755",
            "Skip chmod call"
        ],
        consequence = "Directories with wrong permissions - /tmp not sticky, /root world-readable"
    )]
    #[test]
    fn test_component_dirmode_sets_permissions() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let dir_component = Component {
            name: "TestDirMode",
            phase: Phase::Filesystem,
            ops: &[Op::DirMode("tmp", 0o1777)],
        };

        let result = super::super::execute(&ctx, &dir_component, &tracker);
        assert!(result.is_ok(), "Op::DirMode should succeed: {:?}", result);

        let dir_path = env.initramfs.join("tmp");
        assert!(dir_path.is_dir(), "Directory should be created");

        let metadata = fs::metadata(&dir_path).expect("Should get metadata");
        let mode = metadata.permissions().mode() & 0o7777;
        assert_eq!(
            mode, 0o1777,
            "Directory should have mode 1777 (sticky), got {:o}",
            mode
        );
    }

    #[cheat_aware(
        protects = "Op::Dirs creates multiple directories in one operation",
        severity = "MEDIUM",
        ease = "EASY",
        cheats = [
            "Only create first directory",
            "Skip on any error",
            "Ignore the list entirely"
        ],
        consequence = "Missing directories cause later file operations to fail"
    )]
    #[test]
    fn test_component_dirs_creates_multiple() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let dirs_component = Component {
            name: "TestDirs",
            phase: Phase::Filesystem,
            ops: &[Op::Dirs(&[
                "var/lib/service-a",
                "var/lib/service-b",
                "var/lib/service-c/nested/deep",
            ])],
        };

        let result = super::super::execute(&ctx, &dirs_component, &tracker);
        assert!(result.is_ok(), "Op::Dirs should succeed: {:?}", result);

        // All directories should exist
        assert!(
            env.initramfs.join("var/lib/service-a").is_dir(),
            "service-a directory should exist"
        );
        assert!(
            env.initramfs.join("var/lib/service-b").is_dir(),
            "service-b directory should exist"
        );
        assert!(
            env.initramfs.join("var/lib/service-c/nested/deep").is_dir(),
            "deeply nested directory should exist"
        );
    }
}
