use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Builder for QEMU commands - consolidates common configuration patterns
#[derive(Default)]
struct QemuBuilder {
    cpu: Option<String>,
    memory: Option<String>,
    kernel: Option<PathBuf>,
    initrd: Option<PathBuf>,
    append: Option<String>,
    cdrom: Option<PathBuf>,
    disk: Option<PathBuf>,
    ovmf: Option<PathBuf>,
    nographic: bool,
    no_reboot: bool,
    vga: Option<String>,
}

impl QemuBuilder {
    fn new() -> Self {
        Self::default()
    }

    /// Set CPU type (default: Skylake-Client for x86-64-v3 support)
    fn cpu(mut self, cpu: &str) -> Self {
        self.cpu = Some(cpu.to_string());
        self
    }

    /// Set memory size (e.g., "512M", "1G")
    fn memory(mut self, mem: &str) -> Self {
        self.memory = Some(mem.to_string());
        self
    }

    /// Set kernel for direct boot
    fn kernel(mut self, path: PathBuf) -> Self {
        self.kernel = Some(path);
        self
    }

    /// Set initrd for direct boot
    fn initrd(mut self, path: PathBuf) -> Self {
        self.initrd = Some(path);
        self
    }

    /// Set kernel command line arguments
    fn append(mut self, args: &str) -> Self {
        self.append = Some(args.to_string());
        self
    }

    /// Set ISO for CD-ROM boot
    fn cdrom(mut self, path: PathBuf) -> Self {
        self.cdrom = Some(path);
        self
    }

    /// Add virtio disk
    fn disk(mut self, path: PathBuf) -> Self {
        self.disk = Some(path);
        self
    }

    /// Enable UEFI boot with OVMF firmware
    fn uefi(mut self, ovmf_path: PathBuf) -> Self {
        self.ovmf = Some(ovmf_path);
        self
    }

    /// Disable graphics, use serial console
    fn nographic(mut self) -> Self {
        self.nographic = true;
        self
    }

    /// Don't reboot on exit
    fn no_reboot(mut self) -> Self {
        self.no_reboot = true;
        self
    }

    /// Set VGA adapter type (e.g., "std", "virtio")
    fn vga(mut self, vga_type: &str) -> Self {
        self.vga = Some(vga_type.to_string());
        self
    }

    /// Build the QEMU command
    fn build(self) -> Command {
        let mut cmd = Command::new("qemu-system-x86_64");

        // CPU (default: Skylake-Client for x86-64-v3 support required by Rocky 10)
        let cpu = self.cpu.as_deref().unwrap_or("Skylake-Client");
        cmd.args(["-cpu", cpu]);

        // Memory (default: 512M)
        let mem = self.memory.as_deref().unwrap_or("512M");
        cmd.args(["-m", mem]);

        // Direct kernel boot
        if let Some(kernel) = &self.kernel {
            cmd.args(["-kernel", kernel.to_str().unwrap()]);
        }
        if let Some(initrd) = &self.initrd {
            cmd.args(["-initrd", initrd.to_str().unwrap()]);
        }
        if let Some(append) = &self.append {
            cmd.args(["-append", append]);
        }

        // CD-ROM boot
        if let Some(cdrom) = &self.cdrom {
            cmd.args(["-cdrom", cdrom.to_str().unwrap()]);
        }

        // Virtio disk
        if let Some(disk) = &self.disk {
            cmd.args([
                "-drive",
                &format!("file={},format=qcow2,if=virtio", disk.display()),
            ]);
        }

        // UEFI firmware
        if let Some(ovmf) = &self.ovmf {
            cmd.args([
                "-drive",
                &format!("if=pflash,format=raw,readonly=on,file={}", ovmf.display()),
            ]);
        }

        // Display options
        if self.nographic {
            cmd.args(["-nographic", "-serial", "mon:stdio"]);
        } else if let Some(vga) = &self.vga {
            cmd.args(["-vga", vga]);
        }

        // Reboot behavior
        if self.no_reboot {
            cmd.arg("-no-reboot");
        }

        cmd
    }
}

/// Find OVMF firmware for UEFI boot
fn find_ovmf() -> Option<PathBuf> {
    // Common OVMF locations across distros
    let candidates = [
        // Fedora/RHEL
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        // Debian/Ubuntu
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/qemu/OVMF.fd",
        // Arch
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
        // NixOS
        "/run/libvirt/nix-ovmf/OVMF_CODE.fd",
    ];

    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Quick test: direct kernel boot in terminal (for debugging)
pub fn test_direct(base_dir: &Path, cmd: Option<String>) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let output_dir = base_dir.join("output");

    // Find kernel
    let kernel_path = downloads_dir.join("iso-contents/images/pxeboot/vmlinuz");
    if !kernel_path.exists() {
        bail!(
            "Kernel not found at {}. Run 'leviso extract' first.",
            kernel_path.display()
        );
    }

    // Find initramfs
    let initramfs_path = output_dir.join("initramfs.cpio.gz");
    if !initramfs_path.exists() {
        bail!(
            "Initramfs not found at {}. Run 'leviso initramfs' first.",
            initramfs_path.display()
        );
    }

    // Create/find disk (8GB default) - use separate file from 'run' command
    let disk_path = output_dir.join("test-disk.qcow2");
    if !disk_path.exists() {
        println!("Creating 8GB virtual disk...");
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2", disk_path.to_str().unwrap(), "8G"])
            .status()
            .context("Failed to run qemu-img")?;
        if !status.success() {
            bail!("qemu-img create failed");
        }
    }

    println!("Quick test: direct kernel boot (serial console)");
    println!("  Kernel:    {}", kernel_path.display());
    println!("  Initramfs: {}", initramfs_path.display());
    println!("  Disk:      {}", disk_path.display());

    if let Some(ref command) = cmd {
        println!("  Command:   {}", command);
        run_with_command(kernel_path, initramfs_path, command, Some(disk_path))
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        run_interactive(kernel_path, initramfs_path, Some(disk_path))
    }
}

fn run_interactive(kernel_path: PathBuf, initramfs_path: PathBuf, disk_path: Option<PathBuf>) -> Result<()> {
    let mut builder = QemuBuilder::new()
        .kernel(kernel_path)
        .initrd(initramfs_path)
        .append("console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30")
        .nographic();

    if let Some(disk) = disk_path {
        builder = builder.disk(disk);
    }

    let status = builder
        .build()
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}

fn run_with_command(kernel_path: PathBuf, initramfs_path: PathBuf, command: &str, disk_path: Option<PathBuf>) -> Result<()> {
    // Use a unique marker to detect command completion
    const DONE_MARKER: &str = "___LEVISO_CMD_DONE___";

    let mut builder = QemuBuilder::new()
        .kernel(kernel_path)
        .initrd(initramfs_path)
        .append("console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30")
        .nographic()
        .no_reboot();

    if let Some(disk) = disk_path {
        builder = builder.disk(disk);
    }

    let mut child = builder.build()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn QEMU")?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");

    // Use a thread to read output - this avoids blocking on lines() when prompt has no newline
    let (tx, rx) = std::sync::mpsc::channel();
    let reader_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                if tx.send(l).is_err() {
                    break;
                }
            }
        }
    });

    let start = Instant::now();
    let timeout = Duration::from_secs(30);
    let mut boot_finished = false;
    let mut command_sent = false;
    let mut output_buffer = String::new();

    println!("\n--- Waiting for boot ---");

    loop {
        // Check timeout
        if start.elapsed() > timeout {
            eprintln!("\nTimeout after 30 seconds");
            let _ = child.kill();
            break;
        }

        // Try to receive a line with a short timeout
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                println!("{}", line);
                output_buffer.push_str(&line);
                output_buffer.push('\n');

                // Stage 1: Detect systemd boot completion
                if !boot_finished && line.contains("Startup finished") {
                    boot_finished = true;
                    println!("\n--- Boot finished, sending command ---");

                    // Wait for shell to be ready (2s to be safe)
                    std::thread::sleep(Duration::from_secs(2));

                    // Send command followed by marker
                    let full_cmd = format!(
                        "{}; echo '{}'\n",
                        command, DONE_MARKER
                    );
                    if stdin.write_all(full_cmd.as_bytes()).is_ok() {
                        let _ = stdin.flush();
                        command_sent = true;
                        println!(">>> {}", command);
                    }
                }

                // Stage 2: Look for our completion marker (on its own line, from echo)
                // Don't match the command echo which includes the marker in the command string
                if command_sent && line.trim() == DONE_MARKER {
                    println!("\n--- Command completed, shutting down ---");
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No data, continue waiting
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread finished (QEMU exited)
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    drop(reader_thread);
    Ok(())
}

/// Run the ISO in QEMU GUI (closest to bare metal)
pub fn run_iso(base_dir: &Path, force_bios: bool, disk_size: Option<String>) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join("leviso.iso");

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("Running ISO in QEMU GUI...");
    println!("  ISO: {}", iso_path.display());

    let mut builder = QemuBuilder::new()
        .cdrom(iso_path)
        .vga("std");

    // Handle virtual disk if requested
    if let Some(size) = disk_size {
        let disk_path = output_dir.join("virtual-disk.qcow2");

        // Create disk if it doesn't exist
        if !disk_path.exists() {
            println!("  Creating {}B virtual disk...", size);
            let status = Command::new("qemu-img")
                .args(["create", "-f", "qcow2", disk_path.to_str().unwrap(), &size])
                .status()
                .context("Failed to run qemu-img. Is QEMU installed?")?;

            if !status.success() {
                bail!("qemu-img create failed");
            }
        }

        println!("  Disk: {} ({})", disk_path.display(), size);
        builder = builder.disk(disk_path);
    }

    // UEFI boot by default (it's 2026), unless --bios is specified
    if force_bios {
        println!("  Boot: BIOS (legacy)");
    } else if let Some(ovmf_path) = find_ovmf() {
        println!("  Boot: UEFI ({})", ovmf_path.display());
        builder = builder.uefi(ovmf_path);
    } else {
        println!("  Boot: BIOS (OVMF not found, install edk2-ovmf for UEFI)");
    }

    let status = builder
        .build()
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
