//! Component executor - interprets Op variants and performs actual operations.
//!
//! This module is organized into submodules by operation type:
//! - `binaries` - Binary copy operations (Op::Bin, Op::Bins, Op::Bash, etc.)
//! - `directories` - Directory creation (Op::Dir, Op::DirMode, Op::Dirs)
//! - `files` - File operations (Op::CopyFile, Op::WriteFile, Op::Symlink, etc.)
//! - `systemd` - Systemd operations (Op::Units, Op::Enable, etc.)
//! - `users` - User/group operations (Op::User, Op::Group)
//! - `helpers` - Shared test utilities
//!
//! The executor is the single place where all build operations are implemented.
//! No more copy-paste patterns across 14 files.
//!
//! ALL operations are required. If something is listed, it must exist.
//! There is no "optional" - this is a daily driver OS, not a toy.

mod binaries;
mod directories;
mod files;
mod systemd;
mod users;

#[cfg(test)]
mod helpers;

use anyhow::{Context, Result};

use super::Installable;
use crate::build::context::BuildContext;
use crate::build::licenses::LicenseTracker;

use super::Op;

/// Execute all operations in an installable component.
pub fn execute(
    ctx: &BuildContext,
    component: &impl Installable,
    tracker: &LicenseTracker,
) -> Result<()> {
    let name = component.name();
    let ops = component.ops();

    println!("Installing {}...", name);

    for op in ops.iter() {
        execute_op(ctx, op, tracker)
            .with_context(|| format!("in component '{}': {:?}", name, op))?;
    }

    Ok(())
}

/// Execute a single operation by routing to appropriate handler.
fn execute_op(ctx: &BuildContext, op: &Op, tracker: &LicenseTracker) -> Result<()> {
    match op {
        // Directory operations
        Op::Dir(path) => directories::handle_dir(ctx, path)?,
        Op::DirMode(path, mode) => directories::handle_dirmode(ctx, path, *mode)?,
        Op::Dirs(paths) => directories::handle_dirs(ctx, paths)?,

        // Binary operations - ALL REQUIRED
        Op::Bin(name, dest) => binaries::handle_bin(ctx, name, dest, tracker)?,
        Op::Bins(names, dest) => binaries::handle_bins(ctx, names, dest, tracker)?,
        Op::Bash => binaries::handle_bash(ctx, tracker)?,
        Op::SystemdBinaries(binaries) => binaries::handle_systemd_binaries(ctx, binaries, tracker)?,
        Op::SudoLibs(libs) => binaries::handle_sudo_libs(ctx, libs, tracker)?,

        // File operations - ALL REQUIRED
        Op::CopyFile(path) => files::handle_copyfile(ctx, path)?,
        Op::CopyTree(path) => files::handle_copytree(ctx, path)?,
        Op::WriteFile(path, content) => files::handle_writefile(ctx, path, content)?,
        Op::WriteFileMode(path, content, mode) => {
            files::handle_writefilemode(ctx, path, content, *mode)?
        }
        Op::Symlink(link, target) => files::handle_symlink(ctx, link, target)?,

        // Systemd operations
        Op::Units(names) => systemd::handle_units(ctx, names)?,
        Op::UserUnits(names) => systemd::handle_user_units(ctx, names)?,
        Op::Enable(unit, target) => systemd::handle_enable(ctx, unit, target)?,
        Op::DbusSymlinks(symlinks) => systemd::handle_dbus_symlinks(ctx, symlinks)?,
        Op::UdevHelpers(helpers) => systemd::handle_udev_helpers(ctx, helpers)?,

        // User/group operations
        Op::User {
            name,
            uid,
            gid,
            home,
            shell,
        } => users::handle_user(ctx, name, *uid, *gid, home, shell)?,

        Op::Group { name, gid } => users::handle_group(ctx, name, *gid)?,

        // Custom operations (dispatch to custom.rs)
        Op::Custom(custom_op) => super::custom::execute(ctx, *custom_op, tracker)?,
    }

    Ok(())
}
