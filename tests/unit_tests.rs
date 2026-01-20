//! Unit tests for leviso initramfs builder.
//!
//! These tests exercise pure functions in isolation without requiring
//! a real rootfs or external dependencies.

mod helpers;

use cheat_test::cheat_aware;
use helpers::{
    assert_dir_exists, assert_file_contains, assert_file_exists, assert_symlink,
    create_mock_binary, create_mock_rootfs, TestEnv,
};
use leviso::initramfs::{binary, filesystem, users};
use std::fs;
use std::os::unix::fs::PermissionsExt;

// =============================================================================
// binary.rs tests
// =============================================================================

#[cheat_aware(
    protects = "Binary dependencies are correctly identified from ldd output",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Hardcode expected libraries instead of parsing",
        "Accept partial ldd output",
        "Skip vdso filtering validation"
    ],
    consequence = "Missing shared libraries: binaries crash on load"
)]
#[test]
fn test_parse_ldd_standard_format() {
    let output = r#"
        linux-vdso.so.1 (0x00007ffee9bfe000)
        libc.so.6 => /lib64/libc.so.6 (0x00007f1234000000)
        /lib64/ld-linux-x86-64.so.2 (0x00007f1234500000)
    "#;

    let libs = binary::parse_ldd_output(output).expect("parse should succeed");

    assert!(libs.contains(&"/lib64/libc.so.6".to_string()));
    assert!(libs.contains(&"/lib64/ld-linux-x86-64.so.2".to_string()));
    // linux-vdso is virtual, should not be included
    assert!(!libs.iter().any(|l| l.contains("vdso")));
}

#[cheat_aware(
    protects = "Missing libraries are handled gracefully without crashing",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Ignore 'not found' entries silently",
        "Accept missing lib as optional",
        "Skip warning about missing deps"
    ],
    consequence = "Build succeeds but binary fails at runtime"
)]
#[test]
fn test_parse_ldd_not_found_warning() {
    let output = r#"
        libfoo.so.1 => not found
        libc.so.6 => /lib64/libc.so.6 (0x00007f1234000000)
    "#;

    // Should not fail, just warn about missing lib
    let libs = binary::parse_ldd_output(output).expect("parse should succeed");

    // Should still find libc
    assert!(libs.contains(&"/lib64/libc.so.6".to_string()));
    // Should not include "not found"
    assert!(!libs.iter().any(|l| l.contains("not found")));
}

#[cheat_aware(
    protects = "Empty ldd output handled correctly",
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
fn test_parse_ldd_empty_output() {
    let output = "";
    let libs = binary::parse_ldd_output(output).expect("parse should succeed");
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
fn test_parse_ldd_statically_linked() {
    let output = "    not a dynamic executable";
    let libs = binary::parse_ldd_output(output).expect("parse should succeed");
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
    let result = users::read_uid_from_rootfs(&env.rootfs, "dbus");
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

    let result = users::read_uid_from_rootfs(&env.rootfs, "nonexistent");
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

    let result = users::read_gid_from_rootfs(&env.rootfs, "dbus");
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
    filesystem::copy_dir_recursive(&src, &dst).expect("copy_dir_recursive should succeed");

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
