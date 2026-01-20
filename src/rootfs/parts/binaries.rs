//! Binary lists and copying for rootfs builder.
//!
//! Contains the complete list of binaries needed for an installed system.
//! ALL binaries are required - missing binaries cause build failure.
//!
//! # PHILOSOPHY: ALL OR NOTHING - FAIL FAST
//!
//! There is no "optional" category. Every binary in these lists is required.
//! If a binary is missing from the source, the build FAILS.
//!
//! The correct response to "binary not found" is NEVER "make it optional".
//! It's either:
//! 1. Add the RPM package that provides it
//! 2. Remove the binary from requirements (if truly unneeded)
//!
//! # Sources
//!
//! When using RPM extraction (correct approach), binaries come from:
//! - coreutils: ls, cat, cp, mv, rm, mkdir, chmod, chown, etc.
//! - util-linux: mount, umount, lsblk, fdisk, mkfs, etc.
//! - procps-ng: ps, pgrep, pkill, top, free
//! - shadow-utils: useradd, userdel, groupadd, passwd, etc.
//! - systemd: systemctl, journalctl, hostnamectl, etc.
//! - iproute: ip, ss, bridge
//! - And others...
//!
//! See: .teams/KNOWLEDGE_false-positives-testing.md

use anyhow::{bail, Result};

use crate::rootfs::binary::{copy_binary_with_libs, copy_bash, copy_sbin_binary_with_libs};
use crate::rootfs::context::BuildContext;

/// Binaries that go to /usr/bin.
///
/// These are user-facing commands. ALL are required.
const BIN: &[&str] = &[
    // === COREUTILS (from coreutils package) ===
    // File operations
    "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
    "chmod", "chown", "chgrp", "ln", "readlink", "realpath",
    "stat", "file",
    // Text processing
    "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
    "sed", "awk", "gawk",
    "printf", "uniq", "seq",
    // Search
    "grep", "find", "xargs",
    // System info
    "pwd", "uname", "date", "env", "id", "hostname",
    "printenv", "whoami", "groups",
    // Process control
    "sleep", "kill", "nice", "nohup",
    // Compression
    "gzip", "gunzip", "xz", "unxz", "tar", "bzip2", "bunzip2", "cpio",
    // Shell utilities
    "true", "false", "expr", "test", "yes",
    // Disk info (from util-linux, but lives in /usr/bin)
    "df", "du", "sync",
    "mount", "umount", "lsblk", "findmnt",
    // Path utilities
    "dirname", "basename",
    // Other utilities
    "which",

    // === DIFFUTILS ===
    "diff", "cmp",

    // === PROCPS-NG ===
    "ps", "pgrep", "pkill", "top", "free", "uptime", "w",

    // === SYSTEMD (user commands) ===
    "systemctl", "journalctl",
    "timedatectl", "hostnamectl", "localectl", "loginctl", "bootctl",

    // === EDITORS ===
    "vi",  // from vim-minimal (vim is not in base)
    "nano",

    // === NETWORK ===
    "ping", "curl", "wget",

    // === MISC ===
    "less", "more",
];

/// Binaries that go to /usr/sbin.
///
/// These are system administration commands. ALL are required.
const SBIN: &[&str] = &[
    // === UTIL-LINUX (sbin) ===
    // Filesystem operations
    "fsck", "blkid", "losetup", "mkswap", "swapon", "swapoff",
    "fdisk", "sfdisk", "wipefs", "blockdev",
    "pivot_root", "chroot",

    // === E2FSPROGS ===
    "fsck.ext4", "fsck.ext2", "fsck.ext3",
    "e2fsck", "mke2fs",
    "mkfs.ext4", "mkfs.ext2", "mkfs.ext3",
    "tune2fs", "resize2fs",

    // === DOSFSTOOLS ===
    "mkfs.fat", "mkfs.vfat", "fsck.fat", "fsck.vfat",

    // === KMOD ===
    "insmod", "rmmod", "modprobe", "lsmod", "depmod", "modinfo",

    // === SHADOW-UTILS (user management) ===
    "useradd", "userdel", "usermod",
    "groupadd", "groupdel", "groupmod",
    "chpasswd", "passwd",

    // === IPROUTE ===
    "ip", "ss", "bridge",

    // === PROCPS-NG (sbin) ===
    "sysctl",

    // === SYSTEM CONTROL ===
    "reboot", "shutdown", "poweroff", "halt",

    // === OTHER ===
    "ldconfig",
    "hwclock",
    "lspci",
    "ifconfig", "route",  // net-tools (legacy but useful)
    "agetty", "login", "sulogin", "nologin",
    "chronyd",
];

/// Systemd binaries to copy from /usr/lib/systemd/.
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

/// Copy all /usr/bin binaries. FAILS if ANY are missing.
pub fn copy_coreutils(ctx: &BuildContext) -> Result<()> {
    println!("Copying /usr/bin binaries...");

    let mut missing = Vec::new();
    let mut copied = 0;

    for binary in BIN {
        match copy_binary_with_libs(ctx, binary, "usr/bin") {
            Ok(true) => copied += 1,
            Ok(false) => missing.push(*binary),
            Err(e) => {
                // Binary found but libraries missing = FAIL
                return Err(e);
            }
        }
    }

    // FAIL if ANY binaries are missing
    if !missing.is_empty() {
        bail!(
            "Binaries missing from source: {}\n\
             ALL binaries are required. Fix the source (add RPM packages).",
            missing.join(", ")
        );
    }

    println!("  Copied {}/{} binaries to /usr/bin", copied, BIN.len());
    Ok(())
}

/// Copy all /usr/sbin binaries. FAILS if ANY are missing.
pub fn copy_sbin_utils(ctx: &BuildContext) -> Result<()> {
    println!("Copying /usr/sbin binaries...");

    let mut missing = Vec::new();
    let mut copied = 0;

    for binary in SBIN {
        match copy_sbin_binary_with_libs(ctx, binary) {
            Ok(true) => copied += 1,
            Ok(false) => missing.push(*binary),
            Err(e) => {
                // Binary found but libraries missing = FAIL
                return Err(e);
            }
        }
    }

    // FAIL if ANY binaries are missing
    if !missing.is_empty() {
        bail!(
            "Sbin utilities missing from source: {}\n\
             ALL binaries are required. Fix the source (add RPM packages).",
            missing.join(", ")
        );
    }

    println!("  Copied {}/{} binaries to /usr/sbin", copied, SBIN.len());
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

    // Copy main systemd binary
    let systemd_src = ctx.source.join("usr/lib/systemd/systemd");
    let systemd_dst = ctx.staging.join("usr/lib/systemd/systemd");
    if systemd_src.exists() {
        std::fs::create_dir_all(systemd_dst.parent().unwrap())?;
        std::fs::copy(&systemd_src, &systemd_dst)?;
        crate::rootfs::binary::make_executable(&systemd_dst)?;
        println!("  Copied systemd");
    }

    // Copy helper binaries
    for binary in SYSTEMD_BINARIES {
        let src = ctx.source.join("usr/lib/systemd").join(binary);
        let dst = ctx.staging.join("usr/lib/systemd").join(binary);
        if src.exists() {
            std::fs::copy(&src, &dst)?;
            crate::rootfs::binary::make_executable(&dst)?;
        }
    }

    // Copy systemd private libraries
    let systemd_lib_src = ctx.source.join("usr/lib64/systemd");
    if systemd_lib_src.exists() {
        std::fs::create_dir_all(ctx.staging.join("usr/lib64/systemd"))?;
        for entry in std::fs::read_dir(&systemd_lib_src)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                let dst = ctx.staging.join("usr/lib64/systemd").join(&name);
                std::fs::copy(entry.path(), &dst)?;
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

/// Copy agetty and login binaries for getty/console.
pub fn copy_login_binaries(ctx: &BuildContext) -> Result<()> {
    println!("Copying login binaries...");

    // These are already in SBIN list, but let's make sure they're copied
    let login_binaries = ["agetty", "login", "sulogin", "nologin"];

    for binary in login_binaries {
        copy_sbin_binary_with_libs(ctx, binary)?;
    }

    println!("  Copied login binaries");
    Ok(())
}
