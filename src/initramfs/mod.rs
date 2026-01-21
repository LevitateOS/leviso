//! Initramfs builder module.
//!
//! This module creates a bootable initramfs by:
//! 1. Extracting binaries and libraries from a Rocky Linux rootfs
//! 2. Setting up systemd as init
//! 3. Configuring D-Bus, PAM, Chrony, and NetworkManager
//! 4. Building a cpio archive

pub mod binary;
pub mod chrony;
pub mod context;
pub mod dbus;
pub mod filesystem;
pub mod modules;
pub mod network;
pub mod pam;
pub mod rootfs;
pub mod systemd;
pub mod users;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::Config;
use context::BuildContext;

/// Coreutils binaries to copy.
const COREUTILS: &[&str] = &[
    "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch", "chmod", "chown", "echo", "pwd",
    "head", "tail", "grep", "find", "wc", "sort", "uniq", "uname", "env", "printenv", "clear",
    "sleep", "ln", "readlink", "dirname", "basename",
    // procps-ng utilities (memory/process info)
    "free", "ps", "top", "uptime", "vmstat", "w", "watch", "pgrep", "pkill",
    // Phase 2: disk info
    "lsblk",
    // Phase 3: system config
    "date", "loadkeys",
    // Compression utilities
    "gzip", "gunzip", "xz",
    // Archive utilities (for installation)
    "tar",
    // Text processing (for installation config editing)
    "sed",
    // Systemd utilities
    "timedatectl", "systemctl", "journalctl", "hostnamectl", "localectl",
    // Bootloader installation
    "bootctl",
    // Locale generation
    "localedef",
    // Console
    "agetty", "login",
];

/// Sbin utilities to copy.
const SBIN_UTILS: &[&str] = &[
    "mount", "umount", "hostname", "modprobe", "depmod",
    // Phase 2: disk utilities
    "blkid", "fdisk", "parted", "wipefs", "mkfs.ext4", "mkfs.fat",
    // Non-interactive disk partitioning (for installation)
    "sfdisk",
    // Phase 3: system config
    "chroot", "hwclock",
    // User management (for installation)
    "useradd", "groupadd", "chpasswd",
    // NTP
    "chronyd",
];

/// Build the initramfs.
///
/// This is the main entry point. It orchestrates all the steps needed
/// to create a bootable initramfs from a Rocky Linux rootfs.
pub fn build_initramfs(base_dir: &Path) -> Result<()> {
    // Check host tools first
    rootfs::check_host_tools()?;

    // Load configuration for module list
    let config = Config::load(base_dir);

    let extract_dir = base_dir.join("downloads");
    let output_dir = base_dir.join("output");
    let initramfs_root = output_dir.join("initramfs-root");

    // Find and validate rootfs
    let actual_rootfs = rootfs::find_rootfs(&extract_dir)?;
    println!("Using rootfs from: {}", actual_rootfs.display());
    rootfs::validate_rootfs(&actual_rootfs)?;

    // Clean and create initramfs root
    if initramfs_root.exists() {
        fs::remove_dir_all(&initramfs_root)?;
    }
    fs::create_dir_all(&initramfs_root)?;

    // Create build context
    let ctx = BuildContext::new(actual_rootfs, initramfs_root.clone(), base_dir.to_path_buf());

    // Create FHS directory structure
    filesystem::create_fhs_structure(&ctx.initramfs)?;
    filesystem::create_var_symlinks(&ctx.initramfs)?;

    // Copy bash first (it's required for everything else)
    binary::copy_bash(&ctx)?;

    // Copy coreutils
    for util in COREUTILS {
        binary::copy_binary_with_libs(&ctx, util)?;
    }

    // Copy sbin utilities
    for util in SBIN_UTILS {
        binary::copy_binary_with_libs(&ctx, util)?;
    }

    // Create /bin/sh symlink
    filesystem::create_sh_symlink(&ctx.initramfs)?;

    // Copy keymaps
    filesystem::copy_keymaps(&ctx)?;

    // Set up kernel modules (for disk drivers)
    let module_list = config.all_modules();
    modules::setup_modules(&ctx, &module_list)?;

    // Create root user (must be before dbus adds system users)
    users::create_root_user(&ctx.initramfs)?;

    // Set up systemd as init
    systemd::setup_systemd(&ctx)?;

    // Set up D-Bus (required for systemctl, timedatectl, etc.)
    dbus::setup_dbus(&ctx)?;

    // Set up networking (NetworkManager, wpa_supplicant, WiFi firmware)
    network::setup_network(&ctx)?;

    // Set up Chrony NTP and its user
    chrony::ensure_chrony_user(&ctx)?;
    chrony::setup_chrony(&ctx)?;

    // Copy init script
    filesystem::copy_init_script(&ctx)?;

    // Create shell configuration
    filesystem::create_shell_config(&ctx.initramfs)?;

    // Set up PAM (required for login/agetty)
    pam::setup_pam(&ctx)?;

    // Build cpio archive
    build_cpio_archive(&initramfs_root, &output_dir)?;

    Ok(())
}

/// Build the cpio archive from initramfs root.
fn build_cpio_archive(initramfs_root: &Path, output_dir: &Path) -> Result<()> {
    println!("Building initramfs cpio archive...");
    let initramfs_cpio = output_dir.join("initramfs.cpio.gz");

    let find_output = Command::new("sh")
        .current_dir(initramfs_root)
        .args([
            "-c",
            "find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9",
        ])
        .output()
        .context("Failed to create cpio archive")?;

    if !find_output.status.success() {
        bail!(
            "cpio failed: {}",
            String::from_utf8_lossy(&find_output.stderr)
        );
    }

    fs::write(&initramfs_cpio, &find_output.stdout)?;
    println!("Created initramfs at: {}", initramfs_cpio.display());

    Ok(())
}
