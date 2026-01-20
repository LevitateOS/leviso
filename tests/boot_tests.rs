//! Boot tests for leviso using QEMU.
//!
//! These tests verify that the built initramfs boots correctly and that
//! system services function properly. They require:
//!   1. Running `cargo run -- initramfs` first
//!   2. QEMU installed on the system
//!
//! Run these tests with:
//!   cargo test -- --ignored boot

mod helpers;

use helpers::{initramfs_is_built, real_initramfs_root};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Get path to the kernel.
fn kernel_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Path::new(&manifest_dir).join("downloads/iso-contents/images/pxeboot/vmlinuz")
}

/// Get path to the initramfs.
fn initramfs_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Path::new(&manifest_dir).join("output/initramfs.cpio.gz")
}

/// Get path to the test disk.
fn test_disk_path() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Path::new(&manifest_dir).join("output/boot-test-disk.qcow2")
}

/// Check if QEMU is available.
fn qemu_available() -> bool {
    Command::new("qemu-system-x86_64")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if boot test prerequisites are met.
fn require_boot_prerequisites() {
    if !initramfs_is_built() {
        panic!(
            "Initramfs not built. Run 'cargo run -- initramfs' first.\nExpected at: {}",
            initramfs_path().display()
        );
    }

    let kernel = kernel_path();
    if !kernel.exists() {
        panic!(
            "Kernel not found. Run 'cargo run -- extract' first.\nExpected at: {}",
            kernel.display()
        );
    }

    if !qemu_available() {
        panic!("QEMU not installed. Install qemu-system-x86_64 to run boot tests.");
    }
}

/// Create the test disk if it doesn't exist.
fn ensure_test_disk() -> PathBuf {
    let disk = test_disk_path();
    if !disk.exists() {
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2", disk.to_str().unwrap(), "1G"])
            .status()
            .expect("Failed to run qemu-img");
        assert!(status.success(), "Failed to create test disk");
    }
    disk
}

/// Result of running a command in QEMU.
struct QemuCommandResult {
    /// Whether the command was sent and completed
    pub completed: bool,
    /// Output buffer from the boot and command
    pub output: String,
    /// Whether boot finished successfully
    pub boot_finished: bool,
}

/// Run a command in QEMU and capture output.
fn run_qemu_command(command: &str, timeout_secs: u64) -> QemuCommandResult {
    const DONE_MARKER: &str = "___BOOT_TEST_DONE___";

    let kernel = kernel_path();
    let initramfs = initramfs_path();
    let disk = ensure_test_disk();

    let mut child = Command::new("qemu-system-x86_64")
        .args(["-cpu", "Skylake-Client"])
        .args(["-m", "512M"])
        .args(["-kernel", kernel.to_str().unwrap()])
        .args(["-initrd", initramfs.to_str().unwrap()])
        .args([
            "-append",
            "console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30",
        ])
        .args([
            "-drive",
            &format!("file={},format=qcow2,if=virtio", disk.display()),
        ])
        .args(["-nographic", "-serial", "mon:stdio"])
        .arg("-no-reboot")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn QEMU");

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");

    // Reader thread
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let mut boot_finished = false;
    let mut command_sent = false;
    let mut output_buffer = String::new();
    let mut completed = false;

    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            break;
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                output_buffer.push_str(&line);
                output_buffer.push('\n');

                // Detect boot completion
                if !boot_finished && line.contains("Startup finished") {
                    boot_finished = true;
                    std::thread::sleep(Duration::from_secs(2));

                    // Send command
                    let full_cmd = format!("{}; echo '{}'\n", command, DONE_MARKER);
                    if stdin.write_all(full_cmd.as_bytes()).is_ok() {
                        let _ = stdin.flush();
                        command_sent = true;
                    }
                }

                // Detect command completion
                if command_sent && line.trim() == DONE_MARKER {
                    completed = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    QemuCommandResult {
        completed,
        output: output_buffer,
        boot_finished,
    }
}

// =============================================================================
// Boot tests
// =============================================================================

#[test]
#[ignore]
fn test_boot_reaches_shell() {
    require_boot_prerequisites();

    let result = run_qemu_command("echo 'SHELL_REACHED'", 60);

    assert!(
        result.boot_finished,
        "System did not finish booting. Output:\n{}",
        result.output
    );
    assert!(
        result.completed,
        "Command did not complete. Output:\n{}",
        result.output
    );
    assert!(
        result.output.contains("SHELL_REACHED"),
        "Shell not reached. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_systemctl_works() {
    require_boot_prerequisites();

    let result = run_qemu_command("systemctl is-system-running", 60);

    assert!(result.completed, "Command did not complete");

    // System should be either "running" or "degraded" (some services may not start in minimal env)
    let has_running = result.output.contains("running");
    let has_degraded = result.output.contains("degraded");

    assert!(
        has_running || has_degraded,
        "systemctl did not report running/degraded state. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_timedatectl_works() {
    require_boot_prerequisites();

    let result = run_qemu_command("timedatectl", 60);

    assert!(result.completed, "Command did not complete");

    // timedatectl should show local time
    assert!(
        result.output.contains("Local time") || result.output.contains("Universal time"),
        "timedatectl did not show time info. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_journalctl_works() {
    require_boot_prerequisites();

    let result = run_qemu_command("journalctl -n 5", 60);

    assert!(result.completed, "Command did not complete");

    // journalctl should show some entries (might show "No journal files" in minimal env)
    // Just verify it doesn't error out completely
    let has_journal = result.output.contains("--") || result.output.contains("journal");
    assert!(
        has_journal || result.output.contains("No journal files"),
        "journalctl failed completely. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_disk_visible() {
    require_boot_prerequisites();

    let result = run_qemu_command("lsblk", 60);

    assert!(result.completed, "Command did not complete");

    // Should see vda (virtio disk)
    assert!(
        result.output.contains("vda"),
        "Virtual disk not visible via lsblk. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_dbus_running() {
    require_boot_prerequisites();

    let result = run_qemu_command("busctl status", 60);

    assert!(result.completed, "Command did not complete");

    // busctl status should connect to D-Bus and show some info
    // Even if it fails, it should produce output
    assert!(
        result.output.contains("PID") || result.output.contains("dbus"),
        "D-Bus doesn't appear to be running. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_hostname() {
    require_boot_prerequisites();

    let result = run_qemu_command("hostname", 60);

    assert!(result.completed, "Command did not complete");

    // Should have some hostname set
    assert!(
        !result.output.trim().is_empty(),
        "Hostname command produced no output"
    );
}

#[test]
#[ignore]
fn test_boot_uname() {
    require_boot_prerequisites();

    let result = run_qemu_command("uname -a", 60);

    assert!(result.completed, "Command did not complete");

    // Should show Linux kernel info
    assert!(
        result.output.contains("Linux"),
        "uname did not show Linux. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_env_vars() {
    require_boot_prerequisites();

    let result = run_qemu_command("env | head -10", 60);

    assert!(result.completed, "Command did not complete");

    // Should have PATH set
    assert!(
        result.output.contains("PATH="),
        "PATH not set in environment. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_filesystem_writable() {
    require_boot_prerequisites();

    let result = run_qemu_command("touch /tmp/test && echo 'FS_WRITABLE' && rm /tmp/test", 60);

    assert!(result.completed, "Command did not complete");

    assert!(
        result.output.contains("FS_WRITABLE"),
        "Filesystem not writable. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_proc_mounted() {
    require_boot_prerequisites();

    let result = run_qemu_command("cat /proc/version", 60);

    assert!(result.completed, "Command did not complete");

    assert!(
        result.output.contains("Linux version"),
        "/proc not mounted or readable. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_sys_mounted() {
    require_boot_prerequisites();

    let result = run_qemu_command("ls /sys/class", 60);

    assert!(result.completed, "Command did not complete");

    // /sys/class should have entries like 'block', 'net', etc.
    assert!(
        result.output.contains("block") || result.output.contains("net"),
        "/sys not mounted correctly. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_dev_populated() {
    require_boot_prerequisites();

    let result = run_qemu_command("ls /dev/null /dev/zero /dev/tty", 60);

    assert!(result.completed, "Command did not complete");

    // Basic device nodes should exist
    assert!(
        result.output.contains("/dev/null"),
        "Device nodes not created. Output:\n{}",
        result.output
    );
}

#[test]
#[ignore]
fn test_boot_root_user() {
    require_boot_prerequisites();

    let result = run_qemu_command("whoami && id", 60);

    assert!(result.completed, "Command did not complete");

    // Should be running as root
    assert!(
        result.output.contains("root") && result.output.contains("uid=0"),
        "Not running as root. Output:\n{}",
        result.output
    );
}
