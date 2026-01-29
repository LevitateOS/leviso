//! Component executor - interprets Op variants and performs actual operations.
//!
//! This is the single place where all build operations are implemented.
//! No more copy-paste patterns across 14 files.
//!
//! ALL operations are required. If something is listed, it must exist.
//! There is no "optional" - this is a daily driver OS, not a toy.

use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::{Dest, Installable, Op};
use crate::build::context::BuildContext;
use crate::build::libdeps::{
    copy_bash, copy_binary_with_libs, copy_dir_tree, copy_file, copy_sbin_binary_with_libs,
    copy_systemd_units, make_executable,
};
use crate::build::licenses::LicenseTracker;
use crate::build::users;
use leviso_elf::create_symlink_if_missing;

/// Execute all operations in an installable component.
pub fn execute(
    ctx: &BuildContext,
    component: &impl Installable,
    tracker: &LicenseTracker,
) -> Result<()> {
    let name = component.name();
    let ops = component.ops();

    println!("Installing {}...", name);

    for op in ops.iter() {
        execute_op(ctx, op, tracker)
            .with_context(|| format!("in component '{}': {:?}", name, op))?;
    }

    Ok(())
}

/// Execute a single operation.
fn execute_op(ctx: &BuildContext, op: &Op, tracker: &LicenseTracker) -> Result<()> {
    match op {
        // ─────────────────────────────────────────────────────────────────
        // Directory operations
        // ─────────────────────────────────────────────────────────────────
        Op::Dir(path) => {
            fs::create_dir_all(ctx.staging.join(path))?;
        }

        Op::DirMode(path, mode) => {
            let full_path = ctx.staging.join(path);
            fs::create_dir_all(&full_path)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Dirs(paths) => {
            for path in *paths {
                fs::create_dir_all(ctx.staging.join(path))?;
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Binary operations - ALL REQUIRED
        // ─────────────────────────────────────────────────────────────────
        Op::Bin(name, dest) => {
            let found = match dest {
                Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin", Some(tracker))?,
                Dest::Sbin => copy_sbin_binary_with_libs(ctx, name, Some(tracker))?,
            };
            if !found {
                bail!("{} not found", name);
            }
        }

        Op::Bins(names, dest) => {
            let mut missing = Vec::new();
            for name in *names {
                let found = match dest {
                    Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin", Some(tracker))?,
                    Dest::Sbin => copy_sbin_binary_with_libs(ctx, name, Some(tracker))?,
                };
                if !found {
                    missing.push(*name);
                }
            }
            if !missing.is_empty() {
                bail!("Missing binaries: {}", missing.join(", "));
            }
        }

        Op::Bash => {
            copy_bash(ctx, Some(tracker))?;
        }

        Op::SystemdBinaries(binaries) => {
            // Register systemd for license tracking
            tracker.register_binary("systemd");
            // Copy main systemd binary
            let systemd_src = ctx.source.join("usr/lib/systemd/systemd");
            let systemd_dst = ctx.staging.join("usr/lib/systemd/systemd");
            if systemd_src.exists() {
                fs::create_dir_all(systemd_dst.parent().unwrap())?;
                fs::copy(&systemd_src, &systemd_dst)?;
                make_executable(&systemd_dst)?;
            }

            // Copy helper binaries
            for binary in *binaries {
                let src = ctx.source.join("usr/lib/systemd").join(binary);
                let dst = ctx.staging.join("usr/lib/systemd").join(binary);
                if src.exists() {
                    fs::copy(&src, &dst)?;
                    make_executable(&dst)?;
                }
            }

            // Copy systemd private libraries
            let systemd_lib_src = ctx.source.join("usr/lib64/systemd");
            if systemd_lib_src.exists() {
                fs::create_dir_all(ctx.staging.join("usr/lib64/systemd"))?;
                for entry in fs::read_dir(&systemd_lib_src)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                        let dst = ctx.staging.join("usr/lib64/systemd").join(&name);
                        fs::copy(entry.path(), &dst)?;
                    }
                }
            }

            // Copy system-generators (e.g., systemd-fstab-generator)
            let generators_src = ctx.source.join("usr/lib/systemd/system-generators");
            if generators_src.exists() {
                let generators_dst = ctx.staging.join("usr/lib/systemd/system-generators");
                fs::create_dir_all(&generators_dst)?;
                for entry in fs::read_dir(&generators_src)? {
                    let entry = entry?;
                    let dst = generators_dst.join(entry.file_name());
                    if entry.path().is_file() && !dst.exists() {
                        fs::copy(entry.path(), &dst)?;
                        make_executable(&dst)?;
                    }
                }
            }
        }

        Op::SudoLibs(libs) => {
            // Register sudo for license tracking
            tracker.register_binary("sudo");

            let src_dir = ctx.source.join("usr/libexec/sudo");
            let dst_dir = ctx.staging.join("usr/libexec/sudo");

            if !src_dir.exists() {
                bail!("sudo libexec not found at {}", src_dir.display());
            }

            fs::create_dir_all(&dst_dir)?;

            for lib in *libs {
                let src = src_dir.join(lib);
                let dst = dst_dir.join(lib);

                if src.is_symlink() {
                    let target = fs::read_link(&src)?;
                    if dst.exists() || dst.is_symlink() {
                        fs::remove_file(&dst)?;
                    }
                    std::os::unix::fs::symlink(&target, &dst)?;
                } else if src.exists() {
                    fs::copy(&src, &dst)?;
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // File operations - ALL REQUIRED
        // ─────────────────────────────────────────────────────────────────
        Op::CopyFile(path) => {
            let found = copy_file(ctx, path)?;
            if !found {
                bail!("{} not found", path);
            }
        }

        Op::CopyTree(path) => {
            copy_dir_tree(ctx, path)?;
        }

        Op::WriteFile(path, content) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
        }

        Op::WriteFileMode(path, content, mode) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Symlink(link, target) => {
            let link_path = ctx.staging.join(link);
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if !link_path.exists() && !link_path.is_symlink() {
                std::os::unix::fs::symlink(target, &link_path)?;
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Systemd operations
        // ─────────────────────────────────────────────────────────────────
        Op::Units(names) => {
            copy_systemd_units(ctx, names)?;
        }

        Op::UserUnits(names) => {
            // Copy user-level systemd units (e.g., PipeWire)
            let src_dir = ctx.source.join("usr/lib/systemd/user");
            let dst_dir = ctx.staging.join("usr/lib/systemd/user");
            fs::create_dir_all(&dst_dir)?;

            for name in *names {
                let src = src_dir.join(name);
                let dst = dst_dir.join(name);
                if src.exists() {
                    fs::copy(&src, &dst)?;
                } else if src.is_symlink() {
                    let target = fs::read_link(&src)?;
                    if !dst.exists() {
                        std::os::unix::fs::symlink(&target, &dst)?;
                    }
                }
            }
        }

        Op::Enable(unit, target) => {
            let wants_dir = ctx.staging.join(target.wants_dir());
            fs::create_dir_all(&wants_dir)?;
            let link = wants_dir.join(unit);
            create_symlink_if_missing(
                Path::new(&format!("/usr/lib/systemd/system/{}", unit)),
                &link,
            )?;
        }

        Op::DbusSymlinks(symlinks) => {
            let unit_src = ctx.source.join("usr/lib/systemd/system");
            let unit_dst = ctx.staging.join("usr/lib/systemd/system");

            for symlink in *symlinks {
                let src = unit_src.join(symlink);
                let dst = unit_dst.join(symlink);
                if src.is_symlink() {
                    let target = fs::read_link(&src)?;
                    if !dst.exists() {
                        std::os::unix::fs::symlink(&target, &dst)?;
                    }
                }
            }
        }

        Op::UdevHelpers(helpers) => {
            // Udev helpers are part of systemd
            tracker.register_binary("systemd");

            let udev_src = ctx.source.join("usr/lib/udev");
            let udev_dst = ctx.staging.join("usr/lib/udev");
            fs::create_dir_all(&udev_dst)?;

            for helper in *helpers {
                let src = udev_src.join(helper);
                let dst = udev_dst.join(helper);
                if src.exists() && !dst.exists() {
                    fs::copy(&src, &dst)?;
                    fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // User/group operations
        // ─────────────────────────────────────────────────────────────────
        Op::User {
            name,
            uid,
            gid,
            home,
            shell,
        } => {
            users::ensure_user(&ctx.source, &ctx.staging, name, *uid, *gid, home, shell)?;
        }

        Op::Group { name, gid } => {
            users::ensure_group(&ctx.source, &ctx.staging, name, *gid)?;
        }

        // ─────────────────────────────────────────────────────────────────
        // Custom operations (dispatch to custom.rs)
        // ─────────────────────────────────────────────────────────────────
        Op::Custom(custom_op) => {
            super::custom::execute(ctx, *custom_op, tracker)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::licenses::LicenseTracker;
    use crate::component::{Component, Dest, Op, Phase, Target};
    use leviso_cheat_test::cheat_aware;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test environment with temporary directories for rootfs and initramfs.
    struct TestEnv {
        /// Temporary directory (kept alive for lifetime of TestEnv)
        _temp_dir: TempDir,
        /// Mock rootfs directory (source of binaries)
        rootfs: PathBuf,
        /// Initramfs directory (build destination)
        initramfs: PathBuf,
        /// Base directory (project root simulation)
        base_dir: PathBuf,
    }

    impl TestEnv {
        /// Create a new test environment with temporary directories.
        fn new() -> Self {
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
        fn build_context(&self) -> BuildContext {
            BuildContext::for_testing(&self.rootfs, &self.initramfs, &self.base_dir)
        }
    }

    /// Create a minimal mock rootfs with basic structure.
    fn create_mock_rootfs(rootfs: &std::path::Path) {
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
    fn assert_file_exists(path: &std::path::Path) {
        assert!(
            path.exists(),
            "Expected file to exist: {}",
            path.display()
        );
    }

    /// Assert that a file contains expected content.
    fn assert_file_contains(path: &std::path::Path, expected: &str) {
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
    fn assert_symlink(path: &std::path::Path, expected_target: &str) {
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

    // =============================================================================
    // CRITICAL: Component Executor Tests
    // =============================================================================
    //
    // These tests verify that the component executor fails loudly when binaries
    // are missing, rather than silently producing broken initramfs images.

    #[cheat_aware(
        protects = "Op::Bin fails loudly when binary not found",
        severity = "CRITICAL",
        ease = "EASY",
        cheats = [
            "Return Ok(()) when binary missing",
            "Log warning instead of error",
            "Skip missing binaries silently"
        ],
        consequence = "Initramfs boots but critical commands don't exist - system unusable"
    )]
    #[test]
    fn test_component_missing_required_binary_fails() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        // Create a component that requires a binary that doesn't exist
        let missing_binary_component = Component {
            name: "TestMissingBinary",
            phase: Phase::Binaries,
            ops: &[Op::Bin("nonexistent-binary-xyz", Dest::Bin)],
        };

        // Execute should fail because the binary doesn't exist
        let result = execute(&ctx, &missing_binary_component, &tracker);

        assert!(
            result.is_err(),
            "Op::Bin should fail when binary is not found, got Ok"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nonexistent-binary-xyz") || err_msg.contains("not found"),
            "Error should mention the missing binary name, got: {}",
            err_msg
        );
    }

    #[cheat_aware(
        protects = "Op::Bins reports ALL missing binaries, not just the first",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Stop at first missing binary",
            "Only report last missing binary",
            "Truncate list of missing binaries"
        ],
        consequence = "Developer fixes one missing binary, rebuild fails with another - wastes iteration time"
    )]
    #[test]
    fn test_component_bins_reports_all_missing() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        // Create a component that requires multiple missing binaries
        let missing_bins_component = Component {
            name: "TestMissingBins",
            phase: Phase::Binaries,
            ops: &[Op::Bins(
                &["missing-alpha", "missing-beta", "missing-gamma"],
                Dest::Bin,
            )],
        };

        let result = execute(&ctx, &missing_bins_component, &tracker);

        assert!(result.is_err(), "Op::Bins should fail when binaries missing");

        let err_msg = result.unwrap_err().to_string();

        // All three should be mentioned
        assert!(
            err_msg.contains("missing-alpha"),
            "Error should list missing-alpha, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("missing-beta"),
            "Error should list missing-beta, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("missing-gamma"),
            "Error should list missing-gamma, got: {}",
            err_msg
        );
    }

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
        let tracker = LicenseTracker::new();

        let dir_component = Component {
            name: "TestDir",
            phase: Phase::Filesystem,
            ops: &[Op::Dir("var/lib/deeply/nested/directory")],
        };

        let result = execute(&ctx, &dir_component, &tracker);
        assert!(result.is_ok(), "Op::Dir should succeed: {:?}", result);

        let created_dir = env.initramfs.join("var/lib/deeply/nested/directory");
        assert!(
            created_dir.is_dir(),
            "Directory should be created at {}",
            created_dir.display()
        );
    }

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
        let tracker = LicenseTracker::new();

        let content = "test-content-12345\nline two\n";
        let write_component = Component {
            name: "TestWriteFile",
            phase: Phase::Config,
            ops: &[Op::WriteFile("etc/test-config.conf", "test-content-12345\nline two\n")],
        };

        let result = execute(&ctx, &write_component, &tracker);
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
        let tracker = LicenseTracker::new();

        let symlink_component = Component {
            name: "TestSymlink",
            phase: Phase::Filesystem,
            ops: &[Op::Symlink("bin", "usr/bin")],
        };

        let result = execute(&ctx, &symlink_component, &tracker);
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
        protects = "Op::Enable creates systemd wants symlink",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Create symlink in wrong directory",
            "Point to wrong unit path",
            "Skip wants directory creation"
        ],
        consequence = "Services not started at boot - network, SSH, etc. don't come up"
    )]
    #[test]
    fn test_component_enable_creates_wants_symlink() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        let enable_component = Component {
            name: "TestEnable",
            phase: Phase::Services,
            ops: &[Op::Enable("test-service.service", Target::MultiUser)],
        };

        let result = execute(&ctx, &enable_component, &tracker);
        assert!(result.is_ok(), "Op::Enable should succeed: {:?}", result);

        let wants_link = env
            .initramfs
            .join("etc/systemd/system/multi-user.target.wants/test-service.service");
        assert!(
            wants_link.is_symlink(),
            "Wants symlink should exist at {}",
            wants_link.display()
        );

        let target = fs::read_link(&wants_link).expect("Should read symlink");
        assert!(
            target.to_string_lossy().contains("test-service.service"),
            "Should point to the service unit"
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
        let tracker = LicenseTracker::new();

        let write_component = Component {
            name: "TestWriteFileMode",
            phase: Phase::Config,
            ops: &[Op::WriteFileMode("etc/shadow-test", "root:!:19000::::::", 0o600)],
        };

        let result = execute(&ctx, &write_component, &tracker);
        assert!(result.is_ok(), "Op::WriteFileMode should succeed: {:?}", result);

        let file_path = env.initramfs.join("etc/shadow-test");
        assert!(file_path.exists(), "File should be created");

        let metadata = fs::metadata(&file_path).expect("Should get metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "File should have mode 0600, got {:o}",
            mode
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
        let tracker = LicenseTracker::new();

        let dir_component = Component {
            name: "TestDirMode",
            phase: Phase::Filesystem,
            ops: &[Op::DirMode("tmp", 0o1777)],
        };

        let result = execute(&ctx, &dir_component, &tracker);
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
        let tracker = LicenseTracker::new();

        let copy_component = Component {
            name: "TestCopyFileMissing",
            phase: Phase::Config,
            ops: &[Op::CopyFile("etc/nonexistent-config-file.conf")],
        };

        let result = execute(&ctx, &copy_component, &tracker);
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
        let tracker = LicenseTracker::new();

        let copyfile_component = Component {
            name: "TestCopyFileSuccess",
            phase: Phase::Config,
            ops: &[Op::CopyFile("etc/test-app.conf")],
        };

        let result = execute(&ctx, &copyfile_component, &tracker);
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
        let tracker = LicenseTracker::new();

        let copytree_component = Component {
            name: "TestCopyTree",
            phase: Phase::Config,
            ops: &[Op::CopyTree("usr/share/test-config")],
        };

        let result = execute(&ctx, &copytree_component, &tracker);
        assert!(result.is_ok(), "Op::CopyTree should succeed: {:?}", result);

        // Verify entire tree was copied
        let dst_tree = env.initramfs.join("usr/share/test-config");
        assert!(dst_tree.is_dir(), "Root directory should exist");
        assert!(dst_tree.join("subdir").is_dir(), "Subdirectory should exist");
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
        let tracker = LicenseTracker::new();

        let copytree_component = Component {
            name: "TestCopyTreeSymlink",
            phase: Phase::Config,
            ops: &[Op::CopyTree("etc/test-service")],
        };

        let result = execute(&ctx, &copytree_component, &tracker);
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
        protects = "Op::Units copies systemd unit files from rootfs",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Skip units that don't exist",
            "Create empty unit files",
            "Only copy first unit"
        ],
        consequence = "Services don't start - unit files missing"
    )]
    #[test]
    fn test_component_units_copies_unit_files() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);

        // Create mock systemd units in rootfs
        let unit_dir = env.rootfs.join("usr/lib/systemd/system");
        fs::write(
            unit_dir.join("test-service.service"),
            "[Unit]\nDescription=Test Service\n[Service]\nExecStart=/bin/true\n",
        )
        .unwrap();
        fs::write(
            unit_dir.join("test-socket.socket"),
            "[Unit]\nDescription=Test Socket\n[Socket]\nListenStream=/run/test.sock\n",
        )
        .unwrap();

        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        let units_component = Component {
            name: "TestUnits",
            phase: Phase::Systemd,
            ops: &[Op::Units(&["test-service.service", "test-socket.socket"])],
        };

        let result = execute(&ctx, &units_component, &tracker);
        assert!(result.is_ok(), "Op::Units should succeed: {:?}", result);

        // Verify units were copied
        let dst_unit_dir = env.initramfs.join("usr/lib/systemd/system");
        assert_file_exists(&dst_unit_dir.join("test-service.service"));
        assert_file_exists(&dst_unit_dir.join("test-socket.socket"));
        assert_file_contains(&dst_unit_dir.join("test-service.service"), "Test Service");
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
        let tracker = LicenseTracker::new();

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

        let result = execute(&ctx, &multi_op_component, &tracker);
        assert!(result.is_ok(), "All operations should succeed: {:?}", result);

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
        let tracker = LicenseTracker::new();

        let dirs_component = Component {
            name: "TestDirs",
            phase: Phase::Filesystem,
            ops: &[Op::Dirs(&[
                "var/lib/service-a",
                "var/lib/service-b",
                "var/lib/service-c/nested/deep",
            ])],
        };

        let result = execute(&ctx, &dirs_component, &tracker);
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
