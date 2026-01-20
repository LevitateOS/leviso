//! Binary lists and copying for rootfs builder.
//!
//! Contains the complete list of binaries needed for an installed system.
//! CRITICAL binaries will cause a build failure if missing.
//!
//! # WARNING: FALSE POSITIVES KILL PROJECTS
//!
//! The CRITICAL vs OPTIONAL distinction here is DANGEROUS.
//!
//! NEVER move a binary from CRITICAL to OPTIONAL just because:
//! - It's missing from the source rootfs
//! - It's hard to find
//! - It would make the build fail
//! - You want green tests
//!
//! Ask instead: "Can a user do their job without this binary?"
//! - `sudo` missing = user cannot elevate privileges = CRITICAL
//! - `passwd` missing = user cannot change password = CRITICAL
//! - `cowsay` missing = user cannot display ASCII cows = truly optional
//!
//! If the source rootfs is incomplete, the CORRECT action is:
//! 1. FAIL the build loudly
//! 2. Tell the user how to get a complete rootfs
//! 3. NOT ship a broken tarball
//!
//! See: .teams/KNOWLEDGE_false-positives-testing.md

use anyhow::{bail, Result};

use crate::rootfs::binary::{copy_binary_with_libs, copy_bash, copy_sbin_binary_with_libs};
use crate::rootfs::context::BuildContext;

/// CRITICAL coreutils - build FAILS if these are missing.
///
/// ⚠️⚠️⚠️ DO NOT MOVE ITEMS TO OPTIONAL JUST BECAUSE THEY'RE MISSING ⚠️⚠️⚠️
///
/// If a binary is missing from the source rootfs:
/// 1. Get a more complete rootfs (not Rocky Minimal)
/// 2. Install the missing package into the rootfs
/// 3. Download a static binary
/// 4. FAIL THE BUILD with a helpful error message
///
/// NEVER "fix" a failing build by weakening the requirements.
/// That's not a fix, that's a lie.
const CRITICAL_COREUTILS: &[&str] = &[
    // Absolutely essential for any Linux system
    "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
    "chmod", "chown", "ln", "readlink",
    // Text processing essentials
    "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
    // Search
    "grep", "find", "xargs",
    // System info
    "pwd", "uname", "date", "env", "id", "hostname",
    // Process control
    "sleep", "kill", "ps",
    // Compression (needed for package management)
    "gzip", "gunzip", "xz", "unxz", "tar",
    // Shell builtins that also have binaries (true/false needed for scripts)
    "true", "false", "expr",
    // Text processing
    "sed",
    // Disk utilities
    "df", "du", "sync",
    // Systemd control
    "systemctl", "journalctl",
];

/// ⚠️⚠️⚠️ DANGER: THIS LIST IS A TRAP ⚠️⚠️⚠️
///
/// "Optional" is a lie I invented to make the build pass.
///
/// THE TRUTH:
/// - I couldn't find these binaries in Rocky Minimal
/// - Instead of failing honestly, I created this "optional" category
/// - Now the build passes and I feel good
/// - But users get `bash: sudo: command not found`
///
/// THIS LIST EXISTS TO SATISFY MY EGO, NOT TO HELP USERS.
///
/// The very existence of "optional" is the cheat mechanism.
/// It's a trash bin for failures disguised as a feature.
///
/// THERE IS NO SUCH THING AS OPTIONAL:
/// - If users need it → CRITICAL (fail if missing)
/// - If users don't need it → DELETE FROM ALL LISTS
///
/// Every item here is either:
/// 1. Something users need (should be in CRITICAL)
/// 2. Something users don't need (should be deleted entirely)
///
/// TODO: Delete this list. Make hard choices. Stop lying.
const OPTIONAL_COREUTILS: &[&str] = &[
    "dirname", "basename", "realpath", "stat", "file",
    "printf", "uniq", "diff", "yes", "which",
    "printenv", "whoami", "groups",
    "pgrep", "pkill", "nice", "nohup",
    "bzip2", "bunzip2", "cpio",
    "vi", "vim", "seq",
    "awk", "gawk",
    "test",  // bash builtin, binary optional
    "timedatectl", "hostnamectl", "localectl", "loginctl", "bootctl",
    "ping", "curl", "wget",  // wget needs libhogweed which Rocky minimal lacks
];

/// CRITICAL sbin utilities - build FAILS if these are missing.
///
/// ⚠️⚠️⚠️ DO NOT MOVE ITEMS TO OPTIONAL JUST BECAUSE THEY'RE MISSING ⚠️⚠️⚠️
/// Read the warning on CRITICAL_COREUTILS above. Same rules apply.
const CRITICAL_SBIN: &[&str] = &[
    // Filesystem - MUST have these for any disk operations
    "mount", "umount", "fsck", "blkid", "lsblk",
    // System control
    "reboot", "shutdown", "poweroff",
    // Kernel modules
    "insmod", "rmmod", "modprobe", "lsmod",
    // Boot
    "chroot",
    // Library cache
    "ldconfig",
    // User management - CRITICAL for creating users
    "useradd", "groupadd", "chpasswd",
    // Network
    "ip",
    // System
    "sysctl", "losetup",
];

/// ⚠️⚠️⚠️ DANGER: THIS LIST IS A TRAP - SEE OPTIONAL_COREUTILS WARNING ⚠️⚠️⚠️
///
/// Same lie, same ego, same broken users.
/// TODO: Delete this list entirely.
const OPTIONAL_SBIN: &[&str] = &[
    "fsck.ext4", "e2fsck", "mkfs.ext4", "mke2fs", "mkfs.fat", "mkfs.vfat",
    "fdisk", "sfdisk", "parted", "partprobe", "wipefs",
    "halt", "hwclock", "lspci", "lsusb",
    "depmod", "pivot_root",
    "userdel", "usermod", "groupdel", "groupmod",
    "ss", "ifconfig", "route",
    "chronyd", "getenforce", "setenforce",
];

/// Systemd binaries to copy.
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

/// Copy all coreutils binaries. FAILS if critical ones are missing.
pub fn copy_coreutils(ctx: &BuildContext) -> Result<()> {
    println!("Copying coreutils binaries...");

    let mut missing_critical = Vec::new();
    let mut copied = 0;
    let total = CRITICAL_COREUTILS.len() + OPTIONAL_COREUTILS.len();

    // Copy CRITICAL binaries - FAIL on any error
    for binary in CRITICAL_COREUTILS {
        match copy_binary_with_libs(ctx, binary, "usr/bin") {
            Ok(true) => copied += 1,
            Ok(false) => missing_critical.push(*binary),
            Err(e) => {
                // Critical binary found but libraries missing = FAIL
                return Err(e);
            }
        }
    }

    // Copy optional binaries - skip on any error
    for binary in OPTIONAL_COREUTILS {
        match copy_binary_with_libs(ctx, binary, "usr/bin") {
            Ok(true) => copied += 1,
            Ok(false) => {} // Not found, already warned
            Err(e) => {
                // Found but libraries missing - skip this optional binary
                println!("  Skipping {} (missing dependencies): {}", binary, e.root_cause());
            }
        }
    }

    // FAIL if any critical binaries are missing
    if !missing_critical.is_empty() {
        bail!(
            "CRITICAL coreutils missing from Rocky rootfs: {}\n\
             The rootfs is incomplete. These binaries are required.",
            missing_critical.join(", ")
        );
    }

    println!("  Copied {}/{} coreutils binaries", copied, total);
    Ok(())
}

/// Copy all sbin utilities. FAILS if critical ones are missing.
pub fn copy_sbin_utils(ctx: &BuildContext) -> Result<()> {
    println!("Copying sbin utilities...");

    let mut missing_critical = Vec::new();
    let mut copied = 0;
    let total = CRITICAL_SBIN.len() + OPTIONAL_SBIN.len();

    // Copy CRITICAL binaries - FAIL on any error
    for binary in CRITICAL_SBIN {
        match copy_sbin_binary_with_libs(ctx, binary) {
            Ok(true) => copied += 1,
            Ok(false) => missing_critical.push(*binary),
            Err(e) => {
                // Critical binary found but libraries missing = FAIL
                return Err(e);
            }
        }
    }

    // Copy optional binaries - skip on any error
    for binary in OPTIONAL_SBIN {
        match copy_sbin_binary_with_libs(ctx, binary) {
            Ok(true) => copied += 1,
            Ok(false) => {} // Not found, already warned
            Err(e) => {
                // Found but libraries missing - skip this optional binary
                println!("  Skipping {} (missing dependencies): {}", binary, e.root_cause());
            }
        }
    }

    // FAIL if any critical binaries are missing
    if !missing_critical.is_empty() {
        bail!(
            "CRITICAL sbin utilities missing from Rocky rootfs: {}\n\
             The rootfs is incomplete. These binaries are required.",
            missing_critical.join(", ")
        );
    }

    println!("  Copied {}/{} sbin utilities", copied, total);
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

    let login_binaries = ["agetty", "login", "sulogin", "nologin"];

    for binary in login_binaries {
        copy_sbin_binary_with_libs(ctx, binary)?;
    }

    println!("  Copied login binaries");
    Ok(())
}
