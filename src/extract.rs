use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::Path;

use distro_builder::process::{shell_in, Cmd};

/// RPMs to extract and merge into the rootfs.
/// The install.img (Anaconda installer) is missing utilities that users expect.
/// These RPMs supplement the installer rootfs with essential utilities.
const SUPPLEMENTARY_RPMS: &[&str] = &[
    // === COREUTILS (many binaries missing from installer) ===
    // Provides: printf, printenv, whoami, groups, nice, nohup, test, yes, stty, etc.
    "coreutils",
    "coreutils-common",

    // === PROCPS-NG ===
    // Provides: free, vmstat, uptime, w, watch, ps, top, pgrep, pkill
    "procps-ng",

    // === UTILITIES ===
    "which",       // which command
    "file",        // file command
    "file-libs",   // libmagic for file
    "diffutils",   // diff, cmp
    "ncurses",     // clear, tput
    "ncurses-libs",

    // === MISSING CORE UTILITIES ===
    // These are expected by users and scripts but missing from install.img
    "grep",        // grep, egrep, fgrep - pattern matching
    "findutils",   // find, xargs, locate - file finding
    "tar",         // tar - archive extraction (needed for package installation)

    // === AUTH ===
    "sudo",           // sudo, sudoedit, sudoreplay
    "util-linux",     // su, login, etc.
    "util-linux-core", // sulogin, agetty core
    "shadow-utils",   // useradd, userdel, usermod, groupadd, groupdel, groupmod, passwd, chpasswd

    // === NETWORK ===
    "iproute",      // ip, ss, bridge
    "iproute-tc",   // tc (traffic control)

    // === DISK ===
    "parted",       // parted partitioner

    // === KEYBOARD/LOCALE ===
    "kbd",          // loadkeys, setfont
    "kbd-misc",     // keymaps

    // === UDEV ===
    // udevadm should be in systemd-udev which is in the installer

    // === DRACUT (initramfs generator) ===
    // Required for generating initramfs during installation
    "dracut",
    "dracut-config-generic",  // Generic dracut config (no host-only)
    "dracut-network",         // Network support in initramfs

    // === GLIBC EXTRAS ===
    "glibc-common",           // getent command (needed for user/group lookup)

    // === KMOD (module tools) ===
    "kmod",                   // modprobe, lsmod, etc. (dracut dependency)

    // === SQUASHFS-TOOLS (for installation) ===
    // Provides unsquashfs - REQUIRED by recstrap to extract squashfs to disk
    // (mksquashfs is also included in the RPM but we only copy unsquashfs to the image)
    "squashfs-tools",

    // === FIRMWARE ===
    // Intel CPU microcode for security updates (Spectre, Meltdown, etc.)
    // AMD microcode comes from linux-firmware which is already in install.img
    // Without microcode, CPUs are vulnerable to known security exploits
    "microcode_ctl",

    // === SSH (server and client) ===
    // openssh-server: sshd for remote access TO the live ISO
    // openssh-clients: ssh, scp, sftp for remote access FROM the live ISO
    "openssh-server",
    "openssh-clients",

    // === NETWORK TOOLS ===
    // nmtui: TUI for NetworkManager (user-friendly WiFi configuration)
    "NetworkManager-tui",
    "NetworkManager-wifi",   // Added: WiFi support for NetworkManager
    "newt",   // libnewt.so - required by nmtui
    "slang",  // libslang.so - required by newt
    "ethtool", // Added: Network hardware diagnostics
    "wireless-regdb", // Added: Wireless regulatory database

    // === HARDWARE SUPPORT ===
    "bluez",          // Added: Bluetooth stack
    "bluez-libs",
    "pciutils",       // Added: lspci (duplicate but good to have)
    "usbutils",       // Added: lsusb
    "alsa-utils",     // Added: Audio tools (amixer, alsamixer)

    // === BTRFS ===
    "btrfs-progs",    // Btrfs filesystem tools (mkfs.btrfs, btrfs, btrfsck)

    // === WIRELESS ===
    "iwd",            // Alternative WiFi daemon (iwctl, iwmon)

    // === SOUND FIRMWARE ===
    "alsa-sof-firmware", // Intel Sound Open Firmware (modern laptop audio)
    "libdrm",         // Added: Direct Rendering Manager
    "mesa-dri-drivers", // Added: Basic GPU drivers for Intel/AMD

    // === TERMINAL TOOLS ===
    "tmux",           // Added: Terminal multiplexer
    "htop",           // Added: System monitor
    "vim-minimal",    // Added: vim editor (vi is tiny)
];

/// Extract Rocky ISO from a specific path.
///
/// This is the main extraction function used by the dependency resolver.
pub fn extract_rocky_iso(base_dir: &Path, iso_path: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let iso_contents = extract_dir.join("iso-contents");
    let rootfs_dir = extract_dir.join("rootfs");

    if !iso_path.exists() {
        bail!(
            "Rocky DVD ISO not found at {}.",
            iso_path.display()
        );
    }

    // Step 1: Extract ISO contents with 7z
    if !iso_contents.exists() {
        println!("Extracting ISO contents with 7z...");
        fs::create_dir_all(&iso_contents)?;

        Cmd::new("7z")
            .args(["x", "-y"])
            .arg_path(iso_path)
            .arg(format!("-o{}", iso_contents.display()))
            .error_msg("7z extraction failed. Install: sudo dnf install p7zip-plugins")
            .run_interactive()?;
    } else {
        println!("ISO already extracted to {}", iso_contents.display());
    }

    // Step 2: Find and extract squashfs
    if !rootfs_dir.exists() {
        println!("Looking for squashfs...");

        // Rocky 10 uses images/install.img which is a squashfs
        let squashfs_candidates = [
            iso_contents.join("images/install.img"),
            iso_contents.join("LiveOS/squashfs.img"),
            iso_contents.join("LiveOS/rootfs.img"),
        ];

        let squashfs_path = squashfs_candidates
            .iter()
            .find(|p| p.exists())
            .context("Could not find squashfs image in ISO")?;

        println!("Found squashfs at: {}", squashfs_path.display());

        fs::create_dir_all(&rootfs_dir)?;

        // unsquashfs may return non-zero for xattr warnings, so allow_fail and check manually
        let result = Cmd::new("unsquashfs")
            .args(["-d"])
            .arg_path(&rootfs_dir)
            .args(["-f", "-no-xattrs"])
            .arg_path(squashfs_path)
            .allow_fail()
            .run()?;

        // Check if extraction actually succeeded (regardless of exit code)
        if !result.success() && !rootfs_dir.join("usr").exists() {
            bail!(
                "unsquashfs failed. Install: sudo dnf install squashfs-tools\n{}",
                result.stderr_trimmed()
            );
        }

        // Fix permissions: unsquashfs preserves root ownership which prevents further writes
        // Make the rootfs writable so we can merge in supplementary RPMs
        println!("Fixing permissions on extracted rootfs...");
        let chmod_result = Cmd::new("chmod")
            .args(["-R", "u+rwX"])
            .arg_path(&rootfs_dir)
            .allow_fail()
            .run()?;

        if !chmod_result.success() {
            // FAIL FAST - if we can't fix permissions, the build will fail later
            // Better to fail now with a clear message
            bail!(
                "Could not fix permissions on extracted rootfs.\n\
                 \n\
                 The rootfs contains files owned by root that prevent writing.\n\
                 Without write access, we cannot merge supplementary RPMs.\n\
                 \n\
                 Run with sudo, or run 'sudo chown -R $USER {}' first.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                rootfs_dir.display()
            );
        }
    } else {
        println!("Rootfs already extracted to {}", rootfs_dir.display());
    }

    // Step 3: Extract supplementary RPMs into rootfs
    // The install.img is the Anaconda installer which lacks some utilities users expect
    extract_supplementary_rpms(&iso_contents, &rootfs_dir)?;

    println!("Extraction complete!");
    Ok(())
}

/// Get the list of RPMs to extract.
/// Combines SUPPLEMENTARY_RPMS with any additional RPMs from EXTRA_RPMS env var.
fn get_rpm_list() -> Vec<String> {
    let mut rpms: Vec<String> = SUPPLEMENTARY_RPMS.iter().map(|s| s.to_string()).collect();

    // Append extra RPMs from environment variable (comma-separated)
    if let Ok(extra) = env::var("EXTRA_RPMS") {
        for rpm in extra.split(',') {
            let rpm = rpm.trim();
            if !rpm.is_empty() && !rpms.iter().any(|r| r == rpm) {
                rpms.push(rpm.to_string());
            }
        }
    }

    rpms
}

/// Extract supplementary RPMs and merge them into the rootfs.
fn extract_supplementary_rpms(iso_contents: &Path, rootfs_dir: &Path) -> Result<()> {
    // Search both BaseOS and AppStream for packages
    let package_dirs = [
        iso_contents.join("BaseOS/Packages"),
        iso_contents.join("AppStream/Packages"),
    ];

    let rpm_list = get_rpm_list();
    for rpm_prefix in &rpm_list {
        let first_char = rpm_prefix.chars().next().unwrap();

        // Search in both BaseOS and AppStream
        let mut rpm_path: Option<std::path::PathBuf> = None;
        for packages_dir in &package_dirs {
            if !packages_dir.exists() {
                continue;
            }
            let rpm_subdir = packages_dir.join(first_char.to_string());
            if rpm_subdir.exists() {
                if let Some(found) = find_rpm(&rpm_subdir, rpm_prefix)? {
                    rpm_path = Some(found);
                    break;
                }
            }
        }

        // Find the matching RPM
        let rpm_path = rpm_path;

        if let Some(rpm) = rpm_path {
            println!("Extracting supplementary RPM: {}", rpm.file_name().unwrap().to_string_lossy());

            // Extract RPM contents directly into rootfs
            // rpm2cpio outputs cpio archive, which we extract in rootfs
            let cmd = format!("rpm2cpio '{}' | cpio -idmu --quiet", rpm.display());
            if let Err(e) = shell_in(&cmd, rootfs_dir) {
                // FAIL FAST - RPM extraction failure means missing utilities
                // These RPMs are in the SUPPLEMENTARY_RPMS list because they're REQUIRED
                bail!(
                    "Failed to extract RPM {}:\n{}\n\
                     \n\
                     This RPM is in SUPPLEMENTARY_RPMS because it's REQUIRED.\n\
                     The build cannot continue without it.\n\
                     \n\
                     DO NOT change this to a warning. FAIL FAST.",
                    rpm.display(),
                    e
                );
            }
        } else {
            // FAIL FAST - if an RPM is in SUPPLEMENTARY_RPMS, it's REQUIRED
            // Don't silently continue with missing packages
            bail!(
                "Required RPM not found for prefix: {}\n\
                 \n\
                 This RPM is in SUPPLEMENTARY_RPMS because it provides essential utilities.\n\
                 Without it, users will get 'command not found' errors.\n\
                 \n\
                 Check that the ISO has both BaseOS/Packages and AppStream/Packages.\n\
                 \n\
                 DO NOT change this to a warning. FAIL FAST.",
                rpm_prefix
            );
        }
    }

    Ok(())
}

/// Find an RPM file by prefix in a directory.
fn find_rpm(dir: &Path, prefix: &str) -> Result<Option<std::path::PathBuf>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            // Match RPM files that start with the prefix and end with .rpm
            // e.g., "procps-ng" matches "procps-ng-4.0.4-8.el10.x86_64.rpm"
            // Also accept noarch packages (e.g., microcode_ctl is noarch)
            if name_str.starts_with(prefix)
                && name_str.ends_with(".rpm")
                && !name_str.contains("-devel")
                && !name_str.contains("-i18n")
                && (name_str.contains("x86_64") || name_str.contains("noarch"))
            {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}
