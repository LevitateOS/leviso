//! Declarative component system for building LevitateOS system images.
//!
//! This module replaces imperative build code with declarative component definitions.
//! Instead of 2100 lines of copy-paste patterns across 14 files, we have:
//! - ~150 lines of data structures (this file)
//! - ~200 lines of executor (executor.rs)
//! - ~400 lines of component definitions (definitions.rs)
//! - ~50 lines of orchestration (builder.rs)
//!
//! # Architecture
//!
//! Components are defined as static data structures that describe WHAT needs
//! to happen, not HOW. The executor then interprets these definitions.
//!
//! ```text
//! Component Definition (DATA)     →     Executor (LOGIC)
//! ─────────────────────────────        ─────────────────
//! DBUS = Component {                   for op in component.ops {
//!   ops: [                               execute_op(ctx, op)?;
//!     dir("run/dbus"),                 }
//!     bin_required("dbus-broker"),
//!     enable("dbus.socket", Sockets),
//!   ]
//! }
//! ```
//!
//! # Benefits
//!
//! - **Single source of truth**: No more D-Bus socket enabled in 2 places
//! - **Readable at a glance**: Component requirements are obvious
//! - **Consistent behavior**: One implementation of each operation
//! - **Easy to extend**: Add new Op variants, not new copy-paste code

// Allow dead code for API items not yet used (reserved for future components)
#![allow(dead_code)]

pub mod builder;
pub mod custom;
pub mod definitions;
pub mod executor;
pub mod service;

pub use builder::build_system;
pub use service::{Group, Service, Symlink, User};

use std::borrow::Cow;
use std::fmt;

/// Trait for anything that can be installed by the executor.
///
/// Both static `Component` definitions and dynamic `Service` definitions
/// implement this trait.
///
/// Returns `Cow<'static, [Op]>` to avoid heap allocation when returning
/// static slices (Component), while still supporting dynamic ops (Service).
pub trait Installable {
    /// Name for logging.
    fn name(&self) -> &str;
    /// Build phase for ordering.
    fn phase(&self) -> Phase;
    /// Generate the operations to perform.
    ///
    /// Returns `Cow<'static, [Op]>` to allow:
    /// - `Cow::Borrowed` for static `Component` definitions (zero-copy)
    /// - `Cow::Owned` for dynamic `Service` definitions
    fn ops(&self) -> Cow<'static, [Op]>;
}

/// A system component that can be installed.
///
/// Components are immutable, static data describing what operations
/// need to be performed to set up a particular system service.
///
/// For service-type components (with bins, units, users), prefer using
/// the `Service` struct which provides a more ergonomic API.
#[derive(Debug, Clone)]
pub struct Component {
    /// Human-readable name for logging.
    pub name: &'static str,
    /// Build phase (determines ordering).
    pub phase: Phase,
    /// Operations to perform.
    pub ops: &'static [Op],
}

impl Installable for Component {
    fn name(&self) -> &str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn ops(&self) -> Cow<'static, [Op]> {
        Cow::Borrowed(self.ops)
    }
}

impl Installable for &Component {
    fn name(&self) -> &str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn ops(&self) -> Cow<'static, [Op]> {
        Cow::Borrowed(self.ops)
    }
}

impl Installable for Service {
    fn name(&self) -> &str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn ops(&self) -> Cow<'static, [Op]> {
        Cow::Owned(self.ops())
    }
}

/// Build phases determine component ordering.
///
/// Components are sorted by phase before execution. This ensures
/// dependencies are satisfied (e.g., directories exist before files
/// are copied into them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    /// Create FHS directories and merged-usr symlinks.
    Filesystem = 1,
    /// Copy bash, coreutils, sbin utilities.
    Binaries = 2,
    /// Copy systemd binary, units, getty, udev.
    Systemd = 3,
    /// D-Bus message bus (before network services).
    Dbus = 4,
    /// Services: network, chrony, ssh, pam.
    Services = 5,
    /// /etc configuration files.
    Config = 6,
    /// Recipe package manager, dracut.
    Packages = 7,
    /// WiFi firmware, keymaps.
    Firmware = 8,
    /// Final cleanup and setup.
    Final = 9,
}

/// Operations that can be performed during component installation.
///
/// Each variant represents a single atomic operation. The executor
/// handles the actual implementation, ensuring consistent behavior.
///
/// ALL operations are required. If something is listed, it must exist.
/// There is no "optional" - this is a daily driver OS, not a toy.
#[derive(Debug, Clone)]
pub enum Op {
    // ─────────────────────────────────────────────────────────────────────
    // Directory operations
    // ─────────────────────────────────────────────────────────────────────
    /// Create a directory (uses create_dir_all).
    Dir(&'static str),

    /// Create a directory with specific permissions.
    DirMode(&'static str, u32),

    /// Create multiple directories at once.
    Dirs(&'static [&'static str]),

    // ─────────────────────────────────────────────────────────────────────
    // Binary operations - ALL REQUIRED, build fails if missing
    // ─────────────────────────────────────────────────────────────────────
    /// Copy a binary with library dependencies. Fails if not found.
    Bin(&'static str, Dest),

    /// Copy multiple binaries. Fails if ANY are missing.
    Bins(&'static [&'static str], Dest),

    /// Copy bash shell specifically (special handling).
    Bash,

    /// Copy systemd main binary and helpers.
    SystemdBinaries(&'static [&'static str]),

    /// Copy sudo support libraries from /usr/libexec/sudo.
    SudoLibs(&'static [&'static str]),

    // ─────────────────────────────────────────────────────────────────────
    // File operations - ALL REQUIRED, build fails if missing
    // ─────────────────────────────────────────────────────────────────────
    /// Copy a single file from source to staging. Fails if not found.
    CopyFile(&'static str),

    /// Copy a directory tree from source to staging.
    CopyTree(&'static str),

    /// Write a file with given content.
    WriteFile(&'static str, &'static str),

    /// Write a file with specific permissions.
    WriteFileMode(&'static str, &'static str, u32),

    /// Create a symlink (link_path, target).
    Symlink(&'static str, &'static str),

    // ─────────────────────────────────────────────────────────────────────
    // Systemd operations
    // ─────────────────────────────────────────────────────────────────────
    /// Copy systemd unit files (from /usr/lib/systemd/system/).
    Units(&'static [&'static str]),

    /// Copy systemd user unit files (from /usr/lib/systemd/user/).
    /// Used for per-user services like PipeWire.
    UserUnits(&'static [&'static str]),

    /// Enable a unit by creating symlink in target.wants.
    Enable(&'static str, Target),

    /// Copy D-Bus activation symlinks.
    DbusSymlinks(&'static [&'static str]),

    /// Copy udev helpers to /usr/lib/udev.
    UdevHelpers(&'static [&'static str]),

    // ─────────────────────────────────────────────────────────────────────
    // User/group operations
    // ─────────────────────────────────────────────────────────────────────
    /// Ensure a user exists in passwd file.
    User {
        name: &'static str,
        uid: u32,
        gid: u32,
        home: &'static str,
        shell: &'static str,
    },

    /// Ensure a group exists in group file.
    Group { name: &'static str, gid: u32 },

    // ─────────────────────────────────────────────────────────────────────
    // Special operations (index into custom functions)
    // ─────────────────────────────────────────────────────────────────────
    /// Run a custom operation (index into CUSTOM_FNS array).
    Custom(CustomOp),
}

/// Binary destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// /usr/bin
    Bin,
    /// /usr/sbin
    Sbin,
}

/// Systemd target for enabling units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// multi-user.target.wants
    MultiUser,
    /// getty.target.wants
    Getty,
    /// sockets.target.wants
    Sockets,
    /// sysinit.target.wants
    Sysinit,
}

impl Target {
    /// Get the wants directory path for this target.
    pub fn wants_dir(&self) -> &'static str {
        match self {
            Target::MultiUser => "etc/systemd/system/multi-user.target.wants",
            Target::Getty => "etc/systemd/system/getty.target.wants",
            Target::Sockets => "etc/systemd/system/sockets.target.wants",
            Target::Sysinit => "etc/systemd/system/sysinit.target.wants",
        }
    }
}

/// Custom operations that require imperative code.
///
/// These operations have complex logic that doesn't fit the declarative
/// pattern. Each variant maps to a function in custom.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomOp {
    /// Create FHS symlinks (merged /usr).
    CreateFhsSymlinks,
    /// Create live overlay directory.
    CreateLiveOverlay,
    /// Copy WiFi firmware (size tracking, multiple sources).
    CopyWifiFirmware,
    /// Copy all firmware (daily driver support).
    CopyAllFirmware,
    /// Run depmod for kernel modules.
    RunDepmod,
    /// Copy kernel modules.
    CopyModules,
    /// Create /etc configuration files.
    CreateEtcFiles,
    /// Copy timezone data.
    CopyTimezoneData,
    /// Copy locales.
    CopyLocales,
    /// Copy systemd-boot EFI files.
    CopySystemdBootEfi,
    /// Copy keymaps.
    CopyKeymaps,
    /// Create welcome message.
    CreateWelcomeMessage,
    /// Install recstrap/recfstab/recchroot tools via recipes.
    InstallTools,
    /// Disable SELinux.
    DisableSelinux,
    /// Create PAM system-auth and related files.
    CreatePamFiles,
    /// Create security config files.
    CreateSecurityConfig,
    /// Copy recipe binary.
    CopyRecipe,
    /// Setup recipe config.
    SetupRecipeConfig,
    /// Setup live systemd configs.
    SetupLiveSystemdConfigs,
    /// Copy docs-tui binary.
    CopyDocsTui,
    /// Generate SSH host keys during build.
    CreateSshHostKeys,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for readable component definitions
// ─────────────────────────────────────────────────────────────────────────────

/// Create a directory.
pub const fn dir(path: &'static str) -> Op {
    Op::Dir(path)
}

/// Create a directory with specific mode.
pub const fn dir_mode(path: &'static str, mode: u32) -> Op {
    Op::DirMode(path, mode)
}

/// Create multiple directories.
pub const fn dirs(paths: &'static [&'static str]) -> Op {
    Op::Dirs(paths)
}

/// Copy a binary to /usr/bin. Fails if not found.
pub const fn bin(name: &'static str) -> Op {
    Op::Bin(name, Dest::Bin)
}

/// Copy a binary to /usr/sbin. Fails if not found.
pub const fn sbin(name: &'static str) -> Op {
    Op::Bin(name, Dest::Sbin)
}

/// Copy multiple binaries to /usr/bin. Fails if ANY are missing.
pub const fn bins(names: &'static [&'static str]) -> Op {
    Op::Bins(names, Dest::Bin)
}

/// Copy multiple binaries to /usr/sbin. Fails if ANY are missing.
pub const fn sbins(names: &'static [&'static str]) -> Op {
    Op::Bins(names, Dest::Sbin)
}

/// Copy a directory tree.
pub const fn copy_tree(path: &'static str) -> Op {
    Op::CopyTree(path)
}

/// Copy a file. Fails if not found.
pub const fn copy_file(path: &'static str) -> Op {
    Op::CopyFile(path)
}

/// Copy systemd unit files.
pub const fn units(names: &'static [&'static str]) -> Op {
    Op::Units(names)
}

/// Copy systemd user unit files (for per-user services like PipeWire).
pub const fn user_units(names: &'static [&'static str]) -> Op {
    Op::UserUnits(names)
}

/// Enable a unit for multi-user.target.
pub const fn enable_multi_user(unit: &'static str) -> Op {
    Op::Enable(unit, Target::MultiUser)
}

/// Enable a unit for getty.target.
pub const fn enable_getty(unit: &'static str) -> Op {
    Op::Enable(unit, Target::Getty)
}

/// Enable a unit for sockets.target.
pub const fn enable_sockets(unit: &'static str) -> Op {
    Op::Enable(unit, Target::Sockets)
}

/// Enable a unit for sysinit.target.
pub const fn enable_sysinit(unit: &'static str) -> Op {
    Op::Enable(unit, Target::Sysinit)
}

/// Create a symlink.
pub const fn symlink(link: &'static str, target: &'static str) -> Op {
    Op::Symlink(link, target)
}

/// Write a file.
pub const fn write_file(path: &'static str, content: &'static str) -> Op {
    Op::WriteFile(path, content)
}

/// Write a file with permissions.
pub const fn write_file_mode(path: &'static str, content: &'static str, mode: u32) -> Op {
    Op::WriteFileMode(path, content, mode)
}

/// Ensure a user exists.
pub const fn user(
    name: &'static str,
    uid: u32,
    gid: u32,
    home: &'static str,
    shell: &'static str,
) -> Op {
    Op::User {
        name,
        uid,
        gid,
        home,
        shell,
    }
}

/// Ensure a group exists.
pub const fn group(name: &'static str, gid: u32) -> Op {
    Op::Group { name, gid }
}

/// Run a custom operation.
pub const fn custom(op: CustomOp) -> Op {
    Op::Custom(op)
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Phase::Filesystem => write!(f, "Filesystem"),
            Phase::Binaries => write!(f, "Binaries"),
            Phase::Systemd => write!(f, "Systemd"),
            Phase::Dbus => write!(f, "D-Bus"),
            Phase::Services => write!(f, "Services"),
            Phase::Config => write!(f, "Config"),
            Phase::Packages => write!(f, "Packages"),
            Phase::Firmware => write!(f, "Firmware"),
            Phase::Final => write!(f, "Final"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensure Op enum doesn't grow unexpectedly.
    ///
    /// Op uses &'static str and &'static [&'static str] throughout,
    /// avoiding heap allocation. Largest variant determines size.
    #[test]
    fn op_size() {
        let size = std::mem::size_of::<Op>();
        // Largest variant is Op::User with 5 fields:
        // - name, home, shell: &'static str (16 bytes each = 48)
        // - uid, gid: u32 (4 bytes each = 8)
        // + discriminant (1) + padding to align = 64 bytes
        assert!(size <= 64, "Op grew too large: {} bytes (max 64)", size);
        eprintln!("Op size: {} bytes", size);
    }

    /// Ensure Phase enum uses minimal space.
    #[test]
    fn phase_size() {
        let size = std::mem::size_of::<Phase>();
        // #[repr(u8)] should make this 1 byte
        assert_eq!(size, 1, "Phase should be 1 byte (repr(u8))");
    }

    /// Ensure Component struct is compact.
    #[test]
    fn component_size() {
        let size = std::mem::size_of::<Component>();
        // name: &'static str (16)
        // phase: Phase (1)
        // ops: &'static [Op] (16)
        // + padding = ~40 bytes
        assert!(
            size <= 48,
            "Component grew too large: {} bytes (max 48)",
            size
        );
        eprintln!("Component size: {} bytes", size);
    }

    // =============================================================================
    // CRITICAL: Component Phase Ordering Tests
    // =============================================================================

    use leviso_cheat_test::cheat_aware;

    #[cheat_aware(
        protects = "Component phases are correctly ordered",
        severity = "CRITICAL",
        ease = "MEDIUM",
        cheats = [
            "Remove Ord implementation from Phase",
            "Hardcode phase order incorrectly",
            "Skip sorting entirely"
        ],
        consequence = "Components execute out of order - files copied before directories exist"
    )]
    #[test]
    fn test_phase_ordering_is_correct() {
        // Phase ordering must be: Filesystem < Binaries < Systemd < Dbus < Services < Config < Packages < Firmware < Final
        assert!(
            Phase::Filesystem < Phase::Binaries,
            "Filesystem must come before Binaries"
        );
        assert!(
            Phase::Binaries < Phase::Systemd,
            "Binaries must come before Systemd"
        );
        assert!(
            Phase::Systemd < Phase::Dbus,
            "Systemd must come before Dbus"
        );
        assert!(
            Phase::Dbus < Phase::Services,
            "Dbus must come before Services"
        );
        assert!(
            Phase::Services < Phase::Config,
            "Services must come before Config"
        );
        assert!(
            Phase::Config < Phase::Packages,
            "Config must come before Packages"
        );
        assert!(
            Phase::Packages < Phase::Firmware,
            "Packages must come before Firmware"
        );
        assert!(
            Phase::Firmware < Phase::Final,
            "Firmware must come before Final"
        );
    }

    #[cheat_aware(
        protects = "Filesystem phase creates directories before other phases need them",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Skip directory creation",
            "Create directories in wrong phase",
            "Assume directories exist"
        ],
        consequence = "File copies fail with 'No such file or directory' during build"
    )]
    #[test]
    fn test_filesystem_phase_is_first() {
        // Filesystem must be the absolute first phase
        let phases = [
            Phase::Binaries,
            Phase::Systemd,
            Phase::Dbus,
            Phase::Services,
            Phase::Config,
            Phase::Packages,
            Phase::Firmware,
            Phase::Final,
        ];

        for phase in phases {
            assert!(
                Phase::Filesystem < phase,
                "Filesystem must come before {:?}",
                phase
            );
        }
    }
}
