//! Binary and library copying utilities.
//!
//! Uses `readelf -d` to extract library dependencies (cross-compilation safe).

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::BuildContext;
use crate::common::binary::copy_library_to;

// Re-export commonly used functions
pub use crate::common::binary::{
    find_binary, find_sbin_binary, get_all_dependencies, make_executable,
};

/// Extra library paths (includes sudo private libs).
const EXTRA_LIB_PATHS: &[&str] = &["usr/libexec/sudo"];

/// Known RPM locations for binaries not in the minimal rootfs.
const RPM_BINARY_SOURCES: &[(&str, &str, &str)] = &[
    ("passwd", "shadow-utils-*.rpm", "usr/bin/passwd"),
    ("nano", "nano-*.rpm", "usr/bin/nano"),
];

/// Copy a library from source rootfs to staging.
pub fn copy_library(ctx: &BuildContext, lib_name: &str) -> Result<()> {
    copy_library_to(
        &ctx.source,
        lib_name,
        &ctx.staging,
        "usr/lib64",
        "usr/lib",
        EXTRA_LIB_PATHS,
    )
}

/// Copy a binary and its library dependencies to staging.
/// Returns Ok(false) if binary not found, Ok(true) if copied successfully.
pub fn copy_binary_with_libs(ctx: &BuildContext, binary: &str, dest_dir: &str) -> Result<bool> {
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

    // Copy binary
    let dest = ctx.staging.join(dest_dir).join(binary);
    if !dest.exists() {
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    // Copy all library dependencies
    let libs = get_all_dependencies(&ctx.source, &bin_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name)
            .with_context(|| format!("'{}' requires missing library '{}'", binary, lib_name))?;
    }

    Ok(true)
}

/// Copy a sbin binary and its library dependencies.
/// NOTE: Returns Ok(false) when binary not found - caller decides if that's an error.
pub fn copy_sbin_binary_with_libs(ctx: &BuildContext, binary: &str) -> Result<bool> {
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

    let dest = ctx.staging.join("usr/sbin").join(binary);
    if !dest.exists() {
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    let libs = get_all_dependencies(&ctx.source, &bin_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name)
            .with_context(|| format!("'{}' requires missing library '{}'", binary, lib_name))?;
    }

    Ok(true)
}

/// Copy bash and its dependencies. FAILS if bash not found.
pub fn copy_bash(ctx: &BuildContext) -> Result<()> {
    let bash_candidates = [
        ctx.source.join("usr/bin/bash"),
        ctx.source.join("bin/bash"),
    ];
    let bash_path = bash_candidates
        .iter()
        .find(|p| p.exists())
        .context("CRITICAL: bash not found in source rootfs")?;

    println!("Found bash at: {}", bash_path.display());

    let bash_dest = ctx.staging.join("usr/bin/bash");
    fs::create_dir_all(bash_dest.parent().unwrap())?;
    fs::copy(bash_path, &bash_dest)?;
    make_executable(&bash_dest)?;

    let libs = get_all_dependencies(&ctx.source, bash_path, EXTRA_LIB_PATHS)?;
    for lib_name in &libs {
        copy_library(ctx, lib_name)
            .with_context(|| format!("bash requires missing library '{}'", lib_name))?;
    }

    Ok(())
}

/// Extract a binary from an RPM when it's not in the rootfs.
fn extract_binary_from_rpm(ctx: &BuildContext, binary: &str) -> Option<PathBuf> {
    let rpm_info = RPM_BINARY_SOURCES.iter().find(|(name, _, _)| *name == binary)?;
    let (_name, rpm_pattern, path_in_rpm) = *rpm_info;

    let packages_dir = ctx.base_dir.join("downloads/iso-contents/BaseOS/Packages");
    let rpm_path = find_rpm_by_pattern(&packages_dir, rpm_pattern)?;

    let extract_dir = ctx.base_dir.join("output/rpm-tmp");
    let _ = fs::create_dir_all(&extract_dir);

    let output = Command::new("sh")
        .current_dir(&extract_dir)
        .args([
            "-c",
            &format!(
                "rpm2cpio '{}' | cpio -idm './{}'",
                rpm_path.display(),
                path_in_rpm
            ),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
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
fn find_rpm_by_pattern(packages_dir: &Path, pattern: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(packages_dir).ok()? {
        let entry = entry.ok()?;
        let subdir = entry.path();
        if subdir.is_dir() {
            for rpm_entry in fs::read_dir(&subdir).ok()? {
                let rpm_entry = rpm_entry.ok()?;
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
