//! Tarball verification for rootfs builder.
//!
//! This module verifies that the rootfs tarball contains all critical components.
//!
//! # ⚠️ WARNING: DO NOT WEAKEN THIS VERIFICATION ⚠️
//!
//! This verification exists to catch broken builds BEFORE they ship.
//!
//! If this verification fails, the CORRECT response is:
//! 1. Fix the build to include the missing files
//! 2. NOT: Remove the check for the missing file
//! 3. NOT: Move the file to an "optional" category
//! 4. NOT: Add an exception "just for now"
//!
//! A verification that passes on broken builds is WORSE than no verification.
//! It gives false confidence and lets broken products ship.
//!
//! Remember: "✓ 83/83 passed" means NOTHING if those 83 don't include
//! what users actually need.
//!
//! Read: .teams/KNOWLEDGE_false-positives-testing.md

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::rootfs::parts::auth;

/// Verify tarball contents - checks ALL critical components.
///
/// # ⚠️ WARNING: DO NOT WEAKEN THIS VERIFICATION ⚠️
///
/// This function exists to catch broken builds BEFORE they ship.
///
/// If this verification fails, the CORRECT response is:
/// 1. Fix the build to include the missing files
/// 2. NOT: Remove the check for the missing file
/// 3. NOT: Move the file to an "optional" category
/// 4. NOT: Add an exception "just for now"
///
/// A verification that passes on broken builds is WORSE than no verification.
/// It gives false confidence and lets broken products ship.
///
/// Remember: "✓ 83/83 passed" means NOTHING if those 83 don't include
/// what users actually need.
///
/// Read: .teams/KNOWLEDGE_false-positives-testing.md
pub fn verify_tarball(path: &Path) -> Result<()> {
    println!("Verifying {}...\n", path.display());

    let output = Command::new("tar")
        .args(["-tJf", path.to_str().unwrap()])
        .output()
        .context("Failed to run tar command")?;

    if !output.status.success() {
        anyhow::bail!("tar command failed");
    }

    let contents = String::from_utf8_lossy(&output.stdout);
    let mut missing = Vec::new();
    let mut checked = 0;

    // Critical binaries - SAME list as in binaries.rs
    // ⚠️ DO NOT REMOVE ITEMS FROM THIS LIST JUST BECAUSE THEY'RE MISSING ⚠️
    // If something is missing, FIX THE BUILD, don't weaken the test
    // Note: In Rocky 10, mount/umount/lsblk are in /usr/bin, not /usr/sbin
    let critical_coreutils = [
        "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
        "chmod", "chown", "ln", "readlink",
        "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
        "grep", "find", "xargs",
        "pwd", "uname", "date", "env", "id", "hostname",
        "sleep", "kill", "ps",
        "gzip", "gunzip", "xz", "unxz", "tar",
        "true", "false", "expr",
        "sed",
        "df", "du", "sync",
        "mount", "umount", "lsblk", "findmnt",  // disk utils in /usr/bin
        "systemctl", "journalctl",
    ];

    let critical_sbin = [
        "fsck", "blkid", "losetup",
        "reboot", "shutdown", "poweroff",
        "insmod", "rmmod", "modprobe", "lsmod",
        "chroot", "ldconfig",
        "useradd", "groupadd", "chpasswd",
        "ip", "sysctl",
    ];

    // Check critical coreutils
    println!("Checking critical coreutils...");
    for bin in critical_coreutils {
        let path = format!("./usr/bin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check critical sbin
    println!("Checking critical sbin utilities...");
    for bin in critical_sbin {
        let path = format!("./usr/sbin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check auth critical binaries (from auth.rs single source of truth)
    // These MUST exist - users need su/sudo for privilege escalation
    println!("Checking auth binaries (su/sudo)...");
    for bin in auth::AUTH_CRITICAL_BIN {
        let path = format!("./usr/bin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }
    for bin in auth::AUTH_CRITICAL_SBIN {
        let path = format!("./usr/sbin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check shell
    println!("Checking shell...");
    for path in ["./usr/bin/bash", "./usr/bin/sh"] {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check systemd
    println!("Checking systemd...");
    let systemd_critical = [
        "./usr/lib/systemd/systemd",
        "./usr/sbin/init",
        "./etc/systemd/system/default.target",
    ];
    for path in systemd_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check /etc essentials
    println!("Checking /etc configuration...");
    let etc_critical = [
        "./etc/passwd",
        "./etc/shadow",
        "./etc/group",
        "./etc/os-release",
        "./etc/fstab",
        "./etc/hosts",
    ];
    for path in etc_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check PAM
    println!("Checking PAM...");
    let pam_critical = [
        "./etc/pam.d/system-auth",
        "./etc/pam.d/login",
        "./usr/lib64/security/pam_unix.so",
    ];
    for path in pam_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check login binaries
    println!("Checking login binaries...");
    for bin in ["agetty", "login", "nologin"] {
        let path = format!("./usr/sbin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check dracut (required for initramfs generation)
    println!("Checking dracut...");
    for path in ["./usr/bin/dracut", "./usr/lib/dracut/dracut.sh"] {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check firmware directory exists
    println!("Checking firmware...");
    checked += 1;
    if !contents.contains("./usr/lib/firmware/") {
        missing.push("./usr/lib/firmware/".to_string());
    }

    // Check kernel (optional - can be built separately, but warn)
    println!("Checking kernel (optional)...");
    if !contents.contains("./boot/vmlinuz") {
        println!("  Note: No kernel in tarball. Build with 'leviso kernel' or install via recipe.");
    }

    println!();
    if missing.is_empty() {
        println!("✓ Verified {}/{} critical files present", checked, checked);
        Ok(())
    } else {
        println!("✗ VERIFICATION FAILED");
        println!("  Missing {}/{} critical files:", missing.len(), checked);
        for file in &missing {
            println!("    - {}", file);
        }
        anyhow::bail!("Tarball is INCOMPLETE - {} critical files missing", missing.len());
    }
}
