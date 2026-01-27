//! QEMU launcher for LevitateOS development and testing.
//!
//! Provides `run_iso()` for GUI testing and `test_iso()` for headless boot verification.
//! Uses the shared `recqemu` crate for QEMU command building.

use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use distro_spec::levitate::{
    ISO_FILENAME,
    QEMU_MEMORY_GB, QEMU_DISK_GB,
    QEMU_DISK_FILENAME, QEMU_SERIAL_LOG,
};
use recqemu::{QemuBuilder, find_ovmf, create_disk};

/// Success patterns - if we see any of these, boot succeeded.
const SUCCESS_PATTERNS: &[&str] = &[
    "login:",                         // Getty prompt - definitive success
    "Welcome to LevitateOS",          // Welcome message
    "Startup finished",               // systemd boot complete
    "systemd[1]: Reached target",     // systemd reached a target
    "systemd[1]: Started Getty",      // Getty started
];

/// Failure patterns - if we see any of these, boot failed.
const FAILURE_PATTERNS: &[&str] = &[
    "Kernel panic",
    "not syncing",
    "VFS: Cannot open root device",
    "No init found",
    "can't find /init",
    "EROFS error",
    "failed to mount",
    "emergency.target",
    "No bootable device",
    "Boot Failed",
];

/// Run the ISO in QEMU GUI (closest to bare metal).
pub fn run_iso(base_dir: &Path, disk_size: Option<String>) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join(ISO_FILENAME);

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("Running ISO in QEMU GUI...");
    println!("  ISO: {}", iso_path.display());

    // Check KVM availability
    if recqemu::kvm_available() {
        println!("  Acceleration: KVM (hardware virtualization)");
    } else {
        println!("  Acceleration: TCG (software emulation - slower)");
    }

    // Always include a virtual disk (default 20GB, like a real system)
    let size = disk_size.unwrap_or_else(|| format!("{}G", QEMU_DISK_GB));
    let disk_path = output_dir.join(QEMU_DISK_FILENAME);

    // Create disk if it doesn't exist
    if !disk_path.exists() {
        println!("  Creating {} virtual disk...", size);
        create_disk(&disk_path, &size)?;
    }
    println!("  Disk: {}", disk_path.display());

    // LevitateOS requires UEFI boot
    let ovmf_path = find_ovmf().context(
        "OVMF firmware not found. LevitateOS requires UEFI boot.\n\
         Install OVMF:\n\
         - Fedora/RHEL: sudo dnf install edk2-ovmf\n\
         - Debian/Ubuntu: sudo apt install ovmf\n\
         - Arch: sudo pacman -S edk2-ovmf",
    )?;
    println!("  Boot: UEFI ({})", ovmf_path.display());

    // Build and run QEMU
    let status = QemuBuilder::new()
        .memory(&format!("{}G", QEMU_MEMORY_GB))
        .cdrom(&iso_path)
        .disk(&disk_path)
        .uefi(&ovmf_path)
        .user_network()
        .display("gtk,gl=on")
        .vga("virtio")
        .serial_file(output_dir.join(QEMU_SERIAL_LOG))
        .build_interactive()
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}

/// Test the ISO by booting headless and watching serial output.
///
/// Uses AHCI for CD-ROM (like real SATA hardware) to verify that the
/// real hardware drivers (ahci, libata, sr_mod) work correctly.
///
/// Returns Ok(()) if boot succeeds (login prompt reached).
/// Returns Err if boot fails or times out.
pub fn test_iso(base_dir: &Path, timeout_secs: u64) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join(ISO_FILENAME);

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("=== LevitateOS Boot Test ===\n");
    println!("ISO: {}", iso_path.display());
    println!("Timeout: {}s", timeout_secs);
    println!();

    // Find OVMF
    let ovmf_path = find_ovmf().context("OVMF not found - UEFI boot required")?;

    // Build headless QEMU command
    // Note: We use build() + manual stdio setup because test needs piped stdout
    // but different serial config than build_piped() provides
    let mut cmd = QemuBuilder::new()
        .memory(&format!("{}G", QEMU_MEMORY_GB))
        .smp(2)
        .cdrom(&iso_path)
        .uefi(&ovmf_path)
        .nographic()
        .serial_stdio()
        .no_reboot()
        .build();

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    println!("Starting QEMU (headless, serial console)...\n");

    let mut child = cmd.spawn().context("Failed to spawn qemu-system-x86_64")?;
    let stdout = child.stdout.take().context("Failed to capture stdout")?;

    // Spawn reader thread
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Watch for patterns
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let stall_timeout = Duration::from_secs(30);
    let mut last_output = Instant::now();
    let mut output_buffer: Vec<String> = Vec::new();

    // Boot stage tracking
    let mut saw_uefi = false;
    let mut saw_kernel = false;
    let mut saw_init = false;

    println!("Watching boot output...\n");

    loop {
        // Check overall timeout
        if start.elapsed() > timeout {
            let _ = child.kill();
            let last_lines = output_buffer.iter().rev().take(20).cloned().collect::<Vec<_>>();
            bail!(
                "TIMEOUT: Boot did not complete in {}s\n\nLast output:\n{}",
                timeout_secs,
                last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
            );
        }

        // Check stall
        if last_output.elapsed() > stall_timeout {
            let _ = child.kill();
            let stage = if saw_init {
                "Init started but stalled"
            } else if saw_kernel {
                "Kernel started but init stalled"
            } else if saw_uefi {
                "UEFI ran but kernel stalled"
            } else {
                "No output - QEMU/serial broken"
            };
            bail!("STALL: {} (no output for {}s)", stage, stall_timeout.as_secs());
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                last_output = Instant::now();
                output_buffer.push(line.clone());

                // Print output for visibility
                println!("  {}", line);

                // Track boot stages
                if line.contains("UEFI") || line.contains("EFI") || line.contains("BdsDxe") {
                    saw_uefi = true;
                }
                if line.contains("Linux version") || line.contains("Booting Linux") {
                    saw_kernel = true;
                }
                if line.contains("systemd") || line.contains("init") {
                    saw_init = true;
                }

                // Check failure patterns first (fail fast)
                for pattern in FAILURE_PATTERNS {
                    if line.contains(pattern) {
                        let _ = child.kill();
                        let last_lines = output_buffer.iter().rev().take(30).cloned().collect::<Vec<_>>();
                        bail!(
                            "BOOT FAILED: {}\n\nContext:\n{}",
                            pattern,
                            last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                        );
                    }
                }

                // Check success patterns
                for pattern in SUCCESS_PATTERNS {
                    if line.contains(pattern) {
                        let elapsed = start.elapsed().as_secs_f64();
                        let _ = child.kill();
                        let _ = child.wait();

                        println!();
                        println!("═══════════════════════════════════════════════════════════");
                        println!("BOOT SUCCESS: Matched '{}'", pattern);
                        println!("═══════════════════════════════════════════════════════════");
                        println!();
                        println!("Boot completed in {:.1}s", elapsed);
                        println!("LevitateOS is ready for login.");

                        return Ok(());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let last_lines = output_buffer.iter().rev().take(20).cloned().collect::<Vec<_>>();
                bail!(
                    "QEMU exited unexpectedly\n\nLast output:\n{}",
                    last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                );
            }
        }
    }
}
