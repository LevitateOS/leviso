use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "levitateiso")]
#[command(about = "LevitateOS ISO builder - creates bootable ISO from Rocky Linux packages")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a LevitateOS ISO
    Build {
        /// Path to Rocky Linux minimal ISO
        #[arg(long, default_value = "vendor/images/Rocky-10-latest-x86_64-minimal.iso")]
        source_iso: PathBuf,

        /// Output ISO path
        #[arg(long, short, default_value = "out/levitateos.iso")]
        output: PathBuf,

        /// Profile directory containing packages.txt and airootfs
        #[arg(long, default_value = "profile")]
        profile: PathBuf,

        /// Working directory (will be created/cleaned)
        #[arg(long, default_value = "work")]
        work_dir: PathBuf,

        /// Keep work directory after build
        #[arg(long)]
        keep_work: bool,
    },
    /// List packages in Rocky ISO
    ListPackages {
        /// Path to Rocky Linux ISO
        #[arg(default_value = "vendor/images/Rocky-10-latest-x86_64-minimal.iso")]
        iso: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            source_iso,
            output,
            profile,
            work_dir,
            keep_work,
        } => build_iso(&source_iso, &output, &profile, &work_dir, keep_work),
        Commands::ListPackages { iso } => list_packages(&iso),
    }
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<()> {
    println!("==> Running: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute {}", cmd))?;

    if !status.success() {
        bail!("Command failed: {} {}", cmd, args.join(" "));
    }
    Ok(())
}

fn check_dependencies() -> Result<()> {
    let deps = ["mount", "umount", "rpm", "mksquashfs", "xorriso", "rsync"];

    for dep in deps {
        if Command::new("which")
            .arg(dep)
            .stdout(Stdio::null())
            .status()?
            .success()
        {
            continue;
        }
        bail!(
            "Missing dependency: {}. Install it with your package manager.",
            dep
        );
    }

    // Check if running as root (needed for mount, rpm --root, etc.)
    if !nix_check_root() {
        bail!("levitateiso must be run as root (needed for mount, rpm --root, etc.)");
    }

    Ok(())
}

fn nix_check_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn build_iso(
    source_iso: &Path,
    output: &Path,
    profile: &Path,
    work_dir: &Path,
    keep_work: bool,
) -> Result<()> {
    println!("LevitateOS ISO Builder");
    println!("======================");
    println!();

    check_dependencies()?;

    // Validate inputs
    if !source_iso.exists() {
        bail!("Source ISO not found: {}", source_iso.display());
    }
    if !profile.exists() {
        bail!("Profile directory not found: {}", profile.display());
    }

    let packages_txt = profile.join("packages.txt");
    if !packages_txt.exists() {
        bail!("packages.txt not found in profile: {}", packages_txt.display());
    }

    // Setup directories
    let iso_mount = work_dir.join("iso_mount");
    let rootfs = work_dir.join("rootfs");
    let iso_out = work_dir.join("iso");

    // Clean work directory
    if work_dir.exists() {
        println!("==> Cleaning work directory...");
        // Unmount if mounted
        let _ = Command::new("umount").arg(&iso_mount).status();
        fs::remove_dir_all(work_dir)?;
    }

    fs::create_dir_all(&iso_mount)?;
    fs::create_dir_all(&rootfs)?;
    fs::create_dir_all(iso_out.join("LiveOS"))?;
    fs::create_dir_all(iso_out.join("boot/grub2"))?;
    fs::create_dir_all(iso_out.join("EFI/BOOT"))?;

    // Create output directory
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // Mount source ISO
    println!("==> Mounting source ISO...");
    run_cmd(
        "mount",
        &[
            "-o",
            "loop,ro",
            source_iso.to_str().unwrap(),
            iso_mount.to_str().unwrap(),
        ],
    )?;

    // Find packages directory
    let packages_dir = find_packages_dir(&iso_mount)?;
    println!("==> Found packages at: {}", packages_dir.display());

    // Read package list
    let packages = read_packages(&packages_txt)?;
    println!("==> Installing {} packages...", packages.len());

    // Create basic directory structure for rpm
    create_rpm_dirs(&rootfs)?;

    // Install packages
    for pkg in &packages {
        if let Err(e) = install_package(&packages_dir, &rootfs, pkg) {
            eprintln!("Warning: Failed to install {}: {}", pkg, e);
        }
    }

    // Apply airootfs overlay
    let airootfs = profile.join("airootfs");
    if airootfs.exists() {
        println!("==> Applying airootfs overlay...");
        copy_tree(&airootfs, &rootfs)?;
    }

    // Make installer executable
    let installer = rootfs.join("usr/local/bin/levitate-installer");
    if installer.exists() {
        run_cmd("chmod", &["+x", installer.to_str().unwrap()])?;
    }

    // Set root password to empty (for live environment)
    println!("==> Configuring root account...");
    configure_root_account(&rootfs)?;

    // Generate initramfs with dracut (inside chroot)
    println!("==> Generating initramfs with live boot support...");
    generate_initramfs(&rootfs)?;

    // Find kernel version
    let kernel_version = find_kernel_version(&rootfs)?;
    println!("==> Found kernel version: {}", kernel_version);

    // Create squashfs
    println!("==> Creating squashfs image (this may take a while)...");
    let squashfs_path = iso_out.join("LiveOS/squashfs.img");
    run_cmd(
        "mksquashfs",
        &[
            rootfs.to_str().unwrap(),
            squashfs_path.to_str().unwrap(),
            "-comp",
            "xz",
            "-Xbcj",
            "x86",
            "-b",
            "1M",
            "-noappend",
        ],
    )?;

    // Copy kernel and initramfs to ISO
    println!("==> Copying kernel and initramfs...");
    let vmlinuz_src = rootfs.join(format!("boot/vmlinuz-{}", kernel_version));
    let initramfs_src = rootfs.join("boot/initramfs-live.img");

    fs::copy(&vmlinuz_src, iso_out.join("boot/vmlinuz"))
        .with_context(|| format!("Failed to copy kernel from {}", vmlinuz_src.display()))?;
    fs::copy(&initramfs_src, iso_out.join("boot/initramfs.img"))
        .with_context(|| format!("Failed to copy initramfs from {}", initramfs_src.display()))?;

    // Copy boot templates
    println!("==> Setting up bootloader...");
    let templates = Path::new("templates");
    if templates.join("grub.cfg").exists() {
        fs::copy(templates.join("grub.cfg"), iso_out.join("boot/grub2/grub.cfg"))?;
    }

    // Setup EFI boot
    setup_efi_boot(&rootfs, &iso_out)?;

    // Unmount source ISO
    println!("==> Unmounting source ISO...");
    run_cmd("umount", &[iso_mount.to_str().unwrap()])?;

    // Create final ISO
    println!("==> Creating ISO image...");
    create_iso(&iso_out, output)?;

    // Cleanup
    if !keep_work {
        println!("==> Cleaning up work directory...");
        fs::remove_dir_all(work_dir)?;
    }

    println!();
    println!("========================================");
    println!("ISO created successfully: {}", output.display());
    println!("========================================");
    println!();
    println!("Test with:");
    println!(
        "  qemu-system-x86_64 -enable-kvm -m 4G -cdrom {}",
        output.display()
    );

    Ok(())
}

fn find_packages_dir(iso_mount: &Path) -> Result<PathBuf> {
    // Rocky 10 minimal ISO structure: Minimal/Packages/{a-z}/*.rpm
    // or BaseOS/Packages/{a-z}/*.rpm
    let candidates = [
        iso_mount.join("Minimal/Packages"),
        iso_mount.join("BaseOS/Packages"),
        iso_mount.join("Packages"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    bail!(
        "Could not find Packages directory in ISO. Checked: {:?}",
        candidates
    );
}

fn read_packages(packages_txt: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(packages_txt)?;
    let packages: Vec<String> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();
    Ok(packages)
}

fn create_rpm_dirs(rootfs: &Path) -> Result<()> {
    // Create minimal directory structure for rpm --root
    let dirs = [
        "var/lib/rpm",
        "etc",
        "usr/bin",
        "usr/sbin",
        "usr/lib",
        "usr/lib64",
        "bin",
        "sbin",
        "lib",
        "lib64",
    ];

    for dir in dirs {
        fs::create_dir_all(rootfs.join(dir))?;
    }

    // Initialize RPM database
    run_cmd(
        "rpm",
        &["--root", rootfs.to_str().unwrap(), "--initdb"],
    )?;

    Ok(())
}

fn install_package(packages_dir: &Path, rootfs: &Path, package: &str) -> Result<()> {
    // Find the RPM file
    let rpm_path = find_rpm(packages_dir, package)?;
    println!("    Installing: {} ({})", package, rpm_path.file_name().unwrap().to_string_lossy());

    // Install with rpm
    // Using --nodeps because we're installing in dependency order from packages.txt
    // and some deps might be virtual packages
    run_cmd(
        "rpm",
        &[
            "--root",
            rootfs.to_str().unwrap(),
            "-ivh",
            "--nodeps",
            "--noscripts",
            rpm_path.to_str().unwrap(),
        ],
    )?;

    Ok(())
}

fn find_rpm(packages_dir: &Path, package: &str) -> Result<PathBuf> {
    // Search in subdirectories (Rocky uses a-z subdirs)
    for entry in WalkDir::new(packages_dir).max_depth(2) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            // Match package-version.arch.rpm, avoiding similar names
            // e.g., "bash" should match "bash-5.2.26-6.el10.x86_64.rpm"
            // but not "bash-completion-..."
            if name.starts_with(&format!("{}-", package))
                && name.ends_with(".rpm")
                && !name.contains("-devel-")
                && !name.contains("-doc-")
                && !name.contains("-debuginfo-")
            {
                // Verify it's the right package (not a subpackage)
                let parts: Vec<&str> = name.strip_suffix(".rpm").unwrap().split('-').collect();
                if parts.first() == Some(&package)
                    || (parts.len() >= 2 && parts[..parts.len() - 2].join("-") == package)
                {
                    return Ok(entry.path().to_path_buf());
                }
            }
        }
    }

    bail!("RPM not found for package: {}", package);
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    run_cmd(
        "rsync",
        &["-av", src.to_str().unwrap(), dst.to_str().unwrap()],
    )?;
    Ok(())
}

fn configure_root_account(rootfs: &Path) -> Result<()> {
    // Set empty root password for live environment
    let shadow_path = rootfs.join("etc/shadow");
    if shadow_path.exists() {
        let content = fs::read_to_string(&shadow_path)?;
        let new_content = content
            .lines()
            .map(|line| {
                if line.starts_with("root:") {
                    // Set empty password (root::...)
                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        format!("root::{}", parts[2])
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&shadow_path, new_content + "\n")?;
    }
    Ok(())
}

fn generate_initramfs(rootfs: &Path) -> Result<()> {
    // Find kernel version
    let kernel_version = find_kernel_version(rootfs)?;

    // We need to generate initramfs with live boot support
    // Using dracut with dmsquash-live module

    // First, bind mount necessary filesystems for chroot
    let dev = rootfs.join("dev");
    let proc = rootfs.join("proc");
    let sys = rootfs.join("sys");

    fs::create_dir_all(&dev)?;
    fs::create_dir_all(&proc)?;
    fs::create_dir_all(&sys)?;

    run_cmd("mount", &["--bind", "/dev", dev.to_str().unwrap()])?;
    run_cmd("mount", &["--bind", "/proc", proc.to_str().unwrap()])?;
    run_cmd("mount", &["--bind", "/sys", sys.to_str().unwrap()])?;

    // Generate initramfs with dracut
    let result = Command::new("chroot")
        .arg(rootfs)
        .args([
            "dracut",
            "--force",
            "--add", "dmsquash-live",
            "--omit", "plymouth",
            "--no-hostonly",
            "--no-hostonly-cmdline",
            "/boot/initramfs-live.img",
            &kernel_version,
        ])
        .status();

    // Cleanup mounts (even if dracut failed)
    let _ = Command::new("umount").arg(sys.to_str().unwrap()).status();
    let _ = Command::new("umount").arg(proc.to_str().unwrap()).status();
    let _ = Command::new("umount").arg(dev.to_str().unwrap()).status();

    // Check dracut result
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => bail!("dracut failed to generate initramfs"),
        Err(e) => bail!("Failed to run dracut: {}", e),
    }
}

fn find_kernel_version(rootfs: &Path) -> Result<String> {
    let modules_dir = rootfs.join("lib/modules");
    if !modules_dir.exists() {
        bail!("No kernel modules found in rootfs");
    }

    for entry in fs::read_dir(&modules_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("el10") || name.contains("fc") {
                return Ok(name);
            }
        }
    }

    // Fallback: just use the first directory
    for entry in fs::read_dir(&modules_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            return Ok(entry.file_name().to_string_lossy().to_string());
        }
    }

    bail!("Could not determine kernel version");
}

fn setup_efi_boot(rootfs: &Path, iso_out: &Path) -> Result<()> {
    // Copy EFI bootloader files
    let efi_dir = iso_out.join("EFI/BOOT");

    // Look for shim and grub in the rootfs
    let shim_src = rootfs.join("boot/efi/EFI/rocky/shimx64.efi");
    let grub_src = rootfs.join("boot/efi/EFI/rocky/grubx64.efi");

    // Alternative locations
    let shim_alt = rootfs.join("usr/share/shim/x64/shimx64.efi");
    let grub_alt = rootfs.join("usr/lib/grub/x86_64-efi/grub.efi");

    if shim_src.exists() {
        fs::copy(&shim_src, efi_dir.join("BOOTX64.EFI"))?;
    } else if shim_alt.exists() {
        fs::copy(&shim_alt, efi_dir.join("BOOTX64.EFI"))?;
    } else {
        eprintln!("Warning: shim EFI bootloader not found, EFI boot may not work");
    }

    if grub_src.exists() {
        fs::copy(&grub_src, efi_dir.join("grubx64.efi"))?;
    } else if grub_alt.exists() {
        fs::copy(&grub_alt, efi_dir.join("grubx64.efi"))?;
    }

    // Copy GRUB config for EFI
    let grub_cfg = iso_out.join("boot/grub2/grub.cfg");
    if grub_cfg.exists() {
        fs::create_dir_all(efi_dir.join("grub2"))?;
        fs::copy(&grub_cfg, efi_dir.join("grub2/grub.cfg"))?;
    }

    Ok(())
}

fn create_iso(iso_dir: &Path, output: &Path) -> Result<()> {
    // Create hybrid ISO (bootable via BIOS and UEFI)
    run_cmd(
        "xorriso",
        &[
            "-as", "mkisofs",
            "-o", output.to_str().unwrap(),
            "-V", "LEVITATEOS",  // Volume label (must match grub.cfg root=live:CDLABEL=)
            "-J", "-joliet-long",
            "-r",
            "-iso-level", "3",
            // EFI boot
            "-eltorito-alt-boot",
            "-e", "EFI/BOOT/BOOTX64.EFI",
            "-no-emul-boot",
            "-isohybrid-gpt-basdat",
            // Output directory
            iso_dir.to_str().unwrap(),
        ],
    )?;

    Ok(())
}

fn list_packages(iso: &Path) -> Result<()> {
    if !iso.exists() {
        bail!("ISO not found: {}", iso.display());
    }

    let mount_dir = tempfile::tempdir()?;
    let mount_path = mount_dir.path();

    println!("Mounting ISO...");
    run_cmd(
        "mount",
        &["-o", "loop,ro", iso.to_str().unwrap(), mount_path.to_str().unwrap()],
    )?;

    let packages_dir = find_packages_dir(mount_path)?;

    println!("\nPackages in {}:", iso.display());
    println!("================");

    let mut packages: Vec<String> = Vec::new();
    for entry in WalkDir::new(&packages_dir).max_depth(2) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "rpm") {
            packages.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    packages.sort();
    for pkg in packages {
        println!("  {}", pkg);
    }

    run_cmd("umount", &[mount_path.to_str().unwrap()])?;

    Ok(())
}
