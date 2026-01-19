use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

    println!("Quick test: direct kernel boot (serial console)");
    println!("  Kernel:    {}", kernel_path.display());
    println!("  Initramfs: {}", initramfs_path.display());

    if let Some(ref command) = cmd {
        println!("  Command:   {}", command);
        run_with_command(kernel_path, initramfs_path, command)
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        run_interactive(kernel_path, initramfs_path)
    }
}

fn run_interactive(kernel_path: PathBuf, initramfs_path: PathBuf) -> Result<()> {
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-cpu", "Skylake-Client",
        "-m", "512M",
        "-kernel", kernel_path.to_str().unwrap(),
        "-initrd", initramfs_path.to_str().unwrap(),
        "-append", "console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30",
        "-nographic",
        "-serial", "mon:stdio",
    ]);

    let status = cmd
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}

fn run_with_command(kernel_path: PathBuf, initramfs_path: PathBuf, command: &str) -> Result<()> {
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-cpu", "Skylake-Client",
            "-m", "512M",
            "-kernel", kernel_path.to_str().unwrap(),
            "-initrd", initramfs_path.to_str().unwrap(),
            "-append", "console=tty0 console=ttyS0,115200n8 rdinit=/init panic=30",
            "-nographic",
            "-serial", "mon:stdio",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn QEMU")?;

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let reader = BufReader::new(stdout);

    let start = Instant::now();
    let timeout = Duration::from_secs(90);
    let mut boot_finished = false;
    let mut command_sent = false;
    let mut output_lines = Vec::new();

    println!("\n--- Waiting for boot ---");

    for line in reader.lines() {
        if start.elapsed() > timeout {
            eprintln!("Timeout waiting for command completion");
            let _ = child.kill();
            break;
        }

        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Print boot output
        println!("{}", line);

        // Stage 1: Detect systemd boot completion
        if !boot_finished && line.contains("Startup finished") {
            boot_finished = true;
            println!("\n--- Boot finished, sending command in 2 seconds ---");

            // Wait for shell to be fully ready
            std::thread::sleep(Duration::from_secs(2));

            // Send the command
            let full_cmd = format!("{}\n", command);
            if stdin.write_all(full_cmd.as_bytes()).is_ok() {
                let _ = stdin.flush();
                command_sent = true;
                println!(">>> {}", command);
            }
        }

        // Collect output after command is sent
        if command_sent {
            output_lines.push(line.clone());

            // Exit when we see the shell prompt AFTER the command output
            // First few lines are: command echo, then output, then prompt
            // The prompt format is "root@leviso:~# " or similar
            if output_lines.len() >= 3 && line.contains("root@") && line.contains("#") && !line.contains(&command[..8.min(command.len())]) {
                println!("\n--- Command completed ---");
                let _ = child.kill();
                break;
            }

            // Fallback: exit after 15 lines or 30 seconds
            if output_lines.len() >= 15 || start.elapsed() > Duration::from_secs(30) {
                println!("\n--- Command output collected (timeout) ---");
                let _ = child.kill();
                break;
            }
        }
    }

    let _ = child.wait();
    Ok(())
}

/// Run the ISO in QEMU GUI (closest to bare metal)
pub fn run_iso(base_dir: &Path, force_bios: bool) -> Result<()> {
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

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-cpu", "Skylake-Client",
        "-cdrom", iso_path.to_str().unwrap(),
        "-m", "512M",
        "-vga", "std",
    ]);

    // UEFI boot by default (it's 2026), unless --bios is specified
    if force_bios {
        println!("  Boot: BIOS (legacy)");
    } else if let Some(ovmf_path) = find_ovmf() {
        println!("  Boot: UEFI ({})", ovmf_path.display());
        cmd.args([
            "-drive",
            &format!(
                "if=pflash,format=raw,readonly=on,file={}",
                ovmf_path.display()
            ),
        ]);
    } else {
        println!("  Boot: BIOS (OVMF not found, install edk2-ovmf for UEFI)");
    }

    let status = cmd
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
