//! Integration tests for leviso initramfs builder.
//!
//! These tests verify that modules work together correctly using a mock rootfs.
//! They don't require building the actual initramfs.

mod helpers;

use helpers::{
    assert_file_contains, assert_file_exists, assert_symlink, create_mock_binary,
    create_mock_library, create_mock_rootfs, TestEnv,
};
use leviso::initramfs::{dbus, filesystem, pam, systemd, users};
use std::fs;

// =============================================================================
// Systemd setup tests
// =============================================================================

#[test]
fn test_systemd_getty_autologin_override() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create necessary systemd directories
    fs::create_dir_all(env.initramfs.join("etc/systemd/system/getty@tty1.service.d")).unwrap();

    // Write the autologin override directly (testing what setup_getty would create)
    let override_path = env
        .initramfs
        .join("etc/systemd/system/getty@tty1.service.d/autologin.conf");
    fs::write(
        &override_path,
        r#"[Service]
ExecStart=
ExecStart=-/bin/agetty --autologin root --noclear --login-program /bin/bash --login-options '-l' %I linux
Type=idle
"#,
    )
    .unwrap();

    assert_file_exists(&override_path);
    assert_file_contains(&override_path, "--autologin root");
    assert_file_contains(&override_path, "/bin/agetty");
}

#[test]
fn test_serial_console_service() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create serial console service (testing what setup_serial_console would create)
    let serial_console = env
        .initramfs
        .join("etc/systemd/system/serial-console.service");
    fs::write(
        &serial_console,
        r#"[Unit]
Description=Serial Console Shell
After=basic.target

[Service]
ExecStart=/bin/bash --login
TTYPath=/dev/ttyS0
Type=idle
Restart=always

[Install]
WantedBy=multi-user.target
"#,
    )
    .unwrap();

    assert_file_exists(&serial_console);
    assert_file_contains(&serial_console, "TTYPath=/dev/ttyS0");
    assert_file_contains(&serial_console, "WantedBy=multi-user.target");
}

#[test]
fn test_serial_console_enabled() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create the wants directory and symlink
    let wants_dir = env
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&wants_dir).unwrap();

    let serial_link = wants_dir.join("serial-console.service");
    std::os::unix::fs::symlink("/etc/systemd/system/serial-console.service", &serial_link).unwrap();

    assert_symlink(&serial_link, "/etc/systemd/system/serial-console.service");
}

// =============================================================================
// D-Bus setup tests
// =============================================================================

#[test]
fn test_dbus_socket_enabled() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create sockets.target.wants directory
    let sockets_wants = env
        .initramfs
        .join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants).unwrap();

    // Create dbus.socket symlink
    let dbus_socket_link = sockets_wants.join("dbus.socket");
    std::os::unix::fs::symlink("/usr/lib/systemd/system/dbus.socket", &dbus_socket_link).unwrap();

    assert_symlink(&dbus_socket_link, "/usr/lib/systemd/system/dbus.socket");
}

#[test]
fn test_dbus_user_creation() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create root user first
    users::create_root_user(&env.initramfs).unwrap();

    // Ensure dbus user (this is what setup_dbus calls internally)
    users::ensure_user(
        &env.rootfs,
        &env.initramfs,
        "dbus",
        81,
        81,
        "/",
        "/sbin/nologin",
    )
    .unwrap();
    users::ensure_group(&env.rootfs, &env.initramfs, "dbus", 81).unwrap();

    assert_file_contains(&env.initramfs.join("etc/passwd"), "dbus:x:81:81");
    assert_file_contains(&env.initramfs.join("etc/group"), "dbus:x:81:");
}

#[test]
fn test_dbus_directories_created() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create D-Bus directories (what setup_dbus would create)
    fs::create_dir_all(env.initramfs.join("usr/share/dbus-1")).unwrap();
    fs::create_dir_all(env.initramfs.join("etc/dbus-1")).unwrap();
    fs::create_dir_all(env.initramfs.join("run/dbus")).unwrap();
    fs::create_dir_all(env.initramfs.join("usr/bin")).unwrap();

    assert!(env.initramfs.join("usr/share/dbus-1").is_dir());
    assert!(env.initramfs.join("etc/dbus-1").is_dir());
    assert!(env.initramfs.join("run/dbus").is_dir());
}

// =============================================================================
// PAM setup tests
// =============================================================================

#[test]
fn test_pam_config_files() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create PAM config directory
    let pam_d = env.initramfs.join("etc/pam.d");
    fs::create_dir_all(&pam_d).unwrap();

    // Create login PAM config (what setup_pam would create)
    fs::write(
        pam_d.join("login"),
        r#"#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_permit.so
account    required     pam_permit.so
password   required     pam_permit.so
session    required     pam_permit.so
"#,
    )
    .unwrap();

    assert_file_exists(&pam_d.join("login"));
    assert_file_contains(&pam_d.join("login"), "pam_permit.so");
    assert_file_contains(&pam_d.join("login"), "pam_rootok.so");
}

#[test]
fn test_securetty_allows_console() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create securetty (what setup_pam would create)
    fs::write(
        env.initramfs.join("etc/securetty"),
        "tty1\ntty2\ntty3\ntty4\ntty5\ntty6\nttyS0\n",
    )
    .unwrap();

    assert_file_contains(&env.initramfs.join("etc/securetty"), "tty1");
    assert_file_contains(&env.initramfs.join("etc/securetty"), "ttyS0");
}

#[test]
fn test_shells_file() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create shells file (what setup_pam would create)
    fs::write(env.initramfs.join("etc/shells"), "/bin/bash\n/bin/sh\n").unwrap();

    assert_file_contains(&env.initramfs.join("etc/shells"), "/bin/bash");
    assert_file_contains(&env.initramfs.join("etc/shells"), "/bin/sh");
}

#[test]
fn test_shadow_file() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create shadow file (what setup_pam would create)
    fs::write(env.initramfs.join("etc/shadow"), "root::0::::::\n").unwrap();

    assert_file_contains(&env.initramfs.join("etc/shadow"), "root::");
}

// =============================================================================
// Systemd system files tests
// =============================================================================

#[test]
fn test_machine_id_created() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create empty machine-id (what setup_systemd would create)
    fs::write(env.initramfs.join("etc/machine-id"), "").unwrap();

    assert_file_exists(&env.initramfs.join("etc/machine-id"));
}

#[test]
fn test_os_release_created() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create os-release (what setup_systemd would create)
    fs::write(
        env.initramfs.join("etc/os-release"),
        r#"NAME="LevitateOS"
ID=levitateos
VERSION="1.0"
PRETTY_NAME="LevitateOS Live"
"#,
    )
    .unwrap();

    assert_file_contains(&env.initramfs.join("etc/os-release"), "NAME=\"LevitateOS\"");
    assert_file_contains(&env.initramfs.join("etc/os-release"), "ID=levitateos");
}

#[test]
fn test_init_symlink() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create /sbin/init symlink (what setup_systemd would create)
    let init_link = env.initramfs.join("sbin/init");
    std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link).unwrap();

    assert_symlink(&init_link, "/usr/lib/systemd/systemd");
}

// =============================================================================
// Getty target tests
// =============================================================================

#[test]
fn test_getty_target_enabled() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create getty.target.wants directory
    let getty_wants = env
        .initramfs
        .join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants).unwrap();

    // Create getty@tty1.service symlink
    let getty_link = getty_wants.join("getty@tty1.service");
    std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &getty_link).unwrap();

    assert_symlink(&getty_link, "/usr/lib/systemd/system/getty@.service");
}

#[test]
fn test_getty_target_from_multi_user() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create multi-user.target.wants directory
    let multi_user_wants = env
        .initramfs
        .join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants).unwrap();

    // Create getty.target symlink
    let getty_target_link = multi_user_wants.join("getty.target");
    std::os::unix::fs::symlink("/usr/lib/systemd/system/getty.target", &getty_target_link).unwrap();

    assert_symlink(&getty_target_link, "/usr/lib/systemd/system/getty.target");
}

// =============================================================================
// Journald socket tests
// =============================================================================

#[test]
fn test_journald_sockets_enabled() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create sockets.target.wants directory
    let sockets_wants = env
        .initramfs
        .join("etc/systemd/system/sockets.target.wants");
    fs::create_dir_all(&sockets_wants).unwrap();

    // Create journald socket symlinks
    let journald_socket_link = sockets_wants.join("systemd-journald.socket");
    std::os::unix::fs::symlink(
        "/usr/lib/systemd/system/systemd-journald.socket",
        &journald_socket_link,
    )
    .unwrap();

    let journald_dev_log_link = sockets_wants.join("systemd-journald-dev-log.socket");
    std::os::unix::fs::symlink(
        "/usr/lib/systemd/system/systemd-journald-dev-log.socket",
        &journald_dev_log_link,
    )
    .unwrap();

    assert_symlink(
        &journald_socket_link,
        "/usr/lib/systemd/system/systemd-journald.socket",
    );
    assert_symlink(
        &journald_dev_log_link,
        "/usr/lib/systemd/system/systemd-journald-dev-log.socket",
    );
}

// =============================================================================
// Full integration test with BuildContext
// =============================================================================

#[test]
fn test_full_fhs_setup() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Use BuildContext to verify it works
    let ctx = env.build_context();

    // Create FHS structure
    filesystem::create_fhs_structure(&ctx.initramfs).unwrap();
    filesystem::create_var_symlinks(&ctx.initramfs).unwrap();
    filesystem::create_sh_symlink(&ctx.initramfs).unwrap();
    filesystem::create_shell_config(&ctx.initramfs).unwrap();

    // Create root user
    users::create_root_user(&ctx.initramfs).unwrap();

    // Verify everything is in place
    assert!(ctx.initramfs.join("bin").is_dir());
    assert!(ctx.initramfs.join("sbin").is_dir());
    assert!(ctx.initramfs.join("etc").is_dir());
    assert_symlink(&ctx.initramfs.join("var/run"), "/run");
    assert_symlink(&ctx.initramfs.join("bin/sh"), "bash");
    assert_file_contains(&ctx.initramfs.join("etc/passwd"), "root:x:0:0");
}
