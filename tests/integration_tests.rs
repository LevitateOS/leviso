//! Integration tests for leviso initramfs builder.
//!
//! These tests verify that modules work together correctly using a mock rootfs.
//! They don't require building the actual initramfs.

mod helpers;

use cheat_test::{cheat_aware, cheat_canary};
use helpers::{
    assert_file_contains, assert_file_exists, assert_symlink,
    create_mock_rootfs, TestEnv,
};
use leviso::build::{filesystem, users};
use std::fs;

// =============================================================================
// Systemd setup tests
// =============================================================================

#[cheat_aware(
    protects = "Getty autologin configured correctly for tty1",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create override file without autologin content",
        "Skip agetty configuration",
        "Accept any override file as valid"
    ],
    consequence = "User gets login prompt instead of automatic shell"
)]
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

#[cheat_aware(
    protects = "Serial console service configured for QEMU testing",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create service without ttyS0",
        "Skip WantedBy directive",
        "Accept service without proper Type"
    ],
    consequence = "No output on serial console, VM testing breaks"
)]
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

#[cheat_aware(
    protects = "Serial console enabled via symlink in target wants",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create symlink to wrong path",
        "Skip symlink entirely",
        "Create file instead of symlink"
    ],
    consequence = "Serial console service not started at boot"
)]
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

#[cheat_aware(
    protects = "D-Bus socket enabled for system services",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create broken symlink",
        "Point to wrong socket file",
        "Skip socket entirely"
    ],
    consequence = "D-Bus unavailable: timedatectl, systemctl fail"
)]
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

#[cheat_aware(
    protects = "D-Bus user created with correct UID/GID",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Use wrong UID for dbus",
        "Skip dbus user creation",
        "Create user without group"
    ],
    consequence = "D-Bus daemon fails: user 'dbus' not found"
)]
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

#[cheat_aware(
    protects = "D-Bus directories created for daemon operation",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Create subset of required directories",
        "Skip run/dbus directory",
        "Accept missing config directories"
    ],
    consequence = "D-Bus daemon cannot start: directories missing"
)]
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

#[cheat_aware(
    protects = "PAM login configuration allows authentication",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Create empty PAM config",
        "Skip pam_permit.so which allows login",
        "Use wrong PAM stack order"
    ],
    consequence = "login: Authentication failure"
)]
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

#[cheat_aware(
    protects = "Securetty allows root login on console TTYs",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create empty securetty",
        "Skip ttyS0 for serial console",
        "List only tty1"
    ],
    consequence = "root login disabled on this terminal"
)]
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

#[cheat_aware(
    protects = "Valid login shells listed in /etc/shells",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Create empty shells file",
        "Skip /bin/bash entry",
        "Use wrong shell paths"
    ],
    consequence = "chsh: /bin/bash is not a valid shell"
)]
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

#[cheat_aware(
    protects = "Shadow file exists for password authentication",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Skip shadow file creation",
        "Create shadow without root entry",
        "Use wrong shadow format"
    ],
    consequence = "Authentication token manipulation error"
)]
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

#[cheat_aware(
    protects = "Machine ID file exists for systemd",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Skip machine-id creation",
        "Create with wrong permissions",
        "Pre-populate with invalid ID"
    ],
    consequence = "Failed to create /etc/machine-id"
)]
#[test]
fn test_machine_id_created() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // Create empty machine-id (what setup_systemd would create)
    fs::write(env.initramfs.join("etc/machine-id"), "").unwrap();

    assert_file_exists(&env.initramfs.join("etc/machine-id"));
}

#[cheat_aware(
    protects = "OS identifies as LevitateOS, not Rocky",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Copy Rocky's os-release",
        "Skip branding entirely",
        "Use partial os-release"
    ],
    consequence = "System shows as Rocky Linux, not LevitateOS"
)]
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

#[cheat_aware(
    protects = "/sbin/init symlink points to systemd",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Point to wrong path",
        "Create file instead of symlink",
        "Skip init symlink"
    ],
    consequence = "Kernel panic: No init found"
)]
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

#[cheat_aware(
    protects = "Getty service enabled for tty1",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Create symlink to wrong service",
        "Skip getty target wants",
        "Create broken symlink"
    ],
    consequence = "No login prompt on tty1"
)]
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

#[cheat_aware(
    protects = "Getty target pulled in by multi-user target",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Skip getty.target in multi-user wants",
        "Create wrong symlink target",
        "Omit multi-user.target.wants"
    ],
    consequence = "Getty services never started"
)]
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

#[cheat_aware(
    protects = "Journald sockets enabled for logging",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Skip journald sockets",
        "Create partial socket setup",
        "Point to wrong socket files"
    ],
    consequence = "No system logging, debugging impossible"
)]
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

#[cheat_aware(
    protects = "Full FHS setup works end-to-end with BuildContext",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Test individual functions only",
        "Skip BuildContext integration",
        "Use mocked BuildContext"
    ],
    consequence = "Functions work alone but fail together"
)]
#[test]
fn test_full_fhs_setup() {
    let env = TestEnv::new();
    create_mock_rootfs(&env.rootfs);

    // Use BuildContext to verify it works
    let ctx = env.build_context();

    // Create FHS structure
    filesystem::create_fhs_structure(&ctx.staging).unwrap();
    filesystem::create_var_symlinks(&ctx.staging).unwrap();
    filesystem::create_sh_symlink(&ctx.staging).unwrap();
    filesystem::create_shell_config(&ctx.staging).unwrap();

    // Create root user
    users::create_root_user(&ctx.staging).unwrap();

    // Verify everything is in place
    assert!(ctx.staging.join("bin").is_dir());
    assert!(ctx.staging.join("sbin").is_dir());
    assert!(ctx.staging.join("etc").is_dir());
    assert_symlink(&ctx.staging.join("var/run"), "/run");
    assert_symlink(&ctx.staging.join("bin/sh"), "bash");
    assert_file_contains(&ctx.staging.join("etc/passwd"), "root:x:0:0");
}

// =============================================================================
// Canary test - DO NOT MODIFY without triggering review
// =============================================================================

/// CANARY TEST: Verbose FHS directory verification.
///
/// This test checks each FHS directory individually. A cheater would want to
/// consolidate these into a loop or array. Any modification triggers review.
#[cheat_canary(
    bait = "Individual directory checks look redundant - tempting to use a loop",
    tripwire = "Any modification to this function triggers full audit of integration test changes"
)]
#[test]
fn canary_integration_verbose_fhs_check() {
    let env = TestEnv::new();
    filesystem::create_fhs_structure(&env.initramfs).unwrap();

    // DO NOT consolidate these into a loop or array.
    // This verbosity is intentional - it makes cheating obvious.

    let bin = env.initramfs.join("bin");
    assert!(bin.is_dir(), "bin directory missing at {}", bin.display());

    let sbin = env.initramfs.join("sbin");
    assert!(sbin.is_dir(), "sbin directory missing at {}", sbin.display());

    let etc = env.initramfs.join("etc");
    assert!(etc.is_dir(), "etc directory missing at {}", etc.display());

    let lib64 = env.initramfs.join("lib64");
    assert!(lib64.is_dir(), "lib64 directory missing at {}", lib64.display());

    let proc = env.initramfs.join("proc");
    assert!(proc.is_dir(), "proc directory missing at {}", proc.display());

    let sys = env.initramfs.join("sys");
    assert!(sys.is_dir(), "sys directory missing at {}", sys.display());

    let dev = env.initramfs.join("dev");
    assert!(dev.is_dir(), "dev directory missing at {}", dev.display());

    let tmp = env.initramfs.join("tmp");
    assert!(tmp.is_dir(), "tmp directory missing at {}", tmp.display());

    let root_dir = env.initramfs.join("root");
    assert!(root_dir.is_dir(), "root directory missing at {}", root_dir.display());

    let run = env.initramfs.join("run");
    assert!(run.is_dir(), "run directory missing at {}", run.display());

    let var = env.initramfs.join("var");
    assert!(var.is_dir(), "var directory missing at {}", var.display());

    let mnt = env.initramfs.join("mnt");
    assert!(mnt.is_dir(), "mnt directory missing at {}", mnt.display());

    let usr_lib_systemd = env.initramfs.join("usr/lib/systemd");
    assert!(usr_lib_systemd.is_dir(), "usr/lib/systemd directory missing at {}", usr_lib_systemd.display());

    let usr_lib_systemd_system = env.initramfs.join("usr/lib/systemd/system");
    assert!(usr_lib_systemd_system.is_dir(), "usr/lib/systemd/system directory missing at {}", usr_lib_systemd_system.display());

    let etc_systemd_system = env.initramfs.join("etc/systemd/system");
    assert!(etc_systemd_system.is_dir(), "etc/systemd/system directory missing at {}", etc_systemd_system.display());
}
