//! Binary and library copying utilities for catalyst.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::BuildContext;

/// Parse ldd output to extract library paths.
/// For "not found" libraries, returns the library name so we can search for it in rootfs.
pub fn parse_ldd_output(output: &str) -> Result<Vec<String>> {
    let mut libs = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // Handle "not found" case - ldd runs on HOST, so library might still be in rootfs
        // Return the library name so copy_library can search for it
        if line.contains("not found") {
            if let Some(lib_name) = line.split_whitespace().next() {
                // Return just the library name, copy_library will search for it
                libs.push(lib_name.to_string());
            }
            continue;
        }

        if line.contains("=>") {
            if let Some(path_part) = line.split("=>").nth(1) {
                if let Some(path) = path_part.split_whitespace().next() {
                    if path.starts_with('/') {
                        libs.push(path.to_string());
                    }
                }
            }
        } else if line.starts_with('/') {
            if let Some(path) = line.split_whitespace().next() {
                libs.push(path.to_string());
            }
        }
    }

    Ok(libs)
}

/// Copy a library from rootfs to staging, handling symlinks.
///
/// NOTE: This will FAIL if the library is not found in the rootfs. We do NOT
/// fall back to the host system to ensure reproducible builds.
pub fn copy_library(rootfs: &Path, lib_path: &str, staging: &Path) -> Result<()> {
    let lib_filename = Path::new(lib_path).file_name().unwrap_or_default();

    // Only look in rootfs - never fall back to host system for reproducible builds
    let src_candidates = [
        rootfs.join(lib_path.trim_start_matches('/')),
        rootfs.join("usr").join(lib_path.trim_start_matches('/')),
        // Standard library paths
        rootfs.join("usr/lib64").join(lib_filename),
        rootfs.join("lib64").join(lib_filename),
        rootfs.join("usr/lib").join(lib_filename),
        rootfs.join("lib").join(lib_filename),
        // Systemd private libraries (libsystemd-shared lives here)
        rootfs.join("usr/lib64/systemd").join(lib_filename),
        rootfs.join("usr/lib/systemd").join(lib_filename),
    ];

    let src = src_candidates
        .iter()
        .find(|p| p.exists())
        .with_context(|| {
            format!(
                "Could not find library '{}' in rootfs. Searched:\n  {}",
                lib_path,
                src_candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n  ")
            )
        })?;

    // Determine destination path based on where we found it
    let lib_filename = Path::new(lib_path)
        .file_name()
        .with_context(|| format!("Library path has no filename: {}", lib_path))?;

    // Check if this is a systemd private library
    let dest_path = if src.to_string_lossy().contains("lib64/systemd")
        || src.to_string_lossy().contains("lib/systemd")
    {
        // Systemd private libraries stay in their own directory
        let dest_dir = staging.join("usr/lib64/systemd");
        fs::create_dir_all(&dest_dir)?;
        dest_dir.join(lib_filename)
    } else if lib_path.contains("lib64") || src.to_string_lossy().contains("lib64") {
        staging.join("usr/lib64").join(lib_filename)
    } else {
        staging.join("usr/lib").join(lib_filename)
    };

    if !dest_path.exists() {
        // Handle symlinks
        if src.is_symlink() {
            let link_target = fs::read_link(src)?;
            // If it's a relative symlink, resolve it
            let actual_src = if link_target.is_relative() {
                src.parent()
                    .with_context(|| format!("Library path has no parent: {}", src.display()))?
                    .join(&link_target)
            } else {
                link_target.clone()
            };

            // Copy the actual file
            if actual_src.exists() {
                fs::copy(&actual_src, &dest_path)?;
            } else {
                // Try in rootfs
                let rootfs_target = rootfs.join(
                    link_target
                        .to_str()
                        .with_context(|| {
                            format!("Link target is not valid UTF-8: {}", link_target.display())
                        })?
                        .trim_start_matches('/'),
                );
                if rootfs_target.exists() {
                    fs::copy(&rootfs_target, &dest_path)?;
                } else {
                    fs::copy(src, &dest_path)?;
                }
            }
        } else {
            fs::copy(src, &dest_path)?;
        }
    }

    Ok(())
}

/// Find a binary in the rootfs.
pub fn find_binary(rootfs: &Path, binary: &str) -> Option<PathBuf> {
    let bin_candidates = [
        rootfs.join("usr/bin").join(binary),
        rootfs.join("bin").join(binary),
        rootfs.join("usr/sbin").join(binary),
        rootfs.join("sbin").join(binary),
    ];

    bin_candidates.into_iter().find(|p| p.exists())
}

/// Find a binary in sbin directories.
pub fn find_sbin_binary(rootfs: &Path, binary: &str) -> Option<PathBuf> {
    let sbin_candidates = [
        rootfs.join("usr/sbin").join(binary),
        rootfs.join("sbin").join(binary),
        rootfs.join("usr/bin").join(binary),
        rootfs.join("bin").join(binary),
    ];

    sbin_candidates.into_iter().find(|p| p.exists())
}

/// Make a file executable (chmod 755).
pub fn make_executable(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata: {}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
        .with_context(|| format!("Failed to set permissions: {}", path.display()))?;
    Ok(())
}

/// Copy a binary and its library dependencies to staging directory.
/// Returns Ok(false) if binary not found (caller decides if critical).
/// Returns Err if binary found but libraries missing (binary would be broken).
pub fn copy_binary_with_libs(ctx: &BuildContext, binary: &str, dest_dir: &str) -> Result<bool> {
    let bin_path = match find_binary(&ctx.source, binary) {
        Some(p) => p,
        None => {
            println!("  Warning: {} not found in rootfs", binary);
            return Ok(false);
        }
    };

    // Copy binary to appropriate destination
    let dest = ctx.staging.join(dest_dir).join(binary);
    if !dest.exists() {
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    // Get and copy its libraries - FAIL if any are missing
    let ldd_output = Command::new("ldd").arg(&bin_path).output();

    if let Ok(output) = ldd_output {
        if output.status.success() {
            let libs = parse_ldd_output(&String::from_utf8_lossy(&output.stdout))?;
            for lib in &libs {
                copy_library(&ctx.source, lib, &ctx.staging)
                    .with_context(|| format!("Binary '{}' requires library '{}' which is missing", binary, lib))?;
            }
        }
    }

    Ok(true)
}

/// Copy a sbin binary and its library dependencies.
/// Returns Ok(false) if binary not found (caller decides if critical).
/// Returns Err if binary found but libraries missing (binary would be broken).
pub fn copy_sbin_binary_with_libs(ctx: &BuildContext, binary: &str) -> Result<bool> {
    let bin_path = match find_sbin_binary(&ctx.source, binary) {
        Some(p) => p,
        None => {
            println!("  Warning: {} not found in rootfs", binary);
            return Ok(false);
        }
    };

    // Copy binary to usr/sbin
    let dest = ctx.staging.join("usr/sbin").join(binary);
    if !dest.exists() {
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
    }

    // Get and copy its libraries - FAIL if any are missing
    let ldd_output = Command::new("ldd").arg(&bin_path).output();

    if let Ok(output) = ldd_output {
        if output.status.success() {
            let libs = parse_ldd_output(&String::from_utf8_lossy(&output.stdout))?;
            for lib in &libs {
                copy_library(&ctx.source, lib, &ctx.staging)
                    .with_context(|| format!("Binary '{}' requires library '{}' which is missing", binary, lib))?;
            }
        }
    }

    Ok(true)
}

/// Copy bash and its dependencies. FAILS if bash or its libraries are missing.
pub fn copy_bash(ctx: &BuildContext) -> Result<()> {
    let bash_candidates = [
        ctx.source.join("usr/bin/bash"),
        ctx.source.join("bin/bash"),
    ];
    let bash_path = bash_candidates
        .iter()
        .find(|p| p.exists())
        .context("CRITICAL: Could not find bash in source rootfs")?;

    println!("Found bash at: {}", bash_path.display());

    // Copy bash
    let bash_dest = ctx.staging.join("usr/bin/bash");
    fs::create_dir_all(bash_dest.parent().unwrap())?;
    fs::copy(bash_path, &bash_dest)?;
    make_executable(&bash_dest)?;

    // Get library dependencies using ldd
    let ldd_output = Command::new("ldd")
        .arg(bash_path)
        .output()
        .context("Failed to run ldd")?;

    let libs = parse_ldd_output(&String::from_utf8_lossy(&ldd_output.stdout))?;

    // Copy libraries - FAIL if any are missing
    for lib in &libs {
        copy_library(&ctx.source, lib, &ctx.staging)
            .with_context(|| format!("bash requires library '{}' which is missing", lib))?;
    }

    Ok(())
}
