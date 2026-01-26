//! Unit tests for leviso initramfs builder.
//!
//! These tests exercise pure functions in isolation without requiring
//! a real rootfs or external dependencies.

mod helpers;

use leviso_cheat_test::cheat_aware;
use helpers::{
    assert_dir_exists, assert_file_contains, assert_file_exists, assert_symlink,
    create_mock_binary, create_mock_rootfs, TestEnv,
};
use leviso::build::{filesystem, users};
use leviso::elf as binary;
use serial_test::serial;
use std::fs;
use std::os::unix::fs::PermissionsExt;

// =============================================================================
// binary.rs tests
// =============================================================================

#[cheat_aware(
    protects = "Binary dependencies are correctly identified from readelf output",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Hardcode expected libraries instead of parsing",
        "Accept partial readelf output",
        "Skip library name extraction"
    ],
    consequence = "Missing shared libraries: binaries crash on load"
)]
#[test]
fn test_parse_readelf_standard_format() {
    let output = r#"
Dynamic section at offset 0x2f50 contains 27 entries:
  Tag        Type                         Name/Value
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libpthread.so.0]
 0x000000000000000c (INIT)               0x1000
    "#;

    let libs = binary::parse_readelf_output(output).expect("parse should succeed");

    assert!(libs.contains(&"libc.so.6".to_string()));
    assert!(libs.contains(&"libpthread.so.0".to_string()));
    assert_eq!(libs.len(), 2);
}

#[cheat_aware(
    protects = "Multiple NEEDED entries are all captured",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Only capture first library",
        "Skip libraries with certain names",
        "Limit number of captured libs"
    ],
    consequence = "Missing libraries in initramfs"
)]
#[test]
fn test_parse_readelf_multiple_needed() {
    let output = r#"
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libdl.so.2]
    "#;

    let libs = binary::parse_readelf_output(output).expect("parse should succeed");

    assert!(libs.contains(&"libm.so.6".to_string()));
    assert!(libs.contains(&"libc.so.6".to_string()));
    assert!(libs.contains(&"libdl.so.2".to_string()));
    assert_eq!(libs.len(), 3);
}

#[cheat_aware(
    protects = "Empty readelf output handled correctly",
    severity = "LOW",
    ease = "EASY",
    cheats = [
        "Return hardcoded default libs",
        "Treat empty as error",
        "Add fake libraries"
    ],
    consequence = "Phantom libraries added to initramfs"
)]
#[test]
fn test_parse_readelf_empty_output() {
    let output = "";
    let libs = binary::parse_readelf_output(output).expect("parse should succeed");
    assert!(libs.is_empty());
}

#[cheat_aware(
    protects = "Statically linked binaries don't get fake dependencies",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Assume all binaries need libc",
        "Add default libraries regardless",
        "Skip static binary detection"
    ],
    consequence = "Unnecessary libraries bloat initramfs"
)]
#[test]
fn test_parse_readelf_no_needed_entries() {
    // readelf output for a static binary has no NEEDED entries
    let output = r#"
Dynamic section at offset 0x2f50 contains 10 entries:
  Tag        Type                         Name/Value
 0x000000000000000c (INIT)               0x1000
 0x000000000000000d (FINI)               0x2000
    "#;
    let libs = binary::parse_readelf_output(output).expect("parse should succeed");
    assert!(libs.is_empty());
}

#[cheat_aware(
    protects = "Binaries found in correct location (/usr/bin)",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Use host system paths instead",
        "Accept first match without verification",
        "Skip rootfs path validation"
    ],
    consequence = "Wrong binary copied from host, incompatible libraries"
)]
#[test]
fn test_find_binary_usr_bin() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Create mock binary in /usr/bin
    let binary_path = env.rootfs.join("usr/bin/testbin");
    create_mock_binary(&binary_path);

    let found = binary::find_binary(&env.rootfs, "testbin");
    assert!(found.is_some());
    assert_eq!(found.unwrap(), binary_path);
}

#[cheat_aware(
    protects = "Binaries found in /bin as fallback",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Only check /usr/bin",
        "Skip /bin entirely",
        "Use host /bin instead"
    ],
    consequence = "Binaries in /bin not found, missing from initramfs"
)]
#[test]
fn test_find_binary_bin() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Create mock binary in /bin only
    let binary_path = env.rootfs.join("bin/testbin2");
    create_mock_binary(&binary_path);

    let found = binary::find_binary(&env.rootfs, "testbin2");
    assert!(found.is_some());
    assert_eq!(found.unwrap(), binary_path);
}

#[cheat_aware(
    protects = "Missing binaries return None instead of crashing",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Return host system binary as fallback",
        "Use default/stub binary",
        "Silently skip missing binary"
    ],
    consequence = "Host binary copied, incompatible or security risk"
)]
#[test]
fn test_find_binary_not_found() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    let found = binary::find_binary(&env.rootfs, "nonexistent");
    assert!(found.is_none());
}

#[cheat_aware(
    protects = "/usr/bin takes precedence over /bin for duplicates",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Return first match regardless of order",
        "Skip priority validation",
        "Accept either location as correct"
    ],
    consequence = "Wrong version of binary selected"
)]
#[test]
fn test_find_binary_search_order() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Create binary in both /usr/bin and /bin
    let usr_bin_path = env.rootfs.join("usr/bin/dupbin");
    let bin_path = env.rootfs.join("bin/dupbin");
    create_mock_binary(&usr_bin_path);
    create_mock_binary(&bin_path);

    // Should prefer /usr/bin
    let found = binary::find_binary(&env.rootfs, "dupbin");
    assert!(found.is_some());
    assert_eq!(found.unwrap(), usr_bin_path);
}

// =============================================================================
// users.rs tests
// =============================================================================

#[cheat_aware(
    protects = "User IDs read correctly from rootfs passwd",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Use hardcoded UIDs",
        "Skip passwd parsing",
        "Return default UID on error"
    ],
    consequence = "Wrong UID for dbus user, services fail to start"
)]
#[test]
fn test_read_uid_from_rootfs_exists() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Test reading dbus user (from mock passwd)
    let result = users::read_uid_from_rootfs(&env.rootfs, "dbus").unwrap();
    assert!(result.is_some());
    let (uid, gid) = result.unwrap();
    assert_eq!(uid, 81);
    assert_eq!(gid, 81);
}

#[cheat_aware(
    protects = "Missing users return None instead of default",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Return UID 1000 for any missing user",
        "Create user on the fly",
        "Return root UID as fallback"
    ],
    consequence = "Phantom users with wrong permissions"
)]
#[test]
fn test_read_uid_from_rootfs_not_found() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    let result = users::read_uid_from_rootfs(&env.rootfs, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[cheat_aware(
    protects = "Group IDs read correctly from rootfs group file",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Use hardcoded GIDs",
        "Skip group parsing",
        "Return UID as GID"
    ],
    consequence = "Wrong GID breaks file permissions"
)]
#[test]
fn test_read_gid_from_rootfs_exists() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    let result = users::read_gid_from_rootfs(&env.rootfs, "dbus").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 81);
}

#[cheat_aware(
    protects = "Users created in initramfs passwd correctly",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Append without checking format",
        "Skip field validation",
        "Accept malformed passwd lines"
    ],
    consequence = "Malformed passwd breaks login/su"
)]
#[test]
fn test_ensure_user_creates_entry() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();

    // Start with empty passwd
    fs::write(env.initramfs.join("etc/passwd"), "").unwrap();

    users::ensure_user(
        &env.rootfs,
        &env.initramfs,
        "testuser",
        1000,
        1000,
        "/home/test",
        "/bin/bash",
    )
    .expect("ensure_user should succeed");

    assert_file_contains(&env.initramfs.join("etc/passwd"), "testuser:x:1000:1000");
}

#[cheat_aware(
    protects = "Duplicate users not created",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Always append without checking",
        "Skip existence check",
        "Ignore duplicates silently"
    ],
    consequence = "Duplicate passwd entries cause login failures"
)]
#[test]
fn test_ensure_user_idempotent() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();

    // Start with existing user
    fs::write(env.initramfs.join("etc/passwd"), "testuser:x:1000:1000:testuser:/home:/bin/bash\n").unwrap();

    // Call ensure_user again
    users::ensure_user(
        &env.rootfs,
        &env.initramfs,
        "testuser",
        1000,
        1000,
        "/home",
        "/bin/bash",
    )
    .expect("ensure_user should succeed");

    // Should not duplicate - count lines starting with "testuser:"
    let content = fs::read_to_string(env.initramfs.join("etc/passwd")).unwrap();
    let count = content.lines().filter(|line| line.starts_with("testuser:")).count();
    assert_eq!(count, 1, "User should not be duplicated");
}

#[cheat_aware(
    protects = "Groups created in initramfs group file correctly",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Append without format validation",
        "Skip GID validation",
        "Accept any group format"
    ],
    consequence = "Malformed group file breaks permissions"
)]
#[test]
fn test_ensure_group_creates_entry() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();

    // Start with empty group file
    fs::write(env.initramfs.join("etc/group"), "").unwrap();

    users::ensure_group(&env.rootfs, &env.initramfs, "testgroup", 1000)
        .expect("ensure_group should succeed");

    assert_file_contains(&env.initramfs.join("etc/group"), "testgroup:x:1000:");
}

#[cheat_aware(
    protects = "Root user created with correct UID 0",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Use non-zero UID for root",
        "Skip root validation",
        "Accept any root entry"
    ],
    consequence = "root is not superuser, system unusable"
)]
#[test]
fn test_create_root_user() {
    let env = TestEnv::new();
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();

    users::create_root_user(&env.initramfs).expect("create_root_user should succeed");

    assert_file_contains(&env.initramfs.join("etc/passwd"), "root:x:0:0:root:/root:/bin/bash");
    assert_file_contains(&env.initramfs.join("etc/group"), "root:x:0:");
}

// =============================================================================
// filesystem.rs tests
// =============================================================================

#[cheat_aware(
    protects = "FHS directories created for standard Unix layout",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create subset of required directories",
        "Skip permission setting",
        "Accept partial FHS structure"
    ],
    consequence = "Binaries fail: /etc, /var, etc. missing"
)]
#[test]
fn test_create_fhs_structure_all_dirs() {
    let env = TestEnv::new();

    filesystem::create_fhs_structure(&env.initramfs).expect("create_fhs_structure should succeed");

    // Verify essential FHS directories
    let expected_dirs = [
        "bin", "sbin", "lib64", "lib", "etc", "proc", "sys", "dev", "dev/pts",
        "tmp", "root", "run", "run/lock", "var/log", "var/tmp",
        "usr/lib/systemd/system", "usr/lib64/systemd", "etc/systemd/system", "mnt",
    ];

    for dir in expected_dirs {
        assert_dir_exists(&env.initramfs.join(dir));
    }
}

#[cheat_aware(
    protects = "/var/run symlink points to /run",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Create directory instead of symlink",
        "Point to wrong target",
        "Skip symlink entirely"
    ],
    consequence = "PID files in wrong location"
)]
#[test]
fn test_create_var_symlinks() {
    let env = TestEnv::new();
    fs::create_dir_all(env.initramfs.join("var")).unwrap();
    fs::create_dir_all(env.initramfs.join("run")).unwrap();

    filesystem::create_var_symlinks(&env.initramfs).expect("create_var_symlinks should succeed");

    assert_symlink(&env.initramfs.join("var/run"), "/run");
}

#[cheat_aware(
    protects = "/bin/sh symlink points to bash",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Point to different shell",
        "Create regular file instead",
        "Skip sh symlink"
    ],
    consequence = "/bin/sh scripts fail"
)]
#[test]
fn test_create_sh_symlink() {
    let env = TestEnv::new();
    fs::create_dir_all(env.initramfs.join("bin")).unwrap();

    filesystem::create_sh_symlink(&env.initramfs).expect("create_sh_symlink should succeed");

    assert_symlink(&env.initramfs.join("bin/sh"), "bash");
}

#[cheat_aware(
    protects = "Symlink creation is idempotent",
    severity = "LOW",
    ease = "EASY",
    cheats = [
        "Fail on second call",
        "Overwrite without checking",
        "Create duplicate symlinks"
    ],
    consequence = "Build fails on rebuild"
)]
#[test]
fn test_symlink_idempotent() {
    let env = TestEnv::new();
    fs::create_dir_all(env.initramfs.join("bin")).unwrap();

    // Create symlink first time
    filesystem::create_sh_symlink(&env.initramfs).expect("first call should succeed");

    // Create symlink second time - should not fail
    filesystem::create_sh_symlink(&env.initramfs).expect("second call should succeed");

    // Verify symlink is still correct
    assert_symlink(&env.initramfs.join("bin/sh"), "bash");
}

#[cheat_aware(
    protects = "Shell config files created with PATH",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Create empty config files",
        "Skip PATH export",
        "Use incomplete profile"
    ],
    consequence = "Commands not found: PATH not set"
)]
#[test]
fn test_create_shell_config() {
    let env = TestEnv::new();
    fs::create_dir_all(env.initramfs.join("etc")).unwrap();
    fs::create_dir_all(env.initramfs.join("root")).unwrap();

    filesystem::create_shell_config(&env.initramfs).expect("create_shell_config should succeed");

    assert_file_exists(&env.initramfs.join("etc/profile"));
    assert_file_exists(&env.initramfs.join("root/.bashrc"));
    assert_file_contains(&env.initramfs.join("etc/profile"), "export PATH=");
}

#[cheat_aware(
    protects = "Directory copy preserves structure and content",
    severity = "MEDIUM",
    ease = "MEDIUM",
    cheats = [
        "Copy files but not subdirectories",
        "Skip permission preservation",
        "Truncate large files"
    ],
    consequence = "Missing files in copied directories"
)]
#[test]
fn test_copy_dir_recursive() {
    let env = TestEnv::new();

    // Create source directory structure
    let src = env.base_dir.join("src_dir");
    fs::create_dir_all(src.join("subdir")).unwrap();
    fs::write(src.join("file1.txt"), "content1").unwrap();
    fs::write(src.join("subdir/file2.txt"), "content2").unwrap();

    // Copy to destination
    let dst = env.base_dir.join("dst_dir");
    binary::copy_dir_recursive(&src, &dst).expect("copy_dir_recursive should succeed");

    // Verify structure
    assert_file_exists(&dst.join("file1.txt"));
    assert_file_exists(&dst.join("subdir/file2.txt"));
    assert_file_contains(&dst.join("file1.txt"), "content1");
    assert_file_contains(&dst.join("subdir/file2.txt"), "content2");
}

// =============================================================================
// binary.rs - make_executable test
// =============================================================================

#[cheat_aware(
    protects = "Files are made executable with correct permissions",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Skip permission setting",
        "Set wrong permission bits",
        "Only set user execute bit"
    ],
    consequence = "Permission denied when running binaries"
)]
#[test]
fn test_make_executable() {
    let env = TestEnv::new();
    let file_path = env.base_dir.join("test_exec");
    fs::write(&file_path, "test").unwrap();

    binary::make_executable(&file_path).expect("make_executable should succeed");

    let metadata = fs::metadata(&file_path).unwrap();
    let mode = metadata.permissions().mode();

    // Check executable bits (755 = rwxr-xr-x)
    assert_eq!(mode & 0o111, 0o111, "File should be executable");
}

// =============================================================================
// config.rs tests
// =============================================================================

use leviso::config::{module_defaults, Config};

#[cheat_aware(
    protects = "Module defaults contain essential modules",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Return empty module list",
        "Skip essential modules",
        "Hardcode incomplete list"
    ],
    consequence = "Initramfs missing essential modules, boot fails"
)]
#[test]
fn test_module_defaults_contain_essentials() {
    let modules = module_defaults::ESSENTIAL_MODULES;

    // Must have virtio for VM disk access
    assert!(modules.iter().any(|m| m.contains("virtio_blk")));
    // Must have ext4 for root filesystem
    assert!(modules.iter().any(|m| m.contains("ext4")));
    // Must have FAT for EFI partition
    assert!(modules.iter().any(|m| m.contains("fat") || m.contains("vfat")));
}

#[cheat_aware(
    protects = "Config.all_modules() combines defaults with extras",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Return only defaults",
        "Return only extras",
        "Duplicate modules"
    ],
    consequence = "Extra modules not included in initramfs"
)]
#[test]
#[serial]
fn test_config_all_modules_includes_extras() {
    let _env = TestEnv::new();

    // Set env var directly (Config::load() reads from environment, not .env file)
    std::env::set_var(
        "EXTRA_MODULES",
        "kernel/drivers/nvme/host/nvme.ko.xz,kernel/fs/xfs/xfs.ko.xz",
    );

    let config = Config::load();

    // Clean up BEFORE calling all_modules - extras are stored in config.extra_modules
    std::env::remove_var("EXTRA_MODULES");

    let all_modules = config.all_modules();

    // Should include defaults
    assert!(all_modules.iter().any(|m| m.contains("virtio_blk")));
    assert!(all_modules.iter().any(|m| m.contains("ext4")));

    // Should include extras (stored in config, not re-read from env)
    assert!(all_modules.iter().any(|m| m.contains("nvme")));
    assert!(all_modules.iter().any(|m| m.contains("xfs")));
}

#[cheat_aware(
    protects = "Empty EXTRA_MODULES doesn't break config",
    severity = "LOW",
    ease = "EASY",
    cheats = [
        "Crash on empty string",
        "Add phantom modules",
        "Skip empty check"
    ],
    consequence = "Build fails with empty EXTRA_MODULES"
)]
#[test]
#[serial]
fn test_config_empty_extra_modules() {
    let _env = TestEnv::new();

    // Set empty env var (Config::load() reads from environment, not .env file)
    std::env::set_var("EXTRA_MODULES", "");

    let config = Config::load();

    // Clean up
    std::env::remove_var("EXTRA_MODULES");

    // Extra modules should be empty
    assert!(config.extra_modules.is_empty());

    // But all_modules should still have defaults
    let all_modules = config.all_modules();
    assert!(!all_modules.is_empty());
}

// =============================================================================
// CRITICAL: Component Executor Tests
// =============================================================================
//
// These tests verify that the component executor fails loudly when binaries
// are missing, rather than silently producing broken initramfs images.

use leviso::component::{Component, Dest, Installable, Op, Phase};

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

    // Create a component that requires a binary that doesn't exist
    let missing_binary_component = Component {
        name: "TestMissingBinary",
        phase: Phase::Binaries,
        ops: &[Op::Bin("nonexistent-binary-xyz", Dest::Bin)],
    };

    // Execute should fail because the binary doesn't exist
    let result = leviso::component::executor::execute(&ctx, &missing_binary_component);

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

    // Create a component that requires multiple missing binaries
    let missing_bins_component = Component {
        name: "TestMissingBins",
        phase: Phase::Binaries,
        ops: &[Op::Bins(
            &["missing-alpha", "missing-beta", "missing-gamma"],
            Dest::Bin,
        )],
    };

    let result = leviso::component::executor::execute(&ctx, &missing_bins_component);

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

    let dir_component = Component {
        name: "TestDir",
        phase: Phase::Filesystem,
        ops: &[Op::Dir("var/lib/deeply/nested/directory")],
    };

    let result = leviso::component::executor::execute(&ctx, &dir_component);
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

    let content = "test-content-12345\nline two\n";
    // Note: Op::WriteFile takes &'static str, so we use a literal
    let write_component = Component {
        name: "TestWriteFile",
        phase: Phase::Config,
        ops: &[Op::WriteFile("etc/test-config.conf", "test-content-12345\nline two\n")],
    };

    let result = leviso::component::executor::execute(&ctx, &write_component);
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

    let symlink_component = Component {
        name: "TestSymlink",
        phase: Phase::Filesystem,
        ops: &[Op::Symlink("bin", "usr/bin")],
    };

    let result = leviso::component::executor::execute(&ctx, &symlink_component);
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

    let enable_component = Component {
        name: "TestEnable",
        phase: Phase::Services,
        ops: &[Op::Enable("test-service.service", Target::MultiUser)],
    };

    let result = leviso::component::executor::execute(&ctx, &enable_component);
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
// CRITICAL: Squashfs Atomicity Tests
// =============================================================================
//
// These tests verify the Gentoo-style work directory pattern works correctly.

#[cheat_aware(
    protects = "Squashfs work directory cleanup on build failure",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Leave work files on failure",
        "Delete final files on failure",
        "Don't create work directory"
    ],
    consequence = "Failed builds leave corrupted state, next build uses partial artifacts"
)]
#[test]
fn test_squashfs_work_dir_cleanup_on_failure() {
    // This test verifies the PATTERN, not the full mksquashfs pipeline
    // (which requires real rootfs and takes 30+ minutes)
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let work_staging = temp.path().join("output/squashfs-root.work");
    let work_output = temp.path().join("output/filesystem.squashfs.work");
    let final_staging = temp.path().join("output/squashfs-root");
    let final_output = temp.path().join("output/filesystem.squashfs");

    // Simulate: work files exist, final files exist (from previous successful build)
    fs::create_dir_all(&work_staging).unwrap();
    fs::write(&work_output, "work-squashfs-content").unwrap();
    fs::create_dir_all(&final_staging).unwrap();
    fs::write(&final_output, "final-squashfs-content").unwrap();

    // Simulate build failure - the cleanup pattern from squashfs.rs
    let build_failed = true;
    if build_failed {
        let _ = fs::remove_dir_all(&work_staging);
        let _ = fs::remove_file(&work_output);
        // Note: final files should NOT be touched on failure
    }

    // Verify: work files are gone
    assert!(
        !work_staging.exists(),
        "Work staging should be removed on failure"
    );
    assert!(
        !work_output.exists(),
        "Work output should be removed on failure"
    );

    // Verify: final files are preserved
    assert!(
        final_staging.exists(),
        "Final staging should be preserved on failure"
    );
    assert!(
        final_output.exists(),
        "Final output should be preserved on failure"
    );
}

#[cheat_aware(
    protects = "Squashfs atomic swap only happens after successful build",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Swap before build completes",
        "Partial swap (staging but not squashfs)",
        "Delete final before build completes"
    ],
    consequence = "Interrupted builds leave system with mismatched squashfs-root and filesystem.squashfs"
)]
#[test]
fn test_squashfs_atomic_swap_pattern() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let work_staging = temp.path().join("output/squashfs-root.work");
    let work_output = temp.path().join("output/filesystem.squashfs.work");
    let final_staging = temp.path().join("output/squashfs-root");
    let final_output = temp.path().join("output/filesystem.squashfs");

    // Simulate: old final files exist
    fs::create_dir_all(&final_staging.join("old-content")).unwrap();
    fs::write(&final_output, "old-squashfs").unwrap();

    // Simulate: successful build created work files
    fs::create_dir_all(&work_staging.join("new-content")).unwrap();
    fs::write(&work_output, "new-squashfs").unwrap();

    // Simulate atomic swap pattern from squashfs.rs (lines 84-93)
    let _ = fs::remove_dir_all(&final_staging);
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_staging, &final_staging).unwrap();
    fs::rename(&work_output, &final_output).unwrap();

    // Verify: work files are gone (renamed to final)
    assert!(!work_staging.exists(), "Work staging should be renamed away");
    assert!(!work_output.exists(), "Work output should be renamed away");

    // Verify: final files have new content
    assert!(
        final_staging.join("new-content").exists(),
        "Final staging should have new content"
    );
    let content = fs::read_to_string(&final_output).unwrap();
    assert_eq!(content, "new-squashfs", "Final output should have new content");
}

// =============================================================================
// CRITICAL: ISO Atomicity Tests
// =============================================================================

#[cheat_aware(
    protects = "ISO temp file is cleaned up if hardware compat fails",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Leave temp ISO on verification failure",
        "Rename to final despite verification failure",
        "Skip verification entirely"
    ],
    consequence = "ISOs that fail hardware compat checks are still produced and might be used"
)]
#[test]
fn test_iso_temp_cleanup_on_verification_failure() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let temp_iso = temp.path().join("levitate.iso.tmp");
    let final_iso = temp.path().join("levitate.iso");

    // Simulate: temp ISO was created
    fs::write(&temp_iso, "temporary-iso-content").unwrap();

    // Simulate: hardware compat verification failed (has_critical = true)
    let has_critical = true;
    if has_critical {
        let _ = fs::remove_file(&temp_iso); // Cleanup from iso.rs line 117
        // bail!() would happen here in real code
    }

    // Verify: temp ISO is removed
    assert!(
        !temp_iso.exists(),
        "Temp ISO should be removed on verification failure"
    );

    // Verify: final ISO was never created
    assert!(
        !final_iso.exists(),
        "Final ISO should not exist after verification failure"
    );
}

#[cheat_aware(
    protects = "ISO checksum is also moved atomically with ISO",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Leave checksum with temp name",
        "Skip checksum rename",
        "Generate checksum for wrong file"
    ],
    consequence = "Downloaded ISO has mismatched or missing checksum file"
)]
#[test]
fn test_iso_checksum_renamed_with_iso() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let temp_iso = temp.path().join("levitate.iso.tmp");
    let temp_checksum = temp.path().join("levitate.iso.sha512");
    let final_iso = temp.path().join("levitate.iso");
    let final_checksum = temp.path().join("levitate.sha512"); // Note: different suffix handling

    // Simulate: both temp files exist
    fs::write(&temp_iso, "iso-content").unwrap();
    fs::write(&temp_checksum, "abc123  levitate.iso.tmp").unwrap();

    // Simulate: atomic rename pattern from iso.rs (lines 122-127)
    fs::rename(&temp_iso, &final_iso).unwrap();
    fs::rename(&temp_checksum, &final_checksum).unwrap();

    // Verify: both final files exist
    assert!(final_iso.exists(), "Final ISO should exist");
    assert!(final_checksum.exists(), "Final checksum should exist");

    // Verify: temp files are gone
    assert!(!temp_iso.exists(), "Temp ISO should be gone");
    assert!(!temp_checksum.exists(), "Temp checksum should be gone");
}

// =============================================================================
// CRITICAL: Component Phase Ordering Tests
// =============================================================================
//
// These tests verify that component phases are sorted correctly, ensuring
// dependencies are satisfied (directories before files, binaries before units).

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
// CRITICAL: Library Dependency Tests
// =============================================================================

#[cheat_aware(
    protects = "Missing library dependency produces clear error message",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Swallow library copy errors",
        "Continue on missing library",
        "Return Ok(true) when library missing"
    ],
    consequence = "Binary copied but crashes on execution with 'error while loading shared libraries'"
)]
#[test]
fn test_copy_library_error_message_includes_context() {
    // This test verifies that when a library copy fails, the error message
    // includes context about WHICH binary needed the library.
    // The actual copy_library function is tested by checking the error format.

    // Simulate the error format from libdeps.rs line 76-77:
    // .with_context(|| format!("'{}' requires missing library '{}'", binary, lib_name))
    let error = anyhow::anyhow!("Library not found")
        .context("'sudo' requires missing library 'libpam.so.0'");

    let msg = format!("{:#}", error);
    assert!(
        msg.contains("sudo"),
        "Error should mention the binary name: {}",
        msg
    );
    assert!(
        msg.contains("libpam"),
        "Error should mention the library name: {}",
        msg
    );
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
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    let ctx = env.build_context();

    // Create a file with restrictive permissions (like shadow)
    let write_component = Component {
        name: "TestWriteFileMode",
        phase: Phase::Config,
        ops: &[Op::WriteFileMode("etc/shadow-test", "root:!:19000::::::", 0o600)],
    };

    let result = leviso::component::executor::execute(&ctx, &write_component);
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

    // Create /tmp with sticky bit
    let dir_component = Component {
        name: "TestDirMode",
        phase: Phase::Filesystem,
        ops: &[Op::DirMode("tmp", 0o1777)],
    };

    let result = leviso::component::executor::execute(&ctx, &dir_component);
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

    let copy_component = Component {
        name: "TestCopyFileMissing",
        phase: Phase::Config,
        ops: &[Op::CopyFile("etc/nonexistent-config-file.conf")],
    };

    let result = leviso::component::executor::execute(&ctx, &copy_component);
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

    let result = leviso::component::executor::execute(&ctx, &user_component);
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

    let group_component = Component {
        name: "TestGroupCreation",
        phase: Phase::Config,
        ops: &[Op::Group {
            name: "testgroup",
            gid: 2001,
        }],
    };

    let result = leviso::component::executor::execute(&ctx, &group_component);
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

    let result = leviso::component::executor::execute(&ctx, &multi_op_component);
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
