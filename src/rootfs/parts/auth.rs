//! Authentication and privilege escalation binaries.
//!
//! # SINGLE SOURCE OF TRUTH
//!
//! This module defines ALL packages and binaries needed for:
//! - su (switch user)
//! - sudo (superuser do)
//! - Related authentication tools
//!
//! If you add a PAM config for something, the binary MUST be here.
//! If you add a binary here, the package MUST be here.
//!
//! # Why This Module Exists
//!
//! There are THREE separate lists that must stay in sync:
//! 1. REQUIRED_PACKAGES in rpm.rs - what RPM packages to extract
//! 2. BIN/SBIN arrays in binaries.rs - what binaries to copy to tarball
//! 3. critical lists in builder.rs - what binaries to verify exist
//!
//! This module centralizes auth-related entries so they can't drift.
//!
//! # Components
//!
//! 1. AUTH_PACKAGES - RPM packages to extract
//! 2. AUTH_BIN - Binaries to copy to /usr/bin (su, sudo, sudoreplay)
//! 3. AUTH_SBIN - Binaries to copy to /usr/sbin (visudo)
//! 4. SUDO_LIBEXEC - Support libraries for sudo plugins
//! 5. AUTH_CRITICAL_BIN - Binaries to verify in /usr/bin
//! 6. AUTH_CRITICAL_SBIN - Binaries to verify in /usr/sbin
//!
//! # Note on Permissions
//!
//! sudo binary has `--s--x--x` permissions (setuid, not readable).
//! Building must be done as root or sudo needs special handling.

/// RPM packages providing authentication/privilege binaries.
/// Used by: rpm.rs (combined with REQUIRED_PACKAGES)
pub const AUTH_PACKAGES: &[&str] = &[
    "sudo", // Provides: sudo, sudoreplay, visudo, sudoedit
    // Note: shadow-utils is already in main REQUIRED_PACKAGES and provides: su, passwd, etc.
];

/// Authentication binaries for /usr/bin.
/// Used by: binaries.rs copy_coreutils()
pub const AUTH_BIN: &[&str] = &[
    "su",         // From shadow-utils (switch user) - actually in /usr/bin on Rocky
    "sudo",       // From sudo package (superuser do)
    "sudoedit",   // From sudo package (symlink to sudo)
    "sudoreplay", // From sudo package (replay sudo sessions)
];

/// Authentication binaries for /usr/sbin.
/// Used by: binaries.rs copy_sbin_utils()
pub const AUTH_SBIN: &[&str] = &[
    "visudo", // From sudo package (edit sudoers safely)
];

/// Sudo support libraries in /usr/libexec/sudo/.
/// These are loaded dynamically - not discoverable via ldd.
pub const SUDO_LIBEXEC: &[&str] = &[
    "libsudo_util.so.0.0.0",
    "libsudo_util.so.0",
    "libsudo_util.so",
    "sudoers.so",
    "group_file.so",
    "system_group.so",
];

/// Critical auth binaries in /usr/bin that MUST be verified.
/// Build FAILS if ANY are missing - no exceptions.
/// Used by: builder.rs verify_tarball()
pub const AUTH_CRITICAL_BIN: &[&str] = &[
    "su",
    "sudo",
];

/// Critical auth binaries in /usr/sbin that MUST be verified.
/// Used by: builder.rs verify_tarball()
pub const AUTH_CRITICAL_SBIN: &[&str] = &[
    "visudo",
];
