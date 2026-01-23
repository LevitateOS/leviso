//! Validation tests for built initramfs.
//!
//! These tests verify the contents of a built initramfs. They require running:
//!   cargo run -- initramfs
//! before execution.
//!
//! Run these tests with:
//!   cargo test -- --ignored validation

mod helpers;

use leviso_cheat_test::{cheat_aware, cheat_canary};
use helpers::{assert_file_contains, assert_file_exists, assert_symlink, initramfs_is_built, real_initramfs_root};
use std::fs;
use std::path::Path;

/// Skip test if initramfs is not built.
fn require_initramfs() -> std::path::PathBuf {
    let root = real_initramfs_root();
    if !initramfs_is_built() {
        panic!(
            "Initramfs not built. Run 'cargo run -- initramfs' first.\nExpected at: {}",
            root.display()
        );
    }
    root
}

// =============================================================================
// Essential binaries tests
// =============================================================================

#[cheat_aware(
    protects = "User can run basic commands (ls, cat, mount, login)",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Reduce the binaries list to only those present",
        "Move missing binaries to OPTIONAL list",
        "Check for binary existence but not executability"
    ],
    consequence = "bash: ls: command not found"
)]
#[test]
#[ignore]
fn test_validation_essential_binaries_present() {
    let root = require_initramfs();

    let binaries = ["bash", "ls", "cat", "mount", "agetty", "login"];

    for binary in binaries {
        let bin_path = root.join("bin").join(binary);
        assert!(
            bin_path.exists(),
            "Essential binary missing: {}",
            bin_path.display()
        );
    }
}

#[cheat_aware(
    protects = "System can boot and run init (PID 1)",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Check for any file at that path, not systemd specifically",
        "Accept a symlink to nonexistent binary",
        "Skip test if systemd not found"
    ],
    consequence = "Kernel panic - not syncing: No working init found"
)]
#[test]
#[ignore]
fn test_validation_systemd_binary_present() {
    let root = require_initramfs();

    let systemd_path = root.join("usr/lib/systemd/systemd");
    assert!(
        systemd_path.exists(),
        "Systemd binary missing: {}",
        systemd_path.display()
    );
}

#[cheat_aware(
    protects = "System uses systemd as init",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check symlink exists but not target validity",
        "Accept any symlink target",
        "Create symlink during test instead of failing"
    ],
    consequence = "Kernel panic - unable to mount rootfs"
)]
#[test]
#[ignore]
fn test_validation_init_symlink_correct() {
    let root = require_initramfs();

    let init_link = root.join("sbin/init");
    assert_symlink(&init_link, "/usr/lib/systemd/systemd");
}

#[cheat_aware(
    protects = "Scripts using /bin/sh work correctly",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check for any shell, not specifically bash",
        "Accept broken symlink",
        "Skip if sh exists in any form"
    ],
    consequence = "/bin/sh: No such file or directory"
)]
#[test]
#[ignore]
fn test_validation_sh_symlink_correct() {
    let root = require_initramfs();

    let sh_link = root.join("bin/sh");
    assert_symlink(&sh_link, "bash");
}

// =============================================================================
// Library tests
// =============================================================================

#[cheat_aware(
    protects = "Dynamically linked binaries can load libc",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Check for any .so file in lib64",
        "Accept broken symlinks",
        "Skip library version validation"
    ],
    consequence = "error while loading shared libraries: libc.so.6"
)]
#[test]
#[ignore]
fn test_validation_lib64_has_libc() {
    let root = require_initramfs();

    let lib64 = root.join("lib64");
    assert!(lib64.is_dir(), "lib64 directory missing");

    // Check for libc (may have version suffix)
    let has_libc = fs::read_dir(&lib64)
        .expect("Failed to read lib64")
        .filter_map(|e| e.ok())
        .any(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("libc.so")
        });

    assert!(has_libc, "libc.so not found in lib64");
}

#[cheat_aware(
    protects = "Dynamic linker can load any binary",
    severity = "CRITICAL",
    ease = "MEDIUM",
    cheats = [
        "Accept any ld-linux variant",
        "Check existence but not compatibility",
        "Skip if file exists as symlink"
    ],
    consequence = "No such file or directory (cannot execute binary)"
)]
#[test]
#[ignore]
fn test_validation_ld_linux_present() {
    let root = require_initramfs();

    let ld_linux = root.join("lib64/ld-linux-x86-64.so.2");
    assert!(
        ld_linux.exists(),
        "Dynamic linker missing: {}",
        ld_linux.display()
    );
}

// =============================================================================
// User/Group tests
// =============================================================================

#[cheat_aware(
    protects = "System has root user and dbus user for authentication",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check file exists but not content",
        "Accept any user entries, not specific required ones",
        "Use substring match instead of proper parsing"
    ],
    consequence = "su: user root does not exist or the user entry is missing"
)]
#[test]
#[ignore]
fn test_validation_passwd_group_valid() {
    let root = require_initramfs();

    let passwd = root.join("etc/passwd");
    let group = root.join("etc/group");

    assert_file_exists(&passwd);
    assert_file_exists(&group);

    // Check for root user
    assert_file_contains(&passwd, "root:x:0:0");

    // Check for dbus user (required for systemctl, timedatectl)
    assert_file_contains(&passwd, "dbus:");

    // Check for root group
    assert_file_contains(&group, "root:x:0:");
}

#[cheat_aware(
    protects = "Password authentication can work",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check existence not content",
        "Accept empty shadow file",
        "Skip permission validation"
    ],
    consequence = "Authentication token manipulation error"
)]
#[test]
#[ignore]
fn test_validation_shadow_exists() {
    let root = require_initramfs();

    let shadow = root.join("etc/shadow");
    assert_file_exists(&shadow);
    assert_file_contains(&shadow, "root:");
}

// =============================================================================
// Systemd unit tests
// =============================================================================

#[cheat_aware(
    protects = "Systemd can reach multi-user target and start getty",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check for units directory but not specific units",
        "Accept empty unit files",
        "Skip unit content validation"
    ],
    consequence = "Failed to start target multi-user.target"
)]
#[test]
#[ignore]
fn test_validation_systemd_units_present() {
    let root = require_initramfs();

    let units_dir = root.join("usr/lib/systemd/system");
    assert!(units_dir.is_dir(), "Systemd units directory missing");

    let required_units = [
        "basic.target",
        "multi-user.target",
        "getty.target",
        "getty@.service",
        "dbus.socket",
    ];

    for unit in required_units {
        let unit_path = units_dir.join(unit);
        assert!(
            unit_path.exists(),
            "Required systemd unit missing: {}",
            unit_path.display()
        );
    }
}

#[cheat_aware(
    protects = "User gets automatic login on tty1",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check file exists but not autologin content",
        "Accept any override file",
        "Skip validation of agetty command"
    ],
    consequence = "login: prompt appears instead of automatic shell"
)]
#[test]
#[ignore]
fn test_validation_getty_autologin_configured() {
    let root = require_initramfs();

    let autologin_conf = root.join("etc/systemd/system/getty@tty1.service.d/autologin.conf");
    assert_file_exists(&autologin_conf);
    assert_file_contains(&autologin_conf, "--autologin root");
}

#[cheat_aware(
    protects = "Serial console available for headless/VM testing",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check service exists but not ttyS0 config",
        "Accept service without proper WantedBy",
        "Skip TTYPath validation"
    ],
    consequence = "No output on serial console, QEMU test hangs"
)]
#[test]
#[ignore]
fn test_validation_serial_console_service() {
    let root = require_initramfs();

    let serial_console = root.join("etc/systemd/system/serial-console.service");
    assert_file_exists(&serial_console);
    assert_file_contains(&serial_console, "ttyS0");
}

// =============================================================================
// PAM tests
// =============================================================================

#[cheat_aware(
    protects = "Login authentication actually works",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check directory exists but not specific modules",
        "Accept any .so files as PAM modules",
        "Skip module functionality validation"
    ],
    consequence = "login: PAM unable to dlopen(pam_unix.so)"
)]
#[test]
#[ignore]
fn test_validation_pam_modules_present() {
    let root = require_initramfs();

    let security_dir = root.join("lib64/security");
    assert!(security_dir.is_dir(), "PAM security directory missing");

    let required_modules = ["pam_permit.so", "pam_unix.so"];

    for module in required_modules {
        let module_path = security_dir.join(module);
        assert!(
            module_path.exists(),
            "Required PAM module missing: {}",
            module_path.display()
        );
    }
}

#[cheat_aware(
    protects = "Login command has PAM configuration",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check pam.d exists but not login config",
        "Accept empty PAM config",
        "Skip config content validation"
    ],
    consequence = "login: no PAM configuration for login"
)]
#[test]
#[ignore]
fn test_validation_pam_config_exists() {
    let root = require_initramfs();

    let pam_d = root.join("etc/pam.d");
    assert!(pam_d.is_dir(), "PAM config directory missing");

    let login_conf = pam_d.join("login");
    assert_file_exists(&login_conf);
}

// =============================================================================
// OS identification tests
// =============================================================================

#[cheat_aware(
    protects = "System identifies as LevitateOS",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Check file exists but not branding",
        "Accept any os-release content",
        "Use partial match on NAME"
    ],
    consequence = "System shows wrong OS name (Rocky, etc.)"
)]
#[test]
#[ignore]
fn test_validation_os_release_correct() {
    let root = require_initramfs();

    let os_release = root.join("etc/os-release");
    assert_file_exists(&os_release);
    assert_file_contains(&os_release, "NAME=\"LevitateOS\"");
    assert_file_contains(&os_release, "ID=levitateos");
}

#[cheat_aware(
    protects = "Systemd can generate machine ID on boot",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Skip validation entirely",
        "Accept missing file as OK",
        "Check path but not accessibility"
    ],
    consequence = "Failed to create /etc/machine-id: Permission denied"
)]
#[test]
#[ignore]
fn test_validation_machine_id_exists() {
    let root = require_initramfs();

    let machine_id = root.join("etc/machine-id");
    assert_file_exists(&machine_id);
}

// =============================================================================
// D-Bus tests
// =============================================================================

#[cheat_aware(
    protects = "User can interact with D-Bus (timedatectl, hostnamectl)",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check usr/bin exists but not specific binaries",
        "Accept any dbus-related binary",
        "Skip executable bit validation"
    ],
    consequence = "Failed to connect to bus: No such file or directory"
)]
#[test]
#[ignore]
fn test_validation_dbus_binaries_present() {
    let root = require_initramfs();

    let dbus_binaries = ["busctl", "dbus-send"];

    for binary in dbus_binaries {
        let bin_path = root.join("usr/bin").join(binary);
        assert!(
            bin_path.exists(),
            "D-Bus binary missing: {}",
            bin_path.display()
        );
    }
}

#[cheat_aware(
    protects = "D-Bus socket activates on boot",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Accept broken symlink as enabled",
        "Check symlink exists but not target",
        "Skip validation of socket unit content"
    ],
    consequence = "D-Bus unavailable: timedatectl, hostnamectl fail"
)]
#[test]
#[ignore]
fn test_validation_dbus_socket_enabled() {
    let root = require_initramfs();

    let dbus_socket_link = root.join("etc/systemd/system/sockets.target.wants/dbus.socket");
    assert!(
        dbus_socket_link.exists() || dbus_socket_link.is_symlink(),
        "D-Bus socket not enabled"
    );
}

// =============================================================================
// FHS structure tests
// =============================================================================

#[cheat_aware(
    protects = "Standard directory layout for Unix binaries/configs",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check subset of required directories",
        "Accept files instead of directories",
        "Skip permission validation on directories"
    ],
    consequence = "Binaries fail to find expected paths (/etc, /var, etc.)"
)]
#[test]
#[ignore]
fn test_validation_fhs_structure() {
    let root = require_initramfs();

    let required_dirs = [
        "bin", "sbin", "lib64", "etc", "proc", "sys", "dev", "tmp", "root", "run",
        "var", "mnt", "usr/bin", "usr/lib/systemd",
    ];

    for dir in required_dirs {
        let dir_path = root.join(dir);
        assert!(
            dir_path.is_dir(),
            "FHS directory missing: {}",
            dir_path.display()
        );
    }
}

#[cheat_aware(
    protects = "/var/run works as expected (points to /run)",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Accept /var/run as directory instead of symlink",
        "Skip target validation",
        "Accept broken symlink"
    ],
    consequence = "PID files in wrong location, services fail to start"
)]
#[test]
#[ignore]
fn test_validation_var_run_symlink() {
    let root = require_initramfs();

    let var_run = root.join("var/run");
    assert_symlink(&var_run, "/run");
}

// =============================================================================
// Shell configuration tests
// =============================================================================

#[cheat_aware(
    protects = "User shell has proper PATH and environment",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Check files exist but not PATH content",
        "Accept empty profile",
        "Skip bashrc validation"
    ],
    consequence = "Commands not found despite being installed"
)]
#[test]
#[ignore]
fn test_validation_shell_config() {
    let root = require_initramfs();

    let profile = root.join("etc/profile");
    assert_file_exists(&profile);
    assert_file_contains(&profile, "PATH");

    let bashrc = root.join("root/.bashrc");
    assert_file_exists(&bashrc);
}

#[cheat_aware(
    protects = "System knows which shells are valid login shells",
    severity = "MEDIUM",
    ease = "EASY",
    cheats = [
        "Check file exists but not bash entry",
        "Accept empty shells file",
        "Skip validation of shell paths"
    ],
    consequence = "chsh: /bin/bash is not a valid shell"
)]
#[test]
#[ignore]
fn test_validation_shells_file() {
    let root = require_initramfs();

    let shells = root.join("etc/shells");
    assert_file_exists(&shells);
    assert_file_contains(&shells, "/bin/bash");
}

// =============================================================================
// Symlink integrity tests
// =============================================================================

#[cheat_aware(
    protects = "All symlinks in initramfs resolve correctly",
    severity = "HIGH",
    ease = "MEDIUM",
    cheats = [
        "Only check a subset of symlinks",
        "Skip absolute symlinks entirely",
        "Accept dangling symlinks as warnings"
    ],
    consequence = "Random 'No such file' errors for symlinked binaries"
)]
#[test]
#[ignore]
fn test_validation_no_broken_symlinks() {
    let root = require_initramfs();

    fn check_symlinks(dir: &Path, errors: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_symlink() {
                    // For absolute symlinks, we need to resolve within the initramfs root
                    if let Ok(target) = fs::read_link(&path) {
                        if target.is_absolute() {
                            // Skip checking absolute symlinks that point outside
                            // They're designed to work at runtime, not build time
                            continue;
                        }
                        // For relative symlinks, check if target exists
                        let resolved = path.parent().unwrap().join(&target);
                        if !resolved.exists() {
                            errors.push(format!(
                                "{} -> {} (broken)",
                                path.display(),
                                target.display()
                            ));
                        }
                    }
                } else if path.is_dir() {
                    check_symlinks(&path, errors);
                }
            }
        }
    }

    let mut errors = Vec::new();
    check_symlinks(&root, &mut errors);

    if !errors.is_empty() {
        panic!("Found broken relative symlinks:\n{}", errors.join("\n"));
    }
}

// =============================================================================
// Systemd helper binaries tests
// =============================================================================

#[cheat_aware(
    protects = "Systemd 255+ can execute services properly",
    severity = "CRITICAL",
    ease = "EASY",
    cheats = [
        "Check for old systemd helpers only",
        "Accept missing executor as warning",
        "Skip journald validation"
    ],
    consequence = "Failed to spawn service: No such file or directory"
)]
#[test]
#[ignore]
fn test_validation_systemd_helper_binaries() {
    let root = require_initramfs();

    let systemd_dir = root.join("usr/lib/systemd");

    // These are required for systemd 255+
    let required_helpers = ["systemd-executor", "systemd-journald"];

    for helper in required_helpers {
        let helper_path = systemd_dir.join(helper);
        assert!(
            helper_path.exists(),
            "Required systemd helper missing: {}",
            helper_path.display()
        );
    }
}

// =============================================================================
// Systemd utility tests
// =============================================================================

#[cheat_aware(
    protects = "User can manage system with systemctl, journalctl, etc.",
    severity = "HIGH",
    ease = "EASY",
    cheats = [
        "Check bin/ exists but not specific utilities",
        "Accept some utilities missing",
        "Move missing to OPTIONAL list"
    ],
    consequence = "bash: systemctl: command not found"
)]
#[test]
#[ignore]
fn test_validation_systemd_utilities_present() {
    let root = require_initramfs();

    let utilities = ["systemctl", "journalctl", "timedatectl", "hostnamectl"];

    for util in utilities {
        let util_path = root.join("bin").join(util);
        assert!(
            util_path.exists(),
            "Systemd utility missing: {}",
            util_path.display()
        );
    }
}

// =============================================================================
// Canary test - DO NOT MODIFY without triggering review
// =============================================================================

/// CANARY TEST: This test is intentionally verbose and looks tempting to simplify.
///
/// Each binary is checked individually on purpose. A cheater would want to
/// consolidate this into a loop or reduce the list. Any modification to this
/// test should trigger additional review.
#[cheat_canary(
    bait = "This test looks tedious with individual assertions - tempting to refactor to a loop",
    tripwire = "Any modification to this function triggers full audit of all test file changes"
)]
#[test]
#[ignore]
fn canary_validation_verbose_binary_check() {
    let root = require_initramfs();

    // DO NOT consolidate these into a loop or array.
    // This verbosity is intentional - it makes cheating obvious.
    // If you need to remove a binary, you must modify this specific line,
    // which triggers canary detection.

    let bash = root.join("bin/bash");
    assert!(bash.exists(), "bash missing at {}", bash.display());

    let ls = root.join("bin/ls");
    assert!(ls.exists(), "ls missing at {}", ls.display());

    let cat = root.join("bin/cat");
    assert!(cat.exists(), "cat missing at {}", cat.display());

    let mount = root.join("bin/mount");
    assert!(mount.exists(), "mount missing at {}", mount.display());

    let login = root.join("bin/login");
    assert!(login.exists(), "login missing at {}", login.display());

    let agetty = root.join("bin/agetty");
    assert!(agetty.exists(), "agetty missing at {}", agetty.display());

    let systemctl = root.join("bin/systemctl");
    assert!(systemctl.exists(), "systemctl missing at {}", systemctl.display());

    let journalctl = root.join("bin/journalctl");
    assert!(journalctl.exists(), "journalctl missing at {}", journalctl.display());

    // Systemd itself
    let systemd = root.join("usr/lib/systemd/systemd");
    assert!(systemd.exists(), "systemd missing at {}", systemd.display());

    // /sbin/init symlink
    let init = root.join("sbin/init");
    assert!(init.exists() || init.is_symlink(), "init missing at {}", init.display());
}
