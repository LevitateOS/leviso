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

use crate::process::{shell, Cmd};
use distro_spec::levitate::{
    // Modules
    BOOT_MODULES,
    // Init script generation
    BOOT_DEVICE_PROBE_ORDER,
    ISO_LABEL,
    LIVE_OVERLAY_ISO_PATH,
    SQUASHFS_ISO_PATH,
    // Build paths
    BUSYBOX_URL,
    BUSYBOX_URL_ENV,
    INITRAMFS_BUILD_DIR,
    INITRAMFS_DIRS,
    INITRAMFS_OUTPUT,
    // Compression
    CPIO_GZIP_LEVEL,
};


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
    let output_cpio = output_dir.join(INITRAMFS_OUTPUT);

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

    // Build cpio archive
    build_cpio(&initramfs_root, &output_cpio)?;

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
/// Rocky kernel has these as modules, not built-in:
/// - CDROM: sr_mod, cdrom, isofs, virtio_scsi
/// - Filesystems: loop, squashfs, overlay
fn copy_boot_modules(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Copying boot kernel modules...");

    let rootfs = base_dir.join("downloads/rootfs");

    // Find the kernel modules directory
    let modules_dir = rootfs.join("usr/lib/modules");
    if !modules_dir.exists() {
        // FAIL FAST - CDROM modules are REQUIRED for ISO boot on the Rocky kernel.
        // The Rocky kernel has CDROM support as modules (sr_mod, cdrom, isofs).
        // Without these, the initramfs cannot mount the ISO.
        // DO NOT change this to a warning.
        bail!(
            "No kernel modules found at {}.\n\
             \n\
             CDROM kernel modules (sr_mod, cdrom, isofs) are REQUIRED.\n\
             The Rocky kernel has CDROM support as modules, not built-in.\n\
             Without them, the ISO cannot boot.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            modules_dir.display()
        );
    }

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

    // Copy each boot module - ALL are required
    let mut copied = 0;
    let mut missing = Vec::new();
    for module in BOOT_MODULES {
        let src = kmod_src.join(module);
        if src.exists() {
            let dst = kmod_dst.join(module);
            fs::create_dir_all(dst.parent().unwrap())?;
            fs::copy(&src, &dst)?;
            copied += 1;
        } else {
            missing.push(*module);
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

    println!("  Copied {}/{} boot modules", copied, BOOT_MODULES.len());
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
fn build_cpio(root: &Path, output: &Path) -> Result<()> {
    println!("Building cpio archive...");

    // Use find + cpio to create the archive
    let cpio_cmd = format!(
        "cd {} && find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -{} > {}",
        root.display(),
        CPIO_GZIP_LEVEL,
        output.display()
    );

    shell(&cpio_cmd)?;

    Ok(())
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
        .map(|m| m.trim_end_matches(".ko.xz").trim_end_matches(".ko.gz"))
        .collect();

    Ok(template
        .replace("{{ISO_LABEL}}", ISO_LABEL)
        .replace("{{SQUASHFS_PATH}}", &format!("/{}", SQUASHFS_ISO_PATH))
        .replace("{{BOOT_MODULES}}", &module_names.join(" "))
        .replace("{{BOOT_DEVICES}}", &BOOT_DEVICE_PROBE_ORDER.join(" "))
        .replace(
            "{{LIVE_OVERLAY_PATH}}",
            &format!("/{}", LIVE_OVERLAY_ISO_PATH),
        ))
}
