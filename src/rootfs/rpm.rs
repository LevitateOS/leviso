//! RPM package extraction for rootfs builder.
//!
//! Extracts binaries and libraries from Rocky Linux RPMs instead of relying
//! on an incomplete minimal rootfs. This is the CORRECT approach - get binaries
//! from the actual packages that ship them.
//!
//! ## Philosophy: Exactly Enough (Like Arch)
//!
//! Not minimal (missing critical tools), not bloated (8GB DVD).
//! Include what a functional base system needs:
//! - Shell and coreutils
//! - System management (systemd)
//! - User management (shadow-utils)
//! - Disk utilities (util-linux)
//! - Network basics (iproute)
//! - Text processing (grep, sed, gawk, findutils)
//!
//! ## Usage
//!
//! ```rust,ignore
//! let extractor = RpmExtractor::new(packages_dir, staging_dir);
//! extractor.extract_packages(&REQUIRED_PACKAGES)?;
//! ```

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Required RPM packages for a functional base system.
///
/// This is the Arch-like "exactly enough" list. Each package here is
/// required - if extraction fails, the build fails. No "optional" escape hatch.
///
/// Provides binaries for: binaries.rs BIN and SBIN lists
pub const REQUIRED_PACKAGES: &[&str] = &[
    // === SHELL ===
    "bash",

    // === GNU COREUTILS ===
    // Provides: ls, cat, cp, mv, rm, mkdir, chmod, chown, ln, echo, head, tail,
    //           sort, cut, tr, tee, printf, uniq, seq, pwd, uname, date, env, id,
    //           sleep, kill, nice, nohup, true, false, expr, test, yes, df, du,
    //           sync, dirname, basename, stat, readlink, realpath, whoami, groups,
    //           printenv, chgrp
    "coreutils",
    "coreutils-common",

    // === SYSTEMD ===
    // Provides: systemd, systemctl, journalctl, timedatectl, hostnamectl,
    //           localectl, loginctl, bootctl, and all systemd-* helpers
    "systemd",
    "systemd-libs",
    "systemd-pam",
    "systemd-udev",

    // === UTIL-LINUX ===
    // Provides: mount, umount, lsblk, findmnt, fdisk, sfdisk, blkid, losetup,
    //           mkswap, swapon, swapoff, wipefs, blockdev, pivot_root, chroot,
    //           agetty, login, sulogin, nologin, hwclock
    "util-linux",
    "util-linux-core",

    // === SHADOW-UTILS ===
    // Provides: useradd, userdel, usermod, groupadd, groupdel, groupmod,
    //           chpasswd, passwd
    "shadow-utils",

    // === PROCPS-NG ===
    // Provides: ps, pgrep, pkill, top, free, uptime, w, sysctl
    "procps-ng",

    // === IPROUTE ===
    // Provides: ip, ss, bridge
    "iproute",
    "libbpf",          // ip needs this for BPF support
    "elfutils-libelf", // libbpf dependency
    "libmnl",          // netlink library

    // === IPUTILS ===
    // Provides: ping
    "iputils",

    // === TEXT PROCESSING ===
    "grep",       // grep
    "sed",        // sed
    "gawk",       // awk, gawk
    "findutils",  // find, xargs
    "diffutils",  // diff, cmp

    // === EDITORS ===
    "vim-minimal",  // vi
    "nano",         // nano

    // === NETWORK TOOLS ===
    "net-tools",  // ifconfig, route (legacy but useful)
    "curl",       // curl
    "wget",       // wget
    "hostname",   // hostname command

    // === FILESYSTEM ===
    "e2fsprogs",       // fsck.ext4, mke2fs, mkfs.ext4, tune2fs, resize2fs
    "e2fsprogs-libs",  // libext2fs, libss, libe2p
    "dosfstools",      // mkfs.fat, fsck.fat

    // === KERNEL MODULES ===
    "kmod",       // insmod, rmmod, modprobe, lsmod, depmod, modinfo
    "kmod-libs",  // libkmod

    // === HARDWARE ===
    "pciutils",       // lspci
    "pciutils-libs",  // libpci

    // === TIME ===
    "chrony",      // chronyd
    "libseccomp",  // chrony seccomp sandboxing

    // === COMPRESSION ===
    "gzip",
    "xz",
    "tar",
    "bzip2",
    "bzip2-libs",  // libbz2
    "cpio",        // cpio archiver

    // === UTILITIES ===
    "which",
    "file",
    "less",
    "psmisc",  // killall, pstree

    // === LIBRARIES ===
    // Core C library
    "glibc",
    "glibc-common",
    // Terminal
    "ncurses-libs",
    "readline",
    // Crypto
    "openssl-libs",
    "libgcrypt",
    "gnutls",   // wget needs this
    "nettle",   // gnutls dependency (provides libhogweed)
    // SELinux (required by coreutils, systemd, shadow-utils)
    "libselinux",
    "libsemanage",
    "pcre2",
    // Audit (required by shadow-utils)
    "audit-libs",
    // Capabilities and ACLs (required by coreutils)
    "libcap",
    "libcap-ng",
    "libacl",
    "libattr",
    // Compression
    "zlib-ng-compat",
    "libzstd",
    "xz-libs",  // provides liblzma
    "libbrotli",
    // Crypto support
    "libgpg-error",  // required by libgcrypt
    "libffi",
    "libsepol",      // required by libselinux
    // File utilities
    "file-libs",     // libmagic for 'file' command
    // Curl dependencies (curl binary needs all these)
    "libcurl",
    "libidn2",
    "libnghttp2",
    "libpsl",
    "libssh",
    // LDAP
    "openldap",
    // Kerberos
    "krb5-libs",
    "libkadm5",
    "keyutils-libs",
    "libverto",
    // SASL
    "cyrus-sasl-lib",
    // Event
    "libevent",
    // Systemd dependencies
    "libblkid",
    "libfdisk",
    "libmount",
    "libuuid",
    "libsmartcols",
    // Other commonly needed
    "libeconf",
    "libcom_err",    // e2fsprogs
    "lz4-libs",
    "libgcc",
    // Math libraries (gawk needs these)
    "mpfr",
    "gmp",
    // Unicode
    "libunistring",
    // Crypto/password
    "libxcrypt",  // libcrypt.so
    // Additional common libs
    "libtasn1",
    "p11-kit-trust",
    "p11-kit",
    // Note: libsigsegv and libdb are not in Rocky 10 ISO - gawk doesn't need them

    // === PAM (authentication) ===
    "pam",
    "pam-libs",
];

/// Extracts RPM packages to a staging directory.
pub struct RpmExtractor {
    /// Directories containing RPM packages (BaseOS/Packages, AppStream/Packages)
    packages_dirs: Vec<PathBuf>,
    /// Directory to extract packages into
    staging_dir: PathBuf,
}

impl RpmExtractor {
    /// Create a new RPM extractor.
    pub fn new(packages_dir: impl AsRef<Path>, staging_dir: impl AsRef<Path>) -> Self {
        Self {
            packages_dirs: vec![packages_dir.as_ref().to_path_buf()],
            staging_dir: staging_dir.as_ref().to_path_buf(),
        }
    }

    /// Add additional package directories (e.g., AppStream).
    pub fn with_packages_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.packages_dirs.push(dir.as_ref().to_path_buf());
        self
    }

    /// Extract all required packages.
    pub fn extract_all(&self) -> Result<()> {
        self.extract_packages(REQUIRED_PACKAGES)
    }

    /// Extract specified packages.
    pub fn extract_packages(&self, packages: &[&str]) -> Result<()> {
        println!("Extracting {} RPM packages...", packages.len());

        // Create staging directory
        std::fs::create_dir_all(&self.staging_dir)?;

        let mut extracted = 0;
        let mut failed = Vec::new();

        for package in packages {
            match self.extract_package(package) {
                Ok(true) => {
                    extracted += 1;
                }
                Ok(false) => {
                    failed.push(*package);
                }
                Err(e) => {
                    println!("  ERROR extracting {}: {}", package, e);
                    failed.push(*package);
                }
            }
        }

        // FAIL if any packages couldn't be extracted
        if !failed.is_empty() {
            bail!(
                "Failed to extract {} packages: {}\n\
                 The ISO may be incomplete or corrupted.\n\
                 ALL packages are required - no exceptions.",
                failed.len(),
                failed.join(", ")
            );
        }

        println!("  Extracted {}/{} packages", extracted, packages.len());
        Ok(())
    }

    /// Extract a single package by name.
    ///
    /// Searches all package directories (BaseOS, AppStream, etc.) for the RPM.
    /// Returns Ok(true) if extracted, Ok(false) if not found, Err on failure.
    fn extract_package(&self, package_name: &str) -> Result<bool> {
        // Find the RPM file - it's in a subdirectory by first letter
        let first_char = package_name
            .chars()
            .next()
            .context("Empty package name")?
            .to_lowercase()
            .next()
            .unwrap();

        // Search all package directories
        for packages_dir in &self.packages_dirs {
            let subdir = packages_dir.join(first_char.to_string());

            if !subdir.exists() {
                continue;
            }

            // Find matching RPM file
            if let Some(rpm_path) = self.find_rpm_in_dir(&subdir, package_name)? {
                self.extract_rpm(&rpm_path)?;
                println!("  Extracted: {}", package_name);
                return Ok(true);
            }
        }

        // Not found in any directory
        let searched: Vec<_> = self.packages_dirs.iter()
            .map(|p| p.display().to_string())
            .collect();
        println!("  Warning: {} not found in: {}", package_name, searched.join(", "));
        Ok(false)
    }

    /// Find an RPM file matching the package name in a directory.
    fn find_rpm_in_dir(&self, dir: &Path, package_name: &str) -> Result<Option<PathBuf>> {
        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        // Look for exact match first: package-version.arch.rpm
        // e.g., "coreutils" matches "coreutils-9.5-6.el10.x86_64.rpm"
        // but NOT "coreutils-common-9.5-6.el10.x86_64.rpm"
        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            // Must start with package name followed by a dash and a digit (version)
            // This prevents "coreutils" from matching "coreutils-common"
            let expected_prefix = format!("{}-", package_name);
            if filename_str.starts_with(&expected_prefix) {
                // Check if next char after prefix is a digit (version number)
                let rest = &filename_str[expected_prefix.len()..];
                if rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    if filename_str.ends_with(".rpm") {
                        return Ok(Some(entry.path()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Extract an RPM file using rpm2cpio and cpio via shell.
    fn extract_rpm(&self, rpm_path: &Path) -> Result<()> {
        // Use shell to pipe rpm2cpio to cpio - more reliable for large files
        let output = Command::new("sh")
            .args([
                "-c",
                &format!(
                    "rpm2cpio '{}' | cpio -idm --quiet",
                    rpm_path.display()
                ),
            ])
            .current_dir(&self.staging_dir)
            .output()
            .context("Failed to run rpm2cpio | cpio")?;

        if !output.status.success() {
            bail!(
                "RPM extraction failed for {}: {}",
                rpm_path.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Get the staging directory path.
    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }
}

/// Find RPM packages directories from iso-contents.
/// Returns (BaseOS/Packages, Option<AppStream/Packages>).
pub fn find_packages_dirs(iso_contents: impl AsRef<Path>) -> Result<(PathBuf, Option<PathBuf>)> {
    let base = iso_contents.as_ref();
    let baseos = base.join("BaseOS/Packages");
    let appstream = base.join("AppStream/Packages");

    if !baseos.exists() {
        bail!(
            "RPM packages directory not found at {}\n\
             Make sure the ISO is extracted correctly.",
            baseos.display()
        );
    }

    let appstream = if appstream.exists() {
        Some(appstream)
    } else {
        None
    };

    Ok((baseos, appstream))
}

/// Find RPM packages directory from iso-contents (BaseOS only).
/// For backward compatibility.
pub fn find_packages_dir(iso_contents: impl AsRef<Path>) -> Result<PathBuf> {
    let (baseos, _) = find_packages_dirs(iso_contents)?;
    Ok(baseos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_packages_not_empty() {
        assert!(!REQUIRED_PACKAGES.is_empty());
        assert!(REQUIRED_PACKAGES.contains(&"bash"));
        assert!(REQUIRED_PACKAGES.contains(&"coreutils"));
        assert!(REQUIRED_PACKAGES.contains(&"systemd"));
    }
}
