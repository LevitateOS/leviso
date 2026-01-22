//! Binary lists and copying.
//!
//! ALL binaries are required - missing binaries cause build failure.
//! There is no "optional" category.

use anyhow::{bail, Result};
use std::fs;

use super::libdeps::{copy_bash, copy_binary_with_libs, copy_sbin_binary_with_libs, make_executable};
use super::context::BuildContext;

/// Authentication binaries for /usr/bin.
const AUTH_BIN: &[&str] = &["su", "sudo", "sudoedit", "sudoreplay"];

/// Authentication binaries for /usr/sbin.
const AUTH_SBIN: &[&str] = &["visudo"];

/// Sudo support libraries (dynamically loaded, not discoverable via ldd).
const SUDO_LIBEXEC: &[&str] = &[
    "libsudo_util.so.0.0.0",
    "libsudo_util.so.0",
    "libsudo_util.so",
    "sudoers.so",
    "group_file.so",
    "system_group.so",
];

/// Binaries for /usr/bin.
const BIN: &[&str] = &[
    // === COREUTILS ===
    "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
    "chmod", "chown", "chgrp", "ln", "readlink", "realpath",
    "stat", "file", "mknod", "mkfifo",
    "timeout", "sleep", "true", "false", "test", "[",  // Used by dracut/scripts
    // Text processing
    "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
    "sed", "awk", "gawk", "printf", "uniq", "seq",
    // Search
    "grep", "find", "xargs",
    // System info
    "pwd", "uname", "date", "env", "id", "hostname",
    "printenv", "whoami", "groups", "dmesg",
    // Process control
    "sleep", "kill", "nice", "nohup", "setsid",
    // Compression
    "gzip", "gunzip", "xz", "unxz", "tar", "bzip2", "bunzip2", "cpio",
    // Shell utilities
    "true", "false", "expr", "test", "yes", "mktemp",
    // Disk info
    "df", "du", "sync", "mount", "umount", "lsblk", "findmnt", "flock",
    // Path utilities
    "dirname", "basename",
    // Other
    "which",
    // === DIFFUTILS ===
    "diff", "cmp",
    // === PROCPS-NG ===
    "ps", "pgrep", "pkill", "top", "free", "uptime", "w", "vmstat", "watch",
    // === SYSTEMD ===
    "systemctl", "journalctl", "timedatectl", "hostnamectl", "localectl", "loginctl", "bootctl",
    // === EDITORS ===
    "vi", "nano",
    // === NETWORK ===
    "ping", "curl", "wget",
    // === TERMINAL ===
    "clear", "stty", "tty",
    // === KEYBOARD ===
    "loadkeys",
    // === LOCALE ===
    "localedef",
    // === UDEV ===
    "udevadm",
    // === MISC ===
    "less", "more",
    // === UTIL-LINUX (command line parsing) ===
    "getopt",
    // === DRACUT (initramfs generator) ===
    "dracut",
    // === GLIBC UTILITIES ===
    "getent", "ldd",
];

/// Binaries for /usr/sbin.
const SBIN: &[&str] = &[
    // === UTIL-LINUX ===
    "fsck", "blkid", "losetup", "mkswap", "swapon", "swapoff",
    "fdisk", "sfdisk", "wipefs", "blockdev", "pivot_root", "chroot",
    "switch_root",  // Required by dracut for initramfs
    "parted",
    // === E2FSPROGS ===
    "fsck.ext4", "fsck.ext2", "fsck.ext3", "e2fsck", "mke2fs",
    "mkfs.ext4", "mkfs.ext2", "mkfs.ext3", "tune2fs", "resize2fs",
    // === DOSFSTOOLS ===
    "mkfs.fat", "mkfs.vfat", "fsck.fat", "fsck.vfat",
    // === KMOD ===
    "insmod", "rmmod", "modprobe", "lsmod", "depmod", "modinfo",
    // === SHADOW-UTILS ===
    "useradd", "userdel", "usermod", "groupadd", "groupdel", "groupmod",
    "chpasswd", "passwd",
    // === IPROUTE ===
    "ip", "ss", "bridge",
    // === PROCPS-NG ===
    "sysctl",
    // === SYSTEM CONTROL ===
    "reboot", "shutdown", "poweroff", "halt",
    // === OTHER ===
    "ldconfig", "hwclock", "lspci", "ifconfig", "route",
    "agetty", "login", "sulogin", "nologin", "chronyd",
    // === SQUASHFS-TOOLS (for installation) ===
    // unsquashfs is REQUIRED - recstrap uses it to extract squashfs to disk
    // mksquashfs is NOT included - it's a host tool for building ISOs
    "unsquashfs",
];

/// Systemd helper binaries.
const SYSTEMD_BINARIES: &[&str] = &[
    "systemd-executor",
    "systemd-shutdown",
    "systemd-sulogin-shell",
    "systemd-cgroups-agent",
    "systemd-journald",
    "systemd-modules-load",
    "systemd-sysctl",
    "systemd-tmpfiles",
    "systemd-timedated",
    "systemd-hostnamed",
    "systemd-localed",
    "systemd-logind",
    "systemd-networkd",
    "systemd-resolved",
    "systemd-udevd",
    "systemd-fsck",
    "systemd-remount-fs",
    "systemd-vconsole-setup",
    "systemd-random-seed",
];

/// Copy all /usr/bin binaries.
pub fn copy_coreutils(ctx: &BuildContext) -> Result<()> {
    println!("Copying /usr/bin binaries...");

    let mut missing = Vec::new();
    let mut copied = 0;

    let all_bin: Vec<&str> = BIN.iter().chain(AUTH_BIN.iter()).copied().collect();
    let total = all_bin.len();

    for binary in &all_bin {
        match copy_binary_with_libs(ctx, binary, "usr/bin") {
            Ok(true) => copied += 1,
            Ok(false) => missing.push(*binary),
            Err(e) => return Err(e),
        }
    }

    if !missing.is_empty() {
        bail!(
            "Binaries missing: {}\nALL binaries are required.",
            missing.join(", ")
        );
    }

    println!("  Copied {}/{} binaries to /usr/bin", copied, total);
    Ok(())
}

/// Copy all /usr/sbin binaries.
pub fn copy_sbin_utils(ctx: &BuildContext) -> Result<()> {
    println!("Copying /usr/sbin binaries...");

    let mut missing = Vec::new();
    let mut copied = 0;

    let all_sbin: Vec<&str> = SBIN.iter().chain(AUTH_SBIN.iter()).copied().collect();
    let total = all_sbin.len();

    for binary in &all_sbin {
        match copy_sbin_binary_with_libs(ctx, binary) {
            Ok(true) => copied += 1,
            Ok(false) => missing.push(*binary),
            Err(e) => return Err(e),
        }
    }

    if !missing.is_empty() {
        bail!(
            "Sbin binaries missing: {}\nALL binaries are required.",
            missing.join(", ")
        );
    }

    println!("  Copied {}/{} binaries to /usr/sbin", copied, total);
    Ok(())
}

/// Copy bash shell.
pub fn copy_shell(ctx: &BuildContext) -> Result<()> {
    println!("Copying bash shell...");
    copy_bash(ctx)?;
    println!("  Copied bash");
    Ok(())
}

/// Copy systemd binaries and libraries.
pub fn copy_systemd_binaries(ctx: &BuildContext) -> Result<()> {
    println!("Copying systemd binaries...");

    let systemd_src = ctx.source.join("usr/lib/systemd/systemd");
    let systemd_dst = ctx.staging.join("usr/lib/systemd/systemd");
    if systemd_src.exists() {
        fs::create_dir_all(systemd_dst.parent().unwrap())?;
        fs::copy(&systemd_src, &systemd_dst)?;
        make_executable(&systemd_dst)?;
        println!("  Copied systemd");
    }

    for binary in SYSTEMD_BINARIES {
        let src = ctx.source.join("usr/lib/systemd").join(binary);
        let dst = ctx.staging.join("usr/lib/systemd").join(binary);
        if src.exists() {
            fs::copy(&src, &dst)?;
            make_executable(&dst)?;
        }
    }

    // Copy systemd private libraries
    let systemd_lib_src = ctx.source.join("usr/lib64/systemd");
    if systemd_lib_src.exists() {
        fs::create_dir_all(ctx.staging.join("usr/lib64/systemd"))?;
        for entry in fs::read_dir(&systemd_lib_src)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                let dst = ctx.staging.join("usr/lib64/systemd").join(&name);
                fs::copy(entry.path(), &dst)?;
            }
        }
    }

    // Create /sbin/init -> /usr/lib/systemd/systemd symlink
    let init_link = ctx.staging.join("usr/sbin/init");
    if !init_link.exists() && !init_link.is_symlink() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }

    println!("  Copied {} systemd binaries", SYSTEMD_BINARIES.len());
    Ok(())
}

/// Copy login binaries.
pub fn copy_login_binaries(ctx: &BuildContext) -> Result<()> {
    println!("Copying login binaries...");
    for binary in ["agetty", "login", "sulogin", "nologin"] {
        copy_sbin_binary_with_libs(ctx, binary)?;
    }
    println!("  Copied login binaries");
    Ok(())
}

/// Copy sudo support libraries.
pub fn copy_sudo_libs(ctx: &BuildContext) -> Result<()> {
    println!("Copying sudo support libraries...");

    let src_dir = ctx.source.join("usr/libexec/sudo");
    let dst_dir = ctx.staging.join("usr/libexec/sudo");

    if !src_dir.exists() {
        bail!("sudo libexec not found at {}", src_dir.display());
    }

    fs::create_dir_all(&dst_dir)?;

    let mut copied = 0;
    for lib in SUDO_LIBEXEC {
        let src = src_dir.join(lib);
        let dst = dst_dir.join(lib);

        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            if dst.exists() || dst.is_symlink() {
                fs::remove_file(&dst)?;
            }
            std::os::unix::fs::symlink(&target, &dst)?;
            copied += 1;
        } else if src.exists() {
            fs::copy(&src, &dst)?;
            copied += 1;
        }
    }

    println!("  Copied {}/{} sudo libraries", copied, SUDO_LIBEXEC.len());
    Ok(())
}
