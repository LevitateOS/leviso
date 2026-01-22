//! Shared binary and library utilities.
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
/// ```text
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

/// Find a library in standard paths within a rootfs.
///
/// Searches lib64, lib, and systemd private library paths.
/// The `extra_paths` parameter allows callers to add additional search paths
/// (e.g., `/usr/libexec/sudo` for rootfs builds).
pub fn find_library(source_root: &Path, lib_name: &str, extra_paths: &[&str]) -> Option<PathBuf> {
    // Standard library paths
    let mut candidates = vec![
        source_root.join("usr/lib64").join(lib_name),
        source_root.join("lib64").join(lib_name),
        source_root.join("usr/lib").join(lib_name),
        source_root.join("lib").join(lib_name),
        // Systemd private libraries
        source_root.join("usr/lib64/systemd").join(lib_name),
        source_root.join("usr/lib/systemd").join(lib_name),
    ];

    // Add extra paths from caller
    for extra in extra_paths {
        candidates.push(source_root.join(extra).join(lib_name));
    }

    candidates.into_iter().find(|p| p.exists() || p.is_symlink())
}

/// Recursively get all library dependencies (including transitive).
///
/// Some libraries depend on other libraries. We need to copy all of them.
/// The `extra_lib_paths` parameter is passed to `find_library` for each lookup.
pub fn get_all_dependencies(
    source_root: &Path,
    binary_path: &Path,
    extra_lib_paths: &[&str],
) -> Result<HashSet<String>> {
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
                if let Some(lib_path) = find_library(source_root, &lib_name, extra_lib_paths) {
                    to_process.push(lib_path);
                }
            }
        }
    }

    Ok(all_libs)
}

/// Find a binary in standard bin/sbin directories.
pub fn find_binary(source_root: &Path, binary: &str) -> Option<PathBuf> {
    let bin_candidates = [
        source_root.join("usr/bin").join(binary),
        source_root.join("bin").join(binary),
        source_root.join("usr/sbin").join(binary),
        source_root.join("sbin").join(binary),
    ];

    bin_candidates.into_iter().find(|p| p.exists())
}

/// Find a binary, prioritizing sbin directories.
pub fn find_sbin_binary(source_root: &Path, binary: &str) -> Option<PathBuf> {
    let sbin_candidates = [
        source_root.join("usr/sbin").join(binary),
        source_root.join("sbin").join(binary),
        source_root.join("usr/bin").join(binary),
        source_root.join("bin").join(binary),
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

/// Copy a directory recursively, handling symlinks.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else if path.is_symlink() {
            let target = fs::read_link(&path)?;
            if !dest_path.exists() {
                std::os::unix::fs::symlink(&target, &dest_path)?;
            }
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

/// Copy a library from source to destination, handling symlinks.
///
/// The `dest_lib64_path` and `dest_lib_path` parameters specify where
/// libraries should be copied (e.g., "lib64" for initramfs, "usr/lib64" for rootfs).
pub fn copy_library_to(
    source_root: &Path,
    lib_name: &str,
    dest_root: &Path,
    dest_lib64_path: &str,
    dest_lib_path: &str,
    extra_lib_paths: &[&str],
) -> Result<()> {
    let src = find_library(source_root, lib_name, extra_lib_paths).with_context(|| {
        format!(
            "Could not find library '{}' in source (searched lib64, lib, systemd paths)",
            lib_name
        )
    })?;

    // Check if this is a systemd private library
    let dest_path = if src.to_string_lossy().contains("lib64/systemd")
        || src.to_string_lossy().contains("lib/systemd")
    {
        // Systemd private libraries stay in their own directory
        let dest_dir = dest_root.join(dest_lib64_path).join("systemd");
        fs::create_dir_all(&dest_dir)?;
        dest_dir.join(lib_name)
    } else if src.to_string_lossy().contains("lib64") {
        dest_root.join(dest_lib64_path).join(lib_name)
    } else {
        dest_root.join(dest_lib_path).join(lib_name)
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
            source_root.join(link_target.to_str().unwrap().trim_start_matches('/'))
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
            // Symlink target not found, copy the symlink itself
            fs::copy(&src, &dest_path)?;
        }
    } else {
        fs::copy(&src, &dest_path)?;
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
