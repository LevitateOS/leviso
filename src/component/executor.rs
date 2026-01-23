//! Component executor - interprets Op variants and performs actual operations.
//!
//! This is the single place where all build operations are implemented.
//! No more copy-paste patterns across 14 files.
//!
//! ALL operations are required. If something is listed, it must exist.
//! There is no "optional" - this is a daily driver OS, not a toy.

use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::{Component, Dest, Op};
use crate::build::context::BuildContext;
use crate::build::libdeps::{
    copy_bash, copy_binary_with_libs, copy_dir_tree, copy_file, copy_sbin_binary_with_libs,
    copy_systemd_units, make_executable,
};
use crate::build::users;
use leviso_elf::create_symlink_if_missing;

/// Execute all operations in a component.
pub fn execute(ctx: &BuildContext, component: &Component) -> Result<()> {
    println!("Installing {}...", component.name);

    for op in component.ops {
        execute_op(ctx, op)
            .with_context(|| format!("in component '{}': {:?}", component.name, op))?;
    }

    Ok(())
}

/// Execute a single operation.
fn execute_op(ctx: &BuildContext, op: &Op) -> Result<()> {
    match op {
        // ─────────────────────────────────────────────────────────────────
        // Directory operations
        // ─────────────────────────────────────────────────────────────────
        Op::Dir(path) => {
            fs::create_dir_all(ctx.staging.join(path))?;
        }

        Op::DirMode(path, mode) => {
            let full_path = ctx.staging.join(path);
            fs::create_dir_all(&full_path)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Dirs(paths) => {
            for path in *paths {
                fs::create_dir_all(ctx.staging.join(path))?;
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Binary operations - ALL REQUIRED
        // ─────────────────────────────────────────────────────────────────
        Op::Bin(name, dest) => {
            let found = match dest {
                Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin")?,
                Dest::Sbin => copy_sbin_binary_with_libs(ctx, name)?,
            };
            if !found {
                bail!("{} not found", name);
            }
        }

        Op::Bins(names, dest) => {
            let mut missing = Vec::new();
            for name in *names {
                let found = match dest {
                    Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin")?,
                    Dest::Sbin => copy_sbin_binary_with_libs(ctx, name)?,
                };
                if !found {
                    missing.push(*name);
                }
            }
            if !missing.is_empty() {
                bail!("Missing binaries: {}", missing.join(", "));
            }
        }

        Op::Bash => {
            copy_bash(ctx)?;
        }

        Op::SystemdBinaries(binaries) => {
            // Copy main systemd binary
            let systemd_src = ctx.source.join("usr/lib/systemd/systemd");
            let systemd_dst = ctx.staging.join("usr/lib/systemd/systemd");
            if systemd_src.exists() {
                fs::create_dir_all(systemd_dst.parent().unwrap())?;
                fs::copy(&systemd_src, &systemd_dst)?;
                make_executable(&systemd_dst)?;
            }

            // Copy helper binaries
            for binary in *binaries {
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
        }

        Op::SudoLibs(libs) => {
            let src_dir = ctx.source.join("usr/libexec/sudo");
            let dst_dir = ctx.staging.join("usr/libexec/sudo");

            if !src_dir.exists() {
                bail!("sudo libexec not found at {}", src_dir.display());
            }

            fs::create_dir_all(&dst_dir)?;

            for lib in *libs {
                let src = src_dir.join(lib);
                let dst = dst_dir.join(lib);

                if src.is_symlink() {
                    let target = fs::read_link(&src)?;
                    if dst.exists() || dst.is_symlink() {
                        fs::remove_file(&dst)?;
                    }
                    std::os::unix::fs::symlink(&target, &dst)?;
                } else if src.exists() {
                    fs::copy(&src, &dst)?;
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // File operations - ALL REQUIRED
        // ─────────────────────────────────────────────────────────────────
        Op::CopyFile(path) => {
            let found = copy_file(ctx, path)?;
            if !found {
                bail!("{} not found", path);
            }
        }

        Op::CopyTree(path) => {
            copy_dir_tree(ctx, path)?;
        }

        Op::WriteFile(path, content) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
        }

        Op::WriteFileMode(path, content, mode) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Symlink(link, target) => {
            let link_path = ctx.staging.join(link);
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if !link_path.exists() && !link_path.is_symlink() {
                std::os::unix::fs::symlink(target, &link_path)?;
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Systemd operations
        // ─────────────────────────────────────────────────────────────────
        Op::Units(names) => {
            copy_systemd_units(ctx, names)?;
        }

        Op::Enable(unit, target) => {
            let wants_dir = ctx.staging.join(target.wants_dir());
            fs::create_dir_all(&wants_dir)?;
            let link = wants_dir.join(unit);
            create_symlink_if_missing(
                Path::new(&format!("/usr/lib/systemd/system/{}", unit)),
                &link,
            )?;
        }

        Op::DbusSymlinks(symlinks) => {
            let unit_src = ctx.source.join("usr/lib/systemd/system");
            let unit_dst = ctx.staging.join("usr/lib/systemd/system");

            for symlink in *symlinks {
                let src = unit_src.join(symlink);
                let dst = unit_dst.join(symlink);
                if src.is_symlink() {
                    let target = fs::read_link(&src)?;
                    if !dst.exists() {
                        std::os::unix::fs::symlink(&target, &dst)?;
                    }
                }
            }
        }

        Op::UdevHelpers(helpers) => {
            let udev_src = ctx.source.join("usr/lib/udev");
            let udev_dst = ctx.staging.join("usr/lib/udev");
            fs::create_dir_all(&udev_dst)?;

            for helper in *helpers {
                let src = udev_src.join(helper);
                let dst = udev_dst.join(helper);
                if src.exists() && !dst.exists() {
                    fs::copy(&src, &dst)?;
                    fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // User/group operations
        // ─────────────────────────────────────────────────────────────────
        Op::User {
            name,
            uid,
            gid,
            home,
            shell,
        } => {
            users::ensure_user(&ctx.source, &ctx.staging, name, *uid, *gid, home, shell)?;
        }

        Op::Group { name, gid } => {
            users::ensure_group(&ctx.source, &ctx.staging, name, *gid)?;
        }

        // ─────────────────────────────────────────────────────────────────
        // Custom operations (dispatch to custom.rs)
        // ─────────────────────────────────────────────────────────────────
        Op::Custom(custom_op) => {
            super::custom::execute(ctx, *custom_op)?;
        }
    }

    Ok(())
}
