//! Tiny initramfs builder (~5MB).
//!
//! Creates a minimal initramfs containing only:
//! - Static busybox binary (~1MB)
//! - /init script (shell script that mounts squashfs)
//! - Minimal directory structure
//!
//! # Key Insight: No Modules Needed
//!
//! The kernel has these features built-in (CONFIG_*=y, not =m):
//! - CONFIG_SQUASHFS=y (squashfs filesystem)
//! - CONFIG_BLK_DEV_LOOP=y (loop device for mounting squashfs)
//! - CONFIG_OVERLAY_FS=y (overlay filesystem)
//!
//! No modprobe needed! The init script just mounts.
//!
//! # Boot Flow
//!
//! ```text
//! 1. GRUB loads kernel + this initramfs
//! 2. Kernel extracts initramfs to rootfs, runs /init
//! 3. /init (busybox sh script):
//!    a. Mount /proc, /sys, /dev
//!    b. Find boot device by LABEL=LEVITATEOS
//!    c. Mount ISO read-only
//!    d. Mount filesystem.squashfs via loop device
//!    e. Create overlay: squashfs (lower) + tmpfs (upper)
//!    f. switch_root to overlay
//! 4. systemd (PID 1) takes over
//! ```

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use distro_builder::process::{shell, Cmd};
use distro_builder::build_cpio;
use distro_spec::levitate::{
    // Modules
    BOOT_MODULES,
    // Init script generation
    BOOT_DEVICE_PROBE_ORDER,
    ISO_LABEL,
    LIVE_OVERLAY_ISO_PATH,
    ROOTFS_ISO_PATH,
    // Build paths
    BUSYBOX_URL,
    BUSYBOX_URL_ENV,
    INITRAMFS_BUILD_DIR,
    INITRAMFS_DIRS,
    INITRAMFS_LIVE_OUTPUT,
    INITRAMFS_INSTALLED_OUTPUT,
    // Compression
    CPIO_GZIP_LEVEL,
};
use distro_spec::shared::chroot::CHROOT_BIND_MOUNTS;


/// Get busybox download URL from environment or use default.
fn busybox_url() -> String {
    env::var(BUSYBOX_URL_ENV).unwrap_or_else(|_| BUSYBOX_URL.to_string())
}

/// Commands to symlink from busybox.
const BUSYBOX_COMMANDS: &[&str] = &[
    "sh", "mount", "umount", "mkdir", "cat", "ls", "sleep", "switch_root", "echo", "test", "[",
    "grep", "sed", "ln", "rm", "cp", "mv", "chmod", "chown", "mknod", "losetup", "mount.loop",
    "insmod", "modprobe", "xz", "gunzip", "find", "head",  // For module loading
];

// BOOT_MODULES moved to distro-spec

/// Build the tiny initramfs.
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Tiny Initramfs ===\n");

    let output_dir = base_dir.join("output");
    let initramfs_root = output_dir.join(INITRAMFS_BUILD_DIR);
    let output_cpio = output_dir.join(INITRAMFS_LIVE_OUTPUT);

    // Clean previous build
    if initramfs_root.exists() {
        fs::remove_dir_all(&initramfs_root)?;
    }

    // Create minimal directory structure
    create_directory_structure(&initramfs_root)?;

    // Copy/download busybox
    copy_busybox(base_dir, &initramfs_root)?;

    // Copy CDROM kernel modules (needed for Rocky kernel)
    copy_boot_modules(base_dir, &initramfs_root)?;

    // Create init script
    create_init_script(base_dir, &initramfs_root)?;

    // Build cpio archive to a temporary file (Atomic Artifacts)
    let temp_cpio = output_dir.join(format!("{}.tmp", INITRAMFS_LIVE_OUTPUT));
    build_cpio_archive(&initramfs_root, &temp_cpio)?;

    // Verify the temporary artifact is valid (could extend this with cpio check)
    if !temp_cpio.exists() || fs::metadata(&temp_cpio)?.len() < 1024 {
        bail!("Initramfs build produced invalid or empty file");
    }

    // Atomic rename to final destination
    fs::rename(&temp_cpio, &output_cpio)?;

    let size = fs::metadata(&output_cpio)?.len();
    println!("\n=== Tiny Initramfs Complete ===");
    println!("  Output: {}", output_cpio.display());
    println!("  Size: {} KB", size / 1024);

    Ok(())
}

/// Create minimal directory structure.
fn create_directory_structure(root: &Path) -> Result<()> {
    println!("Creating directory structure...");

    let dirs = INITRAMFS_DIRS;

    for dir in dirs {
        fs::create_dir_all(root.join(dir))?;
    }

    // Create essential device nodes (some kernels need these before devtmpfs)
    // Note: These are created by devtmpfs mount, but having them doesn't hurt
    create_device_nodes(root)?;

    Ok(())
}

/// Create essential device nodes.
fn create_device_nodes(root: &Path) -> Result<()> {
    // We'll let devtmpfs handle this - just ensure /dev exists
    let dev = root.join("dev");
    fs::create_dir_all(&dev)?;

    // Create a note file explaining that devtmpfs creates nodes
    fs::write(
        dev.join(".note"),
        "# Device nodes are created by devtmpfs mount in /init\n",
    )?;

    Ok(())
}

/// Download or copy busybox static binary.
fn copy_busybox(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Setting up busybox...");

    let downloads_dir = base_dir.join("downloads");
    let busybox_cache = downloads_dir.join("busybox-static");
    let busybox_dst = initramfs_root.join("bin/busybox");

    // Download if not cached
    if !busybox_cache.exists() {
        let url = busybox_url();
        println!("  Downloading static busybox from {}", url);
        fs::create_dir_all(&downloads_dir)?;

        Cmd::new("curl")
            .args(["-L", "-o"])
            .arg_path(&busybox_cache)
            .args(["--progress-bar", &url])
            .error_msg("Failed to download busybox. Install: sudo dnf install curl")
            .run_interactive()?;
    }

    // Copy to initramfs
    fs::copy(&busybox_cache, &busybox_dst)?;

    // Make executable
    let mut perms = fs::metadata(&busybox_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&busybox_dst, perms)?;

    // Create symlinks for common commands
    println!("  Creating busybox symlinks...");
    for cmd in BUSYBOX_COMMANDS {
        let link = initramfs_root.join("bin").join(cmd);
        if !link.exists() {
            std::os::unix::fs::symlink("busybox", &link)?;
        }
    }

    println!("  Busybox ready ({} commands)", BUSYBOX_COMMANDS.len());
    Ok(())
}

/// Copy boot kernel modules to the initramfs.
///
/// For CUSTOM kernels: Boot-critical modules (squashfs, overlay, loop) are built-in.
///                     Only non-essential modules need to be copied.
/// For ROCKY kernels: All boot modules are loadable and must be copied.
fn copy_boot_modules(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Copying boot kernel modules...");

    // PRIORITY 1: Check for custom-built kernel modules in output/staging
    let custom_modules_path = base_dir.join("output/staging/usr/lib/modules");
    let rocky_modules_path = base_dir.join("downloads/rootfs/usr/lib/modules");
    let vmlinuz_path = base_dir.join("output/staging/boot/vmlinuz");

    let (modules_dir, is_custom_kernel) = if custom_modules_path.exists() {
        // ANTI-CHEAT: Ensure the kernel binary ACTUALLY exists if we use custom modules
        if !vmlinuz_path.exists() {
            bail!("Custom modules found but vmlinuz is missing from staging.\n\
                   This indicates a broken or partial kernel build.\n\
                   Refusing to build initramfs with half-built kernel.");
        }
        println!("  Using CUSTOM kernel modules from {}", custom_modules_path.display());
        (custom_modules_path, true)
    } else if rocky_modules_path.exists() {
        println!("  Using ROCKY kernel modules from {}", rocky_modules_path.display());
        (rocky_modules_path, false)
    } else {
        bail!(
            "No kernel modules found. Expected at:\n\
             - {}\n\
             - {}\n\
             \n\
             CDROM kernel modules (sr_mod, cdrom, isofs) are REQUIRED.\n\
             Without them, the ISO cannot boot.\n\
             \n\
             Run 'leviso build kernel' or 'leviso extract rocky' first.",
            custom_modules_path.display(),
            rocky_modules_path.display()
        );
    };

    // Find the kernel version directory (e.g., 6.12.0-124.8.1.el10_1.x86_64)
    let kernel_version = fs::read_dir(&modules_dir)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string());

    let Some(kver) = kernel_version else {
        // FAIL FAST - we found the modules directory but no kernel version inside.
        // This is a corrupted or incomplete rootfs extraction.
        // DO NOT change this to a warning.
        bail!(
            "No kernel version directory found in {}.\n\
             \n\
             The modules directory exists but contains no kernel version.\n\
             This indicates a corrupted or incomplete rootfs extraction.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            modules_dir.display()
        );
    };

    let kmod_src = modules_dir.join(&kver);
    let kmod_dst = initramfs_root.join("lib/modules").join(&kver);
    fs::create_dir_all(&kmod_dst)?;

    // Load modules.builtin to check for built-in modules (custom kernel only)
    let builtin_modules: std::collections::HashSet<String> = if is_custom_kernel {
        let builtin_path = kmod_src.join("modules.builtin");
        if builtin_path.exists() {
            fs::read_to_string(&builtin_path)?
                .lines()
                .map(|s| s.to_string())
                .collect()
        } else {
            std::collections::HashSet::new()
        }
    } else {
        std::collections::HashSet::new()
    };

    // Copy each boot module - ALL are required (unless built-in)
    let mut copied = 0;
    let mut builtin_count = 0;
    let mut missing = Vec::new();
    for module_path in BOOT_MODULES {
        // Try to find the module with different extensions
        // distro-spec now provides extension-less paths, but we handle both just in case
        let base_path = module_path.trim_end_matches(".ko.xz").trim_end_matches(".ko.gz").trim_end_matches(".ko");

        // Check if module is built-in (for custom kernels)
        let builtin_key = format!("{}.ko", base_path);
        if is_custom_kernel && builtin_modules.contains(&builtin_key) {
            builtin_count += 1;
            continue; // Module is compiled into kernel, no file to copy
        }

        let mut found = false;
        for ext in [".ko", ".ko.xz", ".ko.gz"] {
            let src = kmod_src.join(format!("{}{}", base_path, ext));
            if src.exists() {
                let dst = kmod_dst.join(format!("{}{}", base_path, ext));
                fs::create_dir_all(dst.parent().unwrap())?;
                fs::copy(&src, &dst)?;
                copied += 1;
                found = true;
                break;
            }
        }

        if !found {
            missing.push(*module_path);
        }
    }

    // FAIL FAST if any module is missing - ALL are required for boot
    if !missing.is_empty() {
        bail!(
            "Boot modules missing: {:?}\n\
             \n\
             These kernel modules are REQUIRED for the ISO to boot:\n\
             - cdrom, sr_mod, virtio_scsi, isofs (CDROM access)\n\
             - loop, squashfs, overlay (squashfs + overlay boot)\n\
             \n\
             Without ALL of these, the initramfs cannot mount the squashfs.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            missing
        );
    }

    if builtin_count > 0 {
        println!("  {} boot modules are built-in to kernel (no copy needed)", builtin_count);
    }
    if copied > 0 {
        println!("  Copied {} boot modules", copied);
    }
    Ok(())
}

/// Create the init script.
///
/// FAIL FAST: profile/init_tiny is REQUIRED.
/// We do not maintain a fallback because:
/// 1. The init script has critical three-layer overlay logic
/// 2. The fallback would quickly become out of sync
/// 3. A silent fallback to a broken init is worse than failing
fn create_init_script(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Creating init script from template...");

    let init_content = generate_init_script(base_dir)?;
    let init_dst = initramfs_root.join("init");

    fs::write(&init_dst, &init_content)?;

    // Make executable
    let mut perms = fs::metadata(&init_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_dst, perms)?;

    Ok(())
}

/// Build the cpio archive from initramfs root.
fn build_cpio_archive(root: &Path, output: &Path) -> Result<()> {
    println!("Building cpio archive...");
    build_cpio(root, output, CPIO_GZIP_LEVEL)
}

/// Build a full initramfs for installed systems using dracut.
///
/// This is different from the "tiny" initramfs used for live boot:
/// - Tiny initramfs: Mounts squashfs from ISO, uses busybox
/// - Install initramfs: Boots from real disk, uses dracut with full systemd
///
/// By pre-building this during ISO creation, we save 2-3 minutes during installation.
/// The initramfs is generic (no hostonly) so it works on any hardware.
pub fn build_install_initramfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Install Initramfs (dracut) ===\n");

    let output_dir = base_dir.join("output");
    let squashfs_root = output_dir.join("squashfs-root");
    let output_file = output_dir.join(INITRAMFS_INSTALLED_OUTPUT);

    // Verify squashfs-root exists (we need dracut and kernel modules from it)
    if !squashfs_root.exists() {
        bail!(
            "squashfs-root not found at {}.\n\
             Run 'leviso build squashfs' first.",
            squashfs_root.display()
        );
    }

    // Find kernel version from modules directory
    let modules_dir = squashfs_root.join("usr/lib/modules");
    let kernel_version = fs::read_dir(&modules_dir)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .context("No kernel modules found in squashfs-root")?;

    println!("  Kernel version: {}", kernel_version);

    // Verify dracut exists in squashfs-root
    let dracut_bin = squashfs_root.join("usr/bin/dracut");
    if !dracut_bin.exists() {
        bail!(
            "dracut not found in squashfs-root at {}.\n\
             The base system must include dracut.",
            dracut_bin.display()
        );
    }

    // Build initramfs using dracut in a chroot environment
    println!("  Running dracut to build install initramfs...");

    // Create output path inside squashfs-root (dracut writes there, we copy out)
    let chroot_output = format!("/tmp/{}", INITRAMFS_INSTALLED_OUTPUT);

    // Run dracut with same options as installation would use
    // --no-hostonly: Include all drivers, not just current hardware
    // --omit: Skip modules that have missing deps or aren't needed
    let dracut_cmd = format!(
        "dracut --force --no-hostonly \
         --omit 'fips bluetooth crypt nfs rdma systemd-sysusers systemd-journald systemd-initrd dracut-systemd' \
         {} {}",
        chroot_output, kernel_version
    );

    // Run dracut in chroot with proper bind mounts
    // Dracut needs /proc, /sys, /dev to detect hardware and build initramfs
    let squashfs_root_str = squashfs_root.to_str()
        .context("squashfs_root path is not valid UTF-8")?;

    run_in_chroot(squashfs_root_str, &dracut_cmd)?;

    // Copy the generated initramfs out of squashfs-root
    let generated_initramfs = squashfs_root.join(format!("tmp/{}", INITRAMFS_INSTALLED_OUTPUT));
    if !generated_initramfs.exists() {
        bail!(
            "dracut did not create initramfs at {}",
            generated_initramfs.display()
        );
    }

    fs::copy(&generated_initramfs, &output_file)?;
    fs::remove_file(&generated_initramfs)?; // Clean up

    let size = fs::metadata(&output_file)?.len();
    println!("\n=== Install Initramfs Complete ===");
    println!("  Output: {}", output_file.display());
    println!("  Size: {} MB", size / 1024 / 1024);

    Ok(())
}

/// Run a command in a chroot environment with proper bind mounts.
///
/// Sets up /dev, /dev/pts, /proc, /sys, /run before entering chroot,
/// and cleans up afterwards (even on failure).
fn run_in_chroot(chroot_root: &str, command: &str) -> Result<()> {
    // Mounts we'll set up (subset of CHROOT_BIND_MOUNTS - only what dracut needs)
    // We skip efivars since it may not exist and dracut doesn't need it
    let mounts_to_setup: Vec<_> = CHROOT_BIND_MOUNTS
        .iter()
        .filter(|m| !m.source.contains("efivars"))
        .collect();

    // Create mount point directories
    for mount in &mounts_to_setup {
        let target = format!("{}{}", chroot_root, mount.target);
        if let Err(e) = fs::create_dir_all(&target) {
            // Only fail if directory doesn't exist after attempted creation
            if !std::path::Path::new(&target).exists() {
                bail!("Failed to create mount point {}: {}", target, e);
            }
            // Otherwise, error was likely "already exists" or similar - safe to ignore
        }
    }

    // Set up bind mounts
    println!("  Setting up chroot environment...");
    let mut mounted: Vec<String> = Vec::new();

    for mount in &mounts_to_setup {
        let target = mount.full_target(chroot_root);

        // Check if source exists (e.g., /dev/pts may not exist in minimal environments)
        if !Path::new(mount.source).exists() {
            if mount.required {
                // Clean up already-mounted paths before failing
                cleanup_mounts(&mounted);
                bail!("Required mount source {} does not exist", mount.source);
            }
            continue;
        }

        let mount_result = shell(&mount.mount_command(chroot_root));
        if mount_result.is_err() {
            if mount.required {
                cleanup_mounts(&mounted);
                bail!("Failed to mount {} to {}", mount.source, target);
            }
            // Non-required mount failed, continue
            continue;
        }

        mounted.push(target);
    }

    // Run the command in chroot
    println!("  Running command in chroot...");
    let chroot_result = Cmd::new("chroot")
        .arg(chroot_root)
        .args(["/bin/sh", "-c", command])
        .run();

    // Always clean up mounts, even if command failed
    cleanup_mounts(&mounted);

    // Now check if the command succeeded
    chroot_result.context("Command failed in chroot")?;

    Ok(())
}

/// Unmount paths in reverse order.
/// Logs warnings on failure but doesn't fail - cleanup must complete.
fn cleanup_mounts(mounted: &[String]) {
    for target in mounted.iter().rev() {
        if let Err(e) = shell(&format!("umount {}", target)) {
            eprintln!("  [WARN] Failed to unmount {}: {}", target, e);
            eprintln!("         Stale mounts may break subsequent builds.");
        }
    }
}

/// Generate init script from template with distro-spec values.
fn generate_init_script(base_dir: &Path) -> Result<String> {
    let template_path = base_dir.join("profile/init_tiny.template");
    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("Failed to read init_tiny.template at {}", template_path.display()))?;

    // Extract module names from full paths
    // e.g., "kernel/fs/squashfs/squashfs.ko.xz" -> "squashfs"
    let module_names: Vec<&str> = BOOT_MODULES
        .iter()
        .filter_map(|m| m.rsplit('/').next())
        .map(|m| m.trim_end_matches(".ko.xz").trim_end_matches(".ko.gz").trim_end_matches(".ko"))
        .collect();

    Ok(template
        .replace("{{ISO_LABEL}}", ISO_LABEL)
        .replace("{{ROOTFS_PATH}}", &format!("/{}", ROOTFS_ISO_PATH))
        .replace("{{BOOT_MODULES}}", &module_names.join(" "))
        .replace("{{BOOT_DEVICES}}", &BOOT_DEVICE_PROBE_ORDER.join(" "))
        .replace(
            "{{LIVE_OVERLAY_PATH}}",
            &format!("/{}", LIVE_OVERLAY_ISO_PATH),
        ))
}
