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
use leviso::initramfs_depr::{binary, filesystem, users};
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

// =============================================================================
// config.rs tests
// =============================================================================

use leviso::config::{module_defaults, Config, RockyConfig};

#[cheat_aware(
    protects = "Rocky config uses correct defaults when no env vars set",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Hardcode wrong defaults",
        "Skip env var checking",
        "Use stale cached values"
    ],
    consequence = "Wrong Rocky version downloaded"
)]
#[test]
fn test_rocky_config_defaults() {
    let config = RockyConfig::default();

    assert_eq!(config.version, "10.1");
    assert_eq!(config.arch, "x86_64");
    assert!(config.url.contains("Rocky-10.1"));
    assert!(config.filename.contains("Rocky-10.1"));
    assert!(!config.sha256.is_empty());
}

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
fn test_config_all_modules_includes_extras() {
    let env = TestEnv::new();

    // Write .env with extra modules
    fs::write(
        env.base_dir.join(".env"),
        "EXTRA_MODULES=kernel/drivers/nvme/host/nvme.ko.xz,kernel/fs/xfs/xfs.ko.xz",
    )
    .unwrap();

    let config = Config::load(&env.base_dir);
    let all_modules = config.all_modules();

    // Should include defaults
    assert!(all_modules.iter().any(|m| m.contains("virtio_blk")));
    assert!(all_modules.iter().any(|m| m.contains("ext4")));

    // Should include extras
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
fn test_config_empty_extra_modules() {
    let env = TestEnv::new();

    // Write .env with empty extra modules
    fs::write(env.base_dir.join(".env"), "EXTRA_MODULES=").unwrap();

    let config = Config::load(&env.base_dir);

    // Extra modules should be empty
    assert!(config.extra_modules.is_empty());

    // But all_modules should still have defaults
    let all_modules = config.all_modules();
    assert!(!all_modules.is_empty());
}
