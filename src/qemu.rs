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

        // CPU: Use "max" to get all features TCG supports without warnings.
        // Skylake-Client causes TCG warnings about unsupported features (pcid, hle, rtm, etc.)
        // "max" provides x86-64-v3+ features that Rocky 10 needs, without the noise.
        let cpu = self.cpu.as_deref().unwrap_or("max");
        cmd.args(["-cpu", cpu]);

        // Memory (default: 4G - LevitateOS is a daily driver OS, not a toy)
        let mem = self.memory.as_deref().unwrap_or("4G");
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

        // CD-ROM (use virtio-scsi for better compatibility with modern kernels)
        if let Some(cdrom) = &self.cdrom {
            // Add virtio-scsi controller and attach CD-ROM as SCSI device
            cmd.args([
                "-device", "virtio-scsi-pci,id=scsi0",
                "-device", "scsi-cd,drive=cdrom0,bus=scsi0.0",
                "-drive", &format!("id=cdrom0,if=none,format=raw,readonly=on,file={}", cdrom.display()),
            ]);
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

        // Network: virtio-net with user-mode NAT (provides DHCP)
        cmd.args([
            "-netdev", "user,id=net0",
            "-device", "virtio-net-pci,netdev=net0",
        ]);

        // Display options
        if self.nographic {
            cmd.args(["-nographic", "-serial", "mon:stdio"]);
        } else if let Some(vga) = &self.vga {
            cmd.args(["-vga", vga]);
            // Add serial port even in GUI mode so kernel messages are captured
            // Kernel cmdline has console=ttyS0 which needs a serial device
            cmd.args(["-serial", "file:/tmp/levitateos-serial.log"]);
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

    // Find ISO (for CDROM - needed to access base tarball)
    let iso_path = output_dir.join("levitateos.iso");
    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso iso' first.",
            iso_path.display()
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
    println!("  ISO:       {}", iso_path.display());
    println!("  Disk:      {}", disk_path.display());

    if let Some(ref command) = cmd {
        println!("  Command:   {}", command);
        run_with_command(kernel_path, initramfs_path, command, Some(disk_path), Some(iso_path))
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        run_interactive(kernel_path, initramfs_path, Some(disk_path), Some(iso_path))
    }
}

fn run_interactive(kernel_path: PathBuf, initramfs_path: PathBuf, disk_path: Option<PathBuf>, iso_path: Option<PathBuf>) -> Result<()> {
    let mut builder = QemuBuilder::new()
        .kernel(kernel_path)
        .initrd(initramfs_path)
        .append("console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30")
        .nographic();

    if let Some(disk) = disk_path {
        builder = builder.disk(disk);
    }

    if let Some(iso) = iso_path {
        builder = builder.cdrom(iso);
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

fn run_with_command(kernel_path: PathBuf, initramfs_path: PathBuf, command: &str, disk_path: Option<PathBuf>, iso_path: Option<PathBuf>) -> Result<()> {
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

    if let Some(iso) = iso_path {
        builder = builder.cdrom(iso);
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
        for l in reader.lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });

    let start = Instant::now();
    let timeout = Duration::from_secs(60);
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
pub fn run_iso(base_dir: &Path, disk_size: Option<String>) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join("levitateos.iso");

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("Running ISO in QEMU GUI...");
    println!("  ISO: {}", iso_path.display());

    // Use virtio-gpu - kernel has CONFIG_DRM_VIRTIO_GPU=y
    // Note: "std" VGA requires efifb/simpledrm for UEFI boot which the kernel lacks
    let mut builder = QemuBuilder::new().cdrom(iso_path.clone()).vga("virtio");

    // Also add serial port so we can see kernel messages even in GUI mode
    // The kernel cmdline in grub.cfg has console=ttyS0 which needs a serial port

    // Always include a virtual disk (default 20GB, like a real system)
    let size = disk_size.unwrap_or_else(|| "20G".to_string());
    let disk_path = output_dir.join("virtual-disk.qcow2");

    // Create disk if it doesn't exist
    if !disk_path.exists() {
        println!("  Creating {} virtual disk...", size);
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2", disk_path.to_str().unwrap(), &size])
            .status()
            .context("Failed to run qemu-img. Is QEMU installed?")?;

        if !status.success() {
            bail!("qemu-img create failed");
        }
    }

    println!("  Disk: {}", disk_path.display());
    builder = builder.disk(disk_path);

    // LevitateOS requires UEFI boot
    let ovmf_path = find_ovmf().context(
        "OVMF firmware not found. LevitateOS requires UEFI boot.\n\
         Install OVMF:\n\
         - Fedora/RHEL: sudo dnf install edk2-ovmf\n\
         - Debian/Ubuntu: sudo apt install ovmf\n\
         - Arch: sudo pacman -S edk2-ovmf",
    )?;

    println!("  Boot: UEFI ({})", ovmf_path.display());
    builder = builder.uefi(ovmf_path);

    let status = builder
        .build()
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
