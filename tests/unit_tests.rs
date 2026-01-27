//! Integration tests for leviso component executor.
//!
//! These tests exercise the component executor which orchestrates
//! multiple operations. Pure unit tests for individual functions
//! are in their respective source files (e.g., build/users.rs).
//!
//! See `leviso/tests/README.md` for what tests belong where.

mod helpers;

use leviso_cheat_test::cheat_aware;
use helpers::{
    assert_file_contains, assert_file_exists, assert_symlink,
    create_mock_rootfs, TestEnv,
};
use leviso::build::licenses::LicenseTracker;
use leviso::component::{Component, Dest, Op, Phase};
use std::fs;

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
    let tracker = LicenseTracker::new();

    // Create a component that requires a binary that doesn't exist
    let missing_binary_component = Component {
        name: "TestMissingBinary",
        phase: Phase::Binaries,
        ops: &[Op::Bin("nonexistent-binary-xyz", Dest::Bin)],
    };

    // Execute should fail because the binary doesn't exist
    let result = leviso::component::executor::execute(&ctx, &missing_binary_component, &tracker);

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

    let result = leviso::component::executor::execute(&ctx, &missing_bins_component, &tracker);

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
    let tracker = LicenseTracker::new();

    let dir_component = Component {
        name: "TestDir",
        phase: Phase::Filesystem,
        ops: &[Op::Dir("var/lib/deeply/nested/directory")],
    };

    let result = leviso::component::executor::execute(&ctx, &dir_component, &tracker);
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
    let tracker = LicenseTracker::new();

    let content = "test-content-12345\nline two\n";
    let write_component = Component {
        name: "TestWriteFile",
        phase: Phase::Config,
        ops: &[Op::WriteFile("etc/test-config.conf", "test-content-12345\nline two\n")],
    };

    let result = leviso::component::executor::execute(&ctx, &write_component, &tracker);
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
    let tracker = LicenseTracker::new();

    let symlink_component = Component {
        name: "TestSymlink",
        phase: Phase::Filesystem,
        ops: &[Op::Symlink("bin", "usr/bin")],
    };

    let result = leviso::component::executor::execute(&ctx, &symlink_component, &tracker);
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
    use leviso::component::Target;

    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let enable_component = Component {
        name: "TestEnable",
        phase: Phase::Services,
        ops: &[Op::Enable("test-service.service", Target::MultiUser)],
    };

    let result = leviso::component::executor::execute(&ctx, &enable_component, &tracker);
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

// =============================================================================
// CRITICAL: Component Phase Ordering Tests
// =============================================================================

#[cheat_aware(
    protects = "Component phases are correctly ordered",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Remove Ord implementation from Phase",
        "Hardcode phase order incorrectly",
        "Skip sorting entirely"
    ],
    consequence = "Components execute out of order - files copied before directories exist"
)]
#[test]
fn test_phase_ordering_is_correct() {
    use leviso::component::Phase;

    // Phase ordering must be: Filesystem < Binaries < Systemd < Dbus < Services < Config < Packages < Firmware < Final
    assert!(
        Phase::Filesystem < Phase::Binaries,
        "Filesystem must come before Binaries"
    );
    assert!(
        Phase::Binaries < Phase::Systemd,
        "Binaries must come before Systemd"
    );
    assert!(
        Phase::Systemd < Phase::Dbus,
        "Systemd must come before Dbus"
    );
    assert!(
        Phase::Dbus < Phase::Services,
        "Dbus must come before Services"
    );
    assert!(
        Phase::Services < Phase::Config,
        "Services must come before Config"
    );
    assert!(
        Phase::Config < Phase::Packages,
        "Config must come before Packages"
    );
    assert!(
        Phase::Packages < Phase::Firmware,
        "Packages must come before Firmware"
    );
    assert!(
        Phase::Firmware < Phase::Final,
        "Firmware must come before Final"
    );
}

#[cheat_aware(
    protects = "Filesystem phase creates directories before other phases need them",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Skip directory creation",
        "Create directories in wrong phase",
        "Assume directories exist"
    ],
    consequence = "File copies fail with 'No such file or directory' during build"
)]
#[test]
fn test_filesystem_phase_is_first() {
    use leviso::component::Phase;

    // Filesystem must be the absolute first phase
    let phases = [
        Phase::Binaries,
        Phase::Systemd,
        Phase::Dbus,
        Phase::Services,
        Phase::Config,
        Phase::Packages,
        Phase::Firmware,
        Phase::Final,
    ];

    for phase in phases {
        assert!(
            Phase::Filesystem < phase,
            "Filesystem must come before {:?}",
            phase
        );
    }
}

// =============================================================================
// CRITICAL: WriteFile Mode Tests
// =============================================================================

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
    use std::os::unix::fs::PermissionsExt;

    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let write_component = Component {
        name: "TestWriteFileMode",
        phase: Phase::Config,
        ops: &[Op::WriteFileMode("etc/shadow-test", "root:!:19000::::::", 0o600)],
    };

    let result = leviso::component::executor::execute(&ctx, &write_component, &tracker);
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
    use std::os::unix::fs::PermissionsExt;

    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let dir_component = Component {
        name: "TestDirMode",
        phase: Phase::Filesystem,
        ops: &[Op::DirMode("tmp", 0o1777)],
    };

    let result = leviso::component::executor::execute(&ctx, &dir_component, &tracker);
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

// =============================================================================
// CRITICAL: CopyFile Error Handling Tests
// =============================================================================

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
    let tracker = LicenseTracker::new();

    let copy_component = Component {
        name: "TestCopyFileMissing",
        phase: Phase::Config,
        ops: &[Op::CopyFile("etc/nonexistent-config-file.conf")],
    };

    let result = leviso::component::executor::execute(&ctx, &copy_component, &tracker);
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

// =============================================================================
// CRITICAL: User/Group Creation Tests
// =============================================================================

#[cheat_aware(
    protects = "Op::User creates user with correct UID/GID",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Assign wrong UID",
        "Skip group assignment",
        "Ignore home directory"
    ],
    consequence = "Services run as wrong user, file ownership broken, security issues"
)]
#[test]
fn test_component_user_creates_entry() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Create etc directory in staging with base passwd file
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();
    fs::write(
        env.initramfs.join("etc/passwd"),
        "root:x:0:0:root:/root:/bin/bash\n",
    ).unwrap();

    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let user_component = Component {
        name: "TestUserCreation",
        phase: Phase::Config,
        ops: &[Op::User {
            name: "testuser",
            uid: 1001,
            gid: 1001,
            home: "/home/testuser",
            shell: "/bin/bash",
        }],
    };

    let result = leviso::component::executor::execute(&ctx, &user_component, &tracker);
    assert!(result.is_ok(), "Op::User should succeed: {:?}", result);

    let passwd_path = env.initramfs.join("etc/passwd");
    let passwd_content = fs::read_to_string(&passwd_path).expect("Should read passwd");

    assert!(
        passwd_content.contains("testuser"),
        "passwd should contain testuser"
    );
    assert!(
        passwd_content.contains("1001"),
        "passwd should contain UID 1001"
    );
    assert!(
        passwd_content.contains("/home/testuser"),
        "passwd should contain home directory"
    );
}

#[cheat_aware(
    protects = "Op::Group creates group with correct GID",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Assign wrong GID",
        "Skip group creation",
        "Create duplicate group"
    ],
    consequence = "Services can't find their groups, permission errors"
)]
#[test]
fn test_component_group_creates_entry() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Create etc directory in staging with base group file
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();
    fs::write(
        env.initramfs.join("etc/group"),
        "root:x:0:\n",
    ).unwrap();

    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let group_component = Component {
        name: "TestGroupCreation",
        phase: Phase::Config,
        ops: &[Op::Group {
            name: "testgroup",
            gid: 2001,
        }],
    };

    let result = leviso::component::executor::execute(&ctx, &group_component, &tracker);
    assert!(result.is_ok(), "Op::Group should succeed: {:?}", result);

    let group_path = env.initramfs.join("etc/group");
    let group_content = fs::read_to_string(&group_path).expect("Should read group");

    assert!(
        group_content.contains("testgroup"),
        "group file should contain testgroup"
    );
    assert!(
        group_content.contains("2001"),
        "group file should contain GID 2001"
    );
}

// =============================================================================
// CRITICAL: Multiple Operations Execution Tests
// =============================================================================

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

    let result = leviso::component::executor::execute(&ctx, &multi_op_component, &tracker);
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

// =============================================================================
// CRITICAL: Op::Dirs Tests
// =============================================================================

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

    let result = leviso::component::executor::execute(&ctx, &dirs_component, &tracker);
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

// =============================================================================
// CRITICAL: Op::CopyTree Tests
// =============================================================================

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
    let tracker = LicenseTracker::new();

    let copytree_component = Component {
        name: "TestCopyTree",
        phase: Phase::Config,
        ops: &[Op::CopyTree("usr/share/test-config")],
    };

    let result = leviso::component::executor::execute(&ctx, &copytree_component, &tracker);
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
    let tracker = LicenseTracker::new();

    let copytree_component = Component {
        name: "TestCopyTreeSymlink",
        phase: Phase::Config,
        ops: &[Op::CopyTree("etc/test-service")],
    };

    let result = leviso::component::executor::execute(&ctx, &copytree_component, &tracker);
    assert!(result.is_ok(), "Op::CopyTree should succeed: {:?}", result);

    // Verify symlink was preserved
    let dst_link = env.initramfs.join("etc/test-service/link.conf");
    assert!(
        dst_link.is_symlink(),
        "Symlink should be preserved, not converted to file"
    );
    assert_symlink(&dst_link, "real.conf");
}

// =============================================================================
// CRITICAL: Op::Units Tests
// =============================================================================

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
    ).unwrap();
    fs::write(
        unit_dir.join("test-socket.socket"),
        "[Unit]\nDescription=Test Socket\n[Socket]\nListenStream=/run/test.sock\n",
    ).unwrap();

    let ctx = env.build_context();
    let tracker = LicenseTracker::new();
    let tracker = LicenseTracker::new();

    let units_component = Component {
        name: "TestUnits",
        phase: Phase::Systemd,
        ops: &[Op::Units(&["test-service.service", "test-socket.socket"])],
    };

    let result = leviso::component::executor::execute(&ctx, &units_component, &tracker);
    assert!(result.is_ok(), "Op::Units should succeed: {:?}", result);

    // Verify units were copied
    let dst_unit_dir = env.initramfs.join("usr/lib/systemd/system");
    assert_file_exists(&dst_unit_dir.join("test-service.service"));
    assert_file_exists(&dst_unit_dir.join("test-socket.socket"));
    assert_file_contains(&dst_unit_dir.join("test-service.service"), "Test Service");
}

// =============================================================================
// CRITICAL: CopyFile Success Tests
// =============================================================================

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
    let tracker = LicenseTracker::new();

    let copyfile_component = Component {
        name: "TestCopyFileSuccess",
        phase: Phase::Config,
        ops: &[Op::CopyFile("etc/test-app.conf")],
    };

    let result = leviso::component::executor::execute(&ctx, &copyfile_component, &tracker);
    assert!(result.is_ok(), "Op::CopyFile should succeed: {:?}", result);

    // Verify file was copied with correct content
    let dst_config = env.initramfs.join("etc/test-app.conf");
    assert_file_exists(&dst_config);
    assert_file_contains(&dst_config, "setting=value");
    assert_file_contains(&dst_config, "other=123");
}
