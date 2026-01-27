//! Service abstraction for declarative service component definitions.
//!
//! Services are a higher-level abstraction over Components that captures
//! the common pattern of system services: binaries, configs, units, users.
//!
//! # Example
//!
//! ```ignore
//! static OPENSSH: Service = Service {
//!     name: "openssh",
//!     phase: Phase::Services,
//!     bins: &["ssh", "scp", "sftp", "ssh-keygen"],
//!     sbins: &["sshd"],
//!     units: &["sshd.service", "sshd.socket"],
//!     enable: &[(Target::MultiUser, "sshd.service")],
//!     config_trees: &["etc/ssh", "usr/libexec/openssh"],
//!     config_files: &["etc/pam.d/sshd"],
//!     dirs: &["var/empty/sshd", "run/sshd"],
//!     users: &[User { name: "sshd", uid: 74, gid: 74, home: "/var/empty/sshd", shell: "/usr/sbin/nologin" }],
//!     groups: &[Group { name: "sshd", gid: 74 }],
//!     ..Service::EMPTY
//! };
//! ```

use super::{CustomOp, Op, Phase, Target};

/// A user definition for service accounts.
#[derive(Debug, Clone, Copy)]
pub struct User {
    pub name: &'static str,
    pub uid: u32,
    pub gid: u32,
    pub home: &'static str,
    pub shell: &'static str,
}

/// A group definition.
#[derive(Debug, Clone, Copy)]
pub struct Group {
    pub name: &'static str,
    pub gid: u32,
}

/// A symlink definition.
#[derive(Debug, Clone, Copy)]
pub struct Symlink {
    pub link: &'static str,
    pub target: &'static str,
}

/// High-level service definition that generates Component ops.
///
/// This captures the common pattern for system services:
/// - Binaries (/usr/bin, /usr/sbin)
/// - Systemd units
/// - Configuration files and directories
/// - Runtime directories
/// - Service users and groups
#[derive(Debug, Clone)]
pub struct Service {
    /// Service name (used for logging).
    pub name: &'static str,
    /// Build phase.
    pub phase: Phase,

    // ─────────────────────────────────────────────────────────────────────
    // Binaries
    // ─────────────────────────────────────────────────────────────────────
    /// Binaries to install in /usr/bin.
    pub bins: &'static [&'static str],
    /// Binaries to install in /usr/sbin.
    pub sbins: &'static [&'static str],

    // ─────────────────────────────────────────────────────────────────────
    // Systemd
    // ─────────────────────────────────────────────────────────────────────
    /// Systemd unit files to copy (from /usr/lib/systemd/system/).
    pub units: &'static [&'static str],
    /// Systemd user unit files to copy (from /usr/lib/systemd/user/).
    /// Used for per-user services like PipeWire.
    pub user_units: &'static [&'static str],
    /// Units to enable (target, unit_name).
    pub enable: &'static [(Target, &'static str)],

    // ─────────────────────────────────────────────────────────────────────
    // Configuration
    // ─────────────────────────────────────────────────────────────────────
    /// Directory trees to copy recursively.
    pub config_trees: &'static [&'static str],
    /// Individual files to copy.
    pub config_files: &'static [&'static str],
    /// Directories to create.
    pub dirs: &'static [&'static str],
    /// Symlinks to create.
    pub symlinks: &'static [Symlink],

    // ─────────────────────────────────────────────────────────────────────
    // Users and groups
    // ─────────────────────────────────────────────────────────────────────
    /// Service users to create.
    pub users: &'static [User],
    /// Groups to create.
    pub groups: &'static [Group],

    // ─────────────────────────────────────────────────────────────────────
    // Escape hatch for complex logic
    // ─────────────────────────────────────────────────────────────────────
    /// Custom operations that don't fit the declarative model.
    pub custom: &'static [CustomOp],
}

impl Service {
    /// Empty service for use with struct update syntax.
    ///
    /// ```ignore
    /// static MY_SERVICE: Service = Service {
    ///     name: "my-service",
    ///     bins: &["my-bin"],
    ///     ..Service::EMPTY
    /// };
    /// ```
    pub const EMPTY: Service = Service {
        name: "",
        phase: Phase::Services,
        bins: &[],
        sbins: &[],
        units: &[],
        user_units: &[],
        enable: &[],
        config_trees: &[],
        config_files: &[],
        dirs: &[],
        symlinks: &[],
        users: &[],
        groups: &[],
        custom: &[],
    };

    /// Generate the list of operations for this service.
    pub fn ops(&self) -> Vec<Op> {
        let mut ops = Vec::new();

        // Order matters: dirs first, then files, then enable, then users

        // Directories
        for dir in self.dirs {
            ops.push(Op::Dir(dir));
        }

        // Binaries
        if !self.bins.is_empty() {
            ops.push(Op::Bins(self.bins, super::Dest::Bin));
        }
        if !self.sbins.is_empty() {
            ops.push(Op::Bins(self.sbins, super::Dest::Sbin));
        }

        // Config trees and files
        for tree in self.config_trees {
            ops.push(Op::CopyTree(tree));
        }
        for file in self.config_files {
            ops.push(Op::CopyFile(file));
        }

        // Symlinks
        for symlink in self.symlinks {
            ops.push(Op::Symlink(symlink.link, symlink.target));
        }

        // Systemd units
        if !self.units.is_empty() {
            ops.push(Op::Units(self.units));
        }

        // Systemd user units (per-user services like PipeWire)
        if !self.user_units.is_empty() {
            ops.push(Op::UserUnits(self.user_units));
        }

        // Enable units
        for (target, unit) in self.enable {
            ops.push(Op::Enable(unit, *target));
        }

        // Custom operations
        for custom in self.custom {
            ops.push(Op::Custom(*custom));
        }

        // Users and groups (groups first to ensure GID exists)
        for group in self.groups {
            ops.push(Op::Group {
                name: group.name,
                gid: group.gid,
            });
        }
        for user in self.users {
            ops.push(Op::User {
                name: user.name,
                uid: user.uid,
                gid: user.gid,
                home: user.home,
                shell: user.shell,
            });
        }

        ops
    }
}
