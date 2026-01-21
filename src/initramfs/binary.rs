//! Binary and library copying utilities.
//!
//! Uses `readelf -d` instead of `ldd` to extract library dependencies.
//! This works for cross-compilation since readelf reads ELF headers directly
//! without executing the binary (which ldd does via the host dynamic linker).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::BuildContext;

/// Extract library dependencies from an ELF binary using readelf.
///
/// This is architecture-independent - readelf reads the ELF headers directly
/// without executing the binary, unlike ldd which uses the host dynamic linker.
pub fn get_library_dependencies(binary_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("readelf")
        .args(["-d", binary_path.to_str().unwrap()])
        .output()
        .context("Failed to run readelf - is binutils installed?")?;

    if !output.status.success() {
        // Not an ELF binary or readelf failed - return empty list
        return Ok(Vec::new());
    }

    parse_readelf_output(&String::from_utf8_lossy(&output.stdout))
}

/// Parse readelf -d output to extract NEEDED library names.
///
/// Example readelf output:
/// ```
/// Dynamic section at offset 0x2d0e0 contains 28 entries:
///   Tag        Type                         Name/Value
///  0x0000000000000001 (NEEDED)             Shared library: [libtinfo.so.6]
///  0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
/// ```
pub fn parse_readelf_output(output: &str) -> Result<Vec<String>> {
    let mut libs = Vec::new();

    for line in output.lines() {
        // Look for lines containing "(NEEDED)" and "Shared library:"
        if line.contains("(NEEDED)") && line.contains("Shared library:") {
            // Extract library name from [libname.so.X]
            if let Some(start) = line.find('[') {
                if let Some(end) = line.find(']') {
                    let lib_name = &line[start + 1..end];
                    libs.push(lib_name.to_string());
                }
            }
        }
    }

    Ok(libs)
}

/// Recursively get all library dependencies (including transitive).
///
/// Some libraries depend on other libraries. We need to copy all of them.
pub fn get_all_dependencies(rootfs: &Path, binary_path: &Path) -> Result<HashSet<String>> {
    let mut all_libs = HashSet::new();
    let mut to_process = vec![binary_path.to_path_buf()];
    let mut processed = HashSet::new();

    while let Some(path) = to_process.pop() {
        if processed.contains(&path) {
            continue;
        }
        processed.insert(path.clone());

        let deps = get_library_dependencies(&path)?;
        for lib_name in deps {
            if all_libs.insert(lib_name.clone()) {
                // New library - find it and check its dependencies too
                if let Some(lib_path) = find_library(rootfs, &lib_name) {
                    to_process.push(lib_path);
                }
            }
        }
    }

    Ok(all_libs)
}

/// Find a library in the rootfs by name.
fn find_library(rootfs: &Path, lib_name: &str) -> Option<PathBuf> {
    let candidates = [
        rootfs.join("usr/lib64").join(lib_name),
        rootfs.join("lib64").join(lib_name),
        rootfs.join("usr/lib").join(lib_name),
        rootfs.join("lib").join(lib_name),
        // Systemd private libraries
        rootfs.join("usr/lib64/systemd").join(lib_name),
        rootfs.join("usr/lib/systemd").join(lib_name),
    ];

    candidates.into_iter().find(|p| p.exists() || p.is_symlink())
}

/// Copy a library from rootfs to initramfs, handling symlinks.
pub fn copy_library(rootfs: &Path, lib_name: &str, initramfs: &Path) -> Result<()> {
    let src = find_library(rootfs, lib_name).with_context(|| {
        format!(
            "Could not find library '{}' in rootfs (searched lib64, lib, systemd paths)",
            lib_name
        )
    })?;

    // Determine destination path
    let dest_path = if src.to_string_lossy().contains("lib64") {
        initramfs.join("lib64").join(lib_name)
    } else {
        initramfs.join("lib").join(lib_name)
    };

    if dest_path.exists() {
        return Ok(()); // Already copied
    }

    // Handle symlinks - copy both the symlink target and create the symlink
    if src.is_symlink() {
        let link_target = fs::read_link(&src)?;

        // Resolve the actual file
        let actual_src = if link_target.is_relative() {
            src.parent()
                .context("Library path has no parent")?
                .join(&link_target)
        } else {
            rootfs.join(link_target.to_str().unwrap().trim_start_matches('/'))
        };

        if actual_src.exists() {
            // Copy the actual file first
            let target_name = link_target.file_name().unwrap_or(link_target.as_os_str());
            let target_dest = dest_path.parent().unwrap().join(target_name);
            if !target_dest.exists() {
                fs::copy(&actual_src, &target_dest)?;
            }
            // Create symlink
            if !dest_path.exists() {
                std::os::unix::fs::symlink(&link_target, &dest_path)?;
            }
        } else {
            // Symlink target not found, copy the symlink itself (may be broken)
            fs::copy(&src, &dest_path)?;
        }
    } else {
        fs::copy(&src, &dest_path)?;
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

/// Copy a binary and its library dependencies to initramfs.
pub fn copy_binary_with_libs(ctx: &BuildContext, binary: &str) -> Result<()> {
    let bin_path = match find_binary(&ctx.rootfs, binary) {
        Some(p) => p,
        None => {
            println!("  Warning: {} not found, skipping", binary);
            return Ok(());
        }
    };

    // Copy binary
    let dest = ctx.initramfs.join("bin").join(binary);
    if !dest.exists() {
        fs::copy(&bin_path, &dest)?;
        make_executable(&dest)?;
        println!("  Copied binary: {}", binary);
    }

    // Get all library dependencies (including transitive) using readelf
    let libs = get_all_dependencies(&ctx.rootfs, &bin_path)?;

    for lib_name in &libs {
        if let Err(e) = copy_library(&ctx.rootfs, lib_name, &ctx.initramfs) {
            println!("  Warning: Failed to copy library {}: {}", lib_name, e);
        }
    }

    Ok(())
}

/// Copy bash and its dependencies.
pub fn copy_bash(ctx: &BuildContext) -> Result<()> {
    let bash_candidates = [
        ctx.rootfs.join("usr/bin/bash"),
        ctx.rootfs.join("bin/bash"),
    ];
    let bash_path = bash_candidates
        .iter()
        .find(|p| p.exists())
        .context("Could not find bash in rootfs")?;

    println!("Found bash at: {}", bash_path.display());

    // Copy bash
    let bash_dest = ctx.initramfs.join("bin/bash");
    fs::copy(bash_path, &bash_dest)?;
    make_executable(&bash_dest)?;

    // Get all library dependencies using readelf (cross-compilation safe)
    println!("Finding library dependencies with readelf...");
    let libs = get_all_dependencies(&ctx.rootfs, bash_path)?;

    // Copy libraries
    for lib_name in &libs {
        if let Err(e) = copy_library(&ctx.rootfs, lib_name, &ctx.initramfs) {
            println!("  Warning: Failed to copy library {}: {}", lib_name, e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_readelf_output() {
        let output = r#"
Dynamic section at offset 0x2d0e0 contains 28 entries:
  Tag        Type                         Name/Value
 0x0000000000000001 (NEEDED)             Shared library: [libtinfo.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x000000000000000c (INIT)               0x5000
"#;
        let libs = parse_readelf_output(output).unwrap();
        assert_eq!(libs, vec!["libtinfo.so.6", "libc.so.6"]);
    }

    #[test]
    fn test_parse_readelf_empty() {
        let output = "not an ELF file";
        let libs = parse_readelf_output(output).unwrap();
        assert!(libs.is_empty());
    }
}
