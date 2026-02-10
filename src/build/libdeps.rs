//! Binary and library copying utilities.
//!
//! Uses `readelf -d` to extract library dependencies (cross-compilation safe).

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::context::BuildContext;
use distro_builder::process::shell_in;
use distro_builder::LicenseTracker;
use leviso_elf::copy_library_to;

// Re-export commonly used functions
pub use leviso_elf::{find_binary, find_sbin_binary, get_all_dependencies, make_executable};

/// Extra library paths (includes sudo private libs, man-db libs, and pulseaudio libs).
const EXTRA_LIB_PATHS: &[&str] = &[
    "usr/libexec/sudo",
    "usr/lib64/man-db",
    "usr/lib64/pulseaudio",
];

/// Private library directories that should preserve their subdirectory structure.
/// For LevitateOS (systemd-based), this is ["systemd"].
const PRIVATE_LIB_DIRS: &[&str] = &["systemd"];

/// Known RPM locations for binaries not in the minimal rootfs.
const RPM_BINARY_SOURCES: &[(&str, &str, &str)] = &[
    ("passwd", "shadow-utils-*.rpm", "usr/bin/passwd"),
    ("nano", "nano-*.rpm", "usr/bin/nano"),
];

/// Copy a library from source rootfs to staging.
pub fn copy_library(
    ctx: &BuildContext,
    lib_name: &str,
    tracker: Option<&LicenseTracker>,
) -> Result<()> {
    if let Some(t) = tracker {
        t.register_library(lib_name);
    }
    copy_library_to(
        &ctx.source,
        lib_name,
        &ctx.staging,
        "usr/lib64",
        "usr/lib",
        EXTRA_LIB_PATHS,
        PRIVATE_LIB_DIRS,
    )
}

/// Copy a binary and its library dependencies to staging.
/// Returns Ok(false) if binary not found, Ok(true) if copied successfully.
pub fn copy_binary_with_libs(
    ctx: &BuildContext,
    binary: &str,
    dest_dir: &str,
    tracker: Option<&LicenseTracker>,
) -> Result<bool> {
    // Try rootfs first, then RPM extraction
    // NOTE: This function returns Ok(false) when binary not found.
    // The CALLER decides if missing binary is an error. No warnings here.
    let bin_path = match find_binary(&ctx.source, binary) {
        Some(p) => p,
        None => match extract_binary_from_rpm(ctx, binary) {
            Some(p) => {
                println!("  Extracted {} from RPM", binary);
                p
            }
            None => {
                // Don't warn - caller decides if this is an error
                return Ok(false);
            }
        },
    };

    // Register binary for license tracking
    if let Some(t) = tracker {
        t.register_binary(binary);
    }

    // Copy binary
    let dest = ctx.staging.join(dest_dir).join(binary);
    if dest.symlink_metadata().is_err() {
        fs::create_dir_all(dest.parent().unwrap())?;
        if bin_path.is_symlink() {
            let link_target = fs::read_link(&bin_path)?;
            std::os::unix::fs::symlink(&link_target, &dest)?;
            if let Some(target_name) = link_target.file_name() {
                let target_name = target_name.to_string_lossy();
                if find_binary(&ctx.source, &target_name).is_some() {
                    let _ = copy_binary_with_libs(ctx, &target_name, dest_dir, tracker);
                }
            }
            return Ok(true);
        }
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    // Copy all library dependencies
    let libs = get_all_dependencies(&ctx.source, &bin_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name, tracker)
            .with_context(|| format!("'{}' requires missing library '{}'", binary, lib_name))?;
    }

    Ok(true)
}

/// Copy a sbin binary and its library dependencies.
/// NOTE: Returns Ok(false) when binary not found - caller decides if that's an error.
pub fn copy_sbin_binary_with_libs(
    ctx: &BuildContext,
    binary: &str,
    tracker: Option<&LicenseTracker>,
) -> Result<bool> {
    let bin_path = match find_sbin_binary(&ctx.source, binary) {
        Some(p) => p,
        None => match extract_binary_from_rpm(ctx, binary) {
            Some(p) => {
                println!("  Extracted {} from RPM", binary);
                p
            }
            None => {
                // Don't warn - caller decides if this is an error
                return Ok(false);
            }
        },
    };

    // Register binary for license tracking
    if let Some(t) = tracker {
        t.register_binary(binary);
    }

    let dest = ctx.staging.join("usr/sbin").join(binary);
    if dest.symlink_metadata().is_err() {
        fs::create_dir_all(dest.parent().unwrap())?;
        if bin_path.is_symlink() {
            // Preserve symlinks (e.g., mkfs.ntfs -> mkntfs)
            let link_target = fs::read_link(&bin_path)?;
            std::os::unix::fs::symlink(&link_target, &dest)?;
            // Also copy the symlink target if it exists in the rootfs
            if let Some(target_name) = link_target.file_name() {
                let target_name = target_name.to_string_lossy();
                // Recursively ensure the target is also copied
                if find_sbin_binary(&ctx.source, &target_name).is_some() {
                    let _ = copy_sbin_binary_with_libs(ctx, &target_name, tracker);
                }
            }
            return Ok(true);
        }
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    let libs = get_all_dependencies(&ctx.source, &bin_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name, tracker)
            .with_context(|| format!("'{}' requires missing library '{}'", binary, lib_name))?;
    }

    Ok(true)
}

/// Copy bash and its dependencies. FAILS if bash not found.
pub fn copy_bash(ctx: &BuildContext, tracker: Option<&LicenseTracker>) -> Result<()> {
    let bash_candidates = [ctx.source.join("usr/bin/bash"), ctx.source.join("bin/bash")];
    let bash_path = bash_candidates
        .iter()
        .find(|p| p.exists())
        .context("CRITICAL: bash not found in source rootfs")?;

    println!("Found bash at: {}", bash_path.display());

    // Register bash for license tracking
    if let Some(t) = tracker {
        t.register_binary("bash");
    }

    let bash_dest = ctx.staging.join("usr/bin/bash");
    fs::create_dir_all(bash_dest.parent().unwrap())?;
    fs::copy(bash_path, &bash_dest)?;
    make_executable(&bash_dest)?;

    let libs = get_all_dependencies(&ctx.source, bash_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name, tracker)
            .with_context(|| format!("bash requires missing library '{}'", lib_name))?;
    }

    Ok(())
}

/// Extract a binary from an RPM when it's not in the rootfs.
fn extract_binary_from_rpm(ctx: &BuildContext, binary: &str) -> Option<PathBuf> {
    let rpm_info = RPM_BINARY_SOURCES
        .iter()
        .find(|(name, _, _)| *name == binary)?;
    let (_name, rpm_pattern, path_in_rpm) = *rpm_info;

    let packages_dir = ctx.base_dir.join("downloads/iso-contents/BaseOS/Packages");
    let rpm_path = find_rpm_by_pattern(&packages_dir, rpm_pattern)?;

    let extract_dir = ctx.base_dir.join("output/rpm-tmp");
    let _ = fs::create_dir_all(&extract_dir);

    let cmd = format!(
        "rpm2cpio '{}' | cpio -idm './{}'",
        rpm_path.display(),
        path_in_rpm
    );

    if shell_in(&cmd, &extract_dir).is_err() {
        return None;
    }

    let extracted_path = extract_dir.join(path_in_rpm);
    if extracted_path.exists() {
        Some(extracted_path)
    } else {
        None
    }
}

/// Find an RPM file matching a glob pattern.
///
/// Logs warnings on I/O errors but returns None (to allow fallback chains).
fn find_rpm_by_pattern(packages_dir: &Path, pattern: &str) -> Option<PathBuf> {
    let dir_entries = match fs::read_dir(packages_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!(
                "  [WARN] Failed to read packages directory {}: {}",
                packages_dir.display(),
                e
            );
            return None;
        }
    };

    for entry in dir_entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "  [WARN] Failed to read directory entry in {}: {}",
                    packages_dir.display(),
                    e
                );
                continue;
            }
        };
        let subdir = entry.path();
        if subdir.is_dir() {
            let subdir_entries = match fs::read_dir(&subdir) {
                Ok(entries) => entries,
                Err(e) => {
                    eprintln!(
                        "  [WARN] Failed to read subdirectory {}: {}",
                        subdir.display(),
                        e
                    );
                    continue;
                }
            };
            for rpm_entry in subdir_entries {
                let rpm_entry = match rpm_entry {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!(
                            "  [WARN] Failed to read RPM entry in {}: {}",
                            subdir.display(),
                            e
                        );
                        continue;
                    }
                };
                let rpm_name = rpm_entry.file_name();
                let rpm_name_str = rpm_name.to_string_lossy();
                let prefix = pattern.trim_end_matches("*.rpm").trim_end_matches("-*.rpm");
                if rpm_name_str.starts_with(prefix) && rpm_name_str.ends_with(".rpm") {
                    return Some(rpm_entry.path());
                }
            }
        }
    }
    None
}

/// Copy systemd unit files from source to staging.
/// Returns the number of units copied.
pub fn copy_systemd_units(ctx: &BuildContext, units: &[&str]) -> Result<usize> {
    let src_dir = ctx.source.join("usr/lib/systemd/system");
    let dst_dir = ctx.staging.join("usr/lib/systemd/system");
    fs::create_dir_all(&dst_dir)?;

    let mut copied = 0;
    for unit in units {
        let src = src_dir.join(unit);
        let dst = dst_dir.join(unit);
        if src.exists() && !dst.exists() {
            fs::copy(&src, &dst)?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// Copy a directory tree from source to staging.
/// Creates parent directories as needed. Skips files that already exist.
pub fn copy_dir_tree(ctx: &BuildContext, rel_path: &str) -> Result<usize> {
    let src = ctx.source.join(rel_path);
    let dst = ctx.staging.join(rel_path);

    if !src.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(&dst)?;
    let mut count = 0;

    for entry in fs::read_dir(&src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            let sub_rel = format!("{}/{}", rel_path, entry.file_name().to_string_lossy());
            count += copy_dir_tree(ctx, &sub_rel)?;
        } else if src_path.is_symlink() {
            if !dst_path.exists() && !dst_path.is_symlink() {
                let target = fs::read_link(&src_path)?;
                std::os::unix::fs::symlink(&target, &dst_path)?;
                count += 1;
            }
        } else if !dst_path.exists() {
            fs::copy(&src_path, &dst_path)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Copy a single file from source to staging.
/// Creates parent directories as needed. Returns false if source doesn't exist.
pub fn copy_file(ctx: &BuildContext, rel_path: &str) -> Result<bool> {
    let src = ctx.source.join(rel_path);
    let dst = ctx.staging.join(rel_path);

    if !src.exists() {
        return Ok(false);
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if !dst.exists() {
        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            std::os::unix::fs::symlink(&target, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(true)
}
