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
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Busybox download URL (static x86_64 build).
const BUSYBOX_URL: &str =
    "https://busybox.net/downloads/binaries/1.35.0-x86_64-linux-musl/busybox";

/// Commands to symlink from busybox.
const BUSYBOX_COMMANDS: &[&str] = &[
    "sh", "mount", "umount", "mkdir", "cat", "ls", "sleep", "switch_root", "echo", "test", "[",
    "grep", "sed", "ln", "rm", "cp", "mv", "chmod", "chown", "mknod", "losetup", "mount.loop",
    "insmod", "modprobe", "xz", "gunzip", "find", "head",  // For module loading
];

/// Kernel modules needed for boot (Rocky kernel has these as modules).
/// Order matters for dependencies.
const BOOT_MODULES: &[&str] = &[
    // CDROM/SCSI support
    "kernel/drivers/cdrom/cdrom.ko.xz",
    "kernel/drivers/scsi/sr_mod.ko.xz",
    "kernel/drivers/scsi/virtio_scsi.ko.xz",  // QEMU virtio-scsi controller
    "kernel/fs/isofs/isofs.ko.xz",
    // Virtio block device (QEMU -drive if=virtio -> /dev/vda)
    "kernel/drivers/block/virtio_blk.ko.xz",
    // Loop device and filesystems for squashfs+overlay boot
    "kernel/drivers/block/loop.ko.xz",
    "kernel/fs/squashfs/squashfs.ko.xz",
    "kernel/fs/overlayfs/overlay.ko.xz",
];

/// Build the tiny initramfs.
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Tiny Initramfs ===\n");

    let output_dir = base_dir.join("output");
    let initramfs_root = output_dir.join("initramfs-tiny-root");
    let output_cpio = output_dir.join("initramfs-tiny.cpio.gz");

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

    let dirs = [
        "bin",
        "dev",
        "proc",
        "sys",
        "tmp",       // Temp files (for module decompression)
        "mnt",       // ISO mount point
        "squashfs",  // Squashfs mount point
        "overlay",   // Overlay work directory
        "newroot",   // Final rootfs
        "lib/modules", // Kernel modules for CDROM
    ];

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
        println!("  Downloading static busybox from {}", BUSYBOX_URL);
        fs::create_dir_all(&downloads_dir)?;

        let status = Command::new("curl")
            .args([
                "-L",
                "-o",
                busybox_cache.to_str().unwrap(),
                "--progress-bar",
                BUSYBOX_URL,
            ])
            .status()
            .context("curl not found. Install curl.")?;

        if !status.success() {
            bail!("Failed to download busybox");
        }
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
fn create_init_script(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Creating init script...");

    let init_src = base_dir.join("profile/init_tiny");
    let init_dst = initramfs_root.join("init");

    if init_src.exists() {
        // Use custom init script from profile/
        fs::copy(&init_src, &init_dst)?;
    } else {
        // Create default init script
        create_default_init_script(&init_dst)?;
    }

    // Make executable
    let mut perms = fs::metadata(&init_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_dst, perms)?;

    Ok(())
}

/// Create the default init script if profile/init_tiny doesn't exist.
fn create_default_init_script(path: &Path) -> Result<()> {
    let script = r#"#!/bin/busybox sh
# LevitateOS Tiny Initramfs
# Mounts squashfs + overlay, then switch_root to live system
#
# REQUIREMENTS:
# - Kernel built with CONFIG_SQUASHFS=y, CONFIG_BLK_DEV_LOOP=y, CONFIG_OVERLAY_FS=y
# - ISO labeled "LEVITATEOS" (set by xorriso -V)
# - Kernel cmdline: root=LABEL=LEVITATEOS

set -e

# Minimal PATH
export PATH=/bin

# Mount essential virtual filesystems
busybox mount -t proc proc /proc
busybox mount -t sysfs sysfs /sys
busybox mount -t devtmpfs devtmpfs /dev

# Load CDROM kernel modules (Rocky kernel has these as modules, not built-in)
# Modules are compressed with xz, so we decompress then insmod
busybox echo "Loading CDROM kernel modules..."
KVER=$(busybox ls /lib/modules 2>/dev/null | busybox head -1)
if [ -n "$KVER" ]; then
    for mod in cdrom sr_mod isofs; do
        modpath="/lib/modules/$KVER/kernel"
        case "$mod" in
            cdrom)  modfile="$modpath/drivers/cdrom/cdrom.ko.xz" ;;
            sr_mod) modfile="$modpath/drivers/scsi/sr_mod.ko.xz" ;;
            isofs)  modfile="$modpath/fs/isofs/isofs.ko.xz" ;;
        esac
        if [ -f "$modfile" ]; then
            busybox xz -d -c "$modfile" > /tmp/$mod.ko 2>/dev/null
            busybox insmod /tmp/$mod.ko 2>/dev/null && busybox echo "  Loaded $mod" || true
            busybox rm -f /tmp/$mod.ko
        fi
    done
fi

# Wait for devices to settle after module load
busybox sleep 1

# Parse cmdline for root= parameter
CMDLINE=$(busybox cat /proc/cmdline)
ROOT_LABEL=""
EMERGENCY=""
for param in $CMDLINE; do
    case "$param" in
        root=LABEL=*) ROOT_LABEL="${param#root=LABEL=}" ;;
        emergency) EMERGENCY=1 ;;
    esac
done

# Default label if not specified
[ -z "$ROOT_LABEL" ] && ROOT_LABEL="LEVITATEOS"

busybox echo "LevitateOS: Searching for boot device with label '$ROOT_LABEL'..."

# Find device by label - check common device names
BOOT_DEV=""
for dev in /dev/sr0 /dev/sda /dev/sda1 /dev/sdb /dev/sdb1 /dev/vda /dev/vda1 /dev/nvme0n1p1 /dev/loop0; do
    [ -b "$dev" ] || continue
    # Try mounting to check for squashfs
    busybox mount -o ro "$dev" /mnt 2>/dev/null || continue

    # Check if this has our squashfs
    if [ -f /mnt/live/filesystem.squashfs ]; then
        BOOT_DEV="$dev"
        busybox echo "Found boot device: $dev"
        break
    fi
    busybox umount /mnt 2>/dev/null
done

if [ -z "$BOOT_DEV" ]; then
    busybox echo "ERROR: Could not find boot device with filesystem.squashfs"
    busybox echo "Kernel cmdline: $CMDLINE"
    busybox echo "Available block devices:"
    busybox ls -la /dev/sd* /dev/sr* /dev/vd* /dev/nvme* 2>/dev/null || true
    busybox echo ""
    busybox echo "Dropping to emergency shell..."
    exec busybox sh
fi

# Emergency shell before continuing?
if [ -n "$EMERGENCY" ]; then
    busybox echo "Emergency shell requested. Type 'exit' to continue boot."
    busybox sh
fi

# Create mount points
busybox mkdir -p /squashfs /overlay /overlay/upper /overlay/work /newroot

# Mount squashfs read-only (via loop - kernel handles this automatically)
busybox echo "Mounting squashfs..."
busybox mount -t squashfs -o ro,loop /mnt/live/filesystem.squashfs /squashfs

# Create overlay: squashfs (read-only lower) + tmpfs (writable upper)
busybox echo "Creating overlay filesystem..."
busybox mount -t tmpfs -o size=50% tmpfs /overlay
busybox mkdir -p /overlay/upper /overlay/work

busybox mount -t overlay overlay \
    -o lowerdir=/squashfs,upperdir=/overlay/upper,workdir=/overlay/work \
    /newroot

# Move virtual filesystems to new root
busybox echo "Preparing switch_root..."
busybox mount --move /dev /newroot/dev
busybox mount --move /proc /newroot/proc
busybox mount --move /sys /newroot/sys

# Keep ISO mounted for recstrap to access squashfs later
busybox mkdir -p /newroot/media/cdrom
busybox mount --move /mnt /newroot/media/cdrom

# Verify init exists
if [ ! -x /newroot/sbin/init ] && [ ! -L /newroot/sbin/init ]; then
    busybox echo "ERROR: /newroot/sbin/init not found or not executable"
    busybox echo "Contents of /newroot/sbin:"
    busybox ls -la /newroot/sbin/
    exec busybox sh
fi

# switch_root to the live system
busybox echo "Switching root to live system..."
exec busybox switch_root /newroot /sbin/init
"#;

    fs::write(path, script)?;
    Ok(())
}

/// Build the cpio archive from initramfs root.
fn build_cpio(root: &Path, output: &Path) -> Result<()> {
    println!("Building cpio archive...");

    // Use find + cpio to create the archive
    let cpio_cmd = format!(
        "cd {} && find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 > {}",
        root.display(),
        output.display()
    );

    let status = Command::new("sh")
        .args(["-c", &cpio_cmd])
        .status()
        .context("Failed to run cpio/gzip")?;

    if !status.success() {
        bail!("cpio/gzip failed");
    }

    Ok(())
}
