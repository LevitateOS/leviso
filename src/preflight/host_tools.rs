//! Host tool availability checks.

use std::path::Path;

use crate::process;

use super::types::CheckResult;

/// Check host tools are installed.
pub fn check_host_tools() -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Required tools with package hints
    let required_tools = [
        ("mksquashfs", "squashfs-tools", "Required to create squashfs filesystem"),
        ("unsquashfs", "squashfs-tools", "Required to extract squashfs"),
        ("xorriso", "xorriso", "Required to create ISO image"),
        ("mkfs.fat", "dosfstools", "Required for EFI partition"),
        ("readelf", "binutils", "Required for library dependency detection"),
    ];

    for (tool, package, purpose) in required_tools {
        let result = check_tool_exists(tool, package, purpose, true);
        results.push(result);
    }

    // Optional tools (for testing/development)
    let optional_tools = [
        ("qemu-system-x86_64", "qemu-system-x86", "Required for `leviso run`"),
        ("qemu-img", "qemu-utils", "Required for virtual disk creation"),
    ];

    for (tool, package, purpose) in optional_tools {
        let result = check_tool_exists(tool, package, purpose, false);
        results.push(result);
    }

    // Check for OVMF (UEFI firmware)
    let ovmf_paths = [
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/qemu/OVMF.fd",
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
    ];

    let ovmf_found = ovmf_paths.iter().any(|p| Path::new(p).exists());
    if ovmf_found {
        results.push(CheckResult::pass("OVMF firmware"));
    } else {
        results.push(CheckResult::warn(
            "OVMF firmware",
            "Not found - `leviso run` requires UEFI. Install edk2-ovmf or ovmf package.",
        ));
    }

    results
}

/// Check if a tool exists in PATH.
fn check_tool_exists(tool: &str, package: &str, purpose: &str, required: bool) -> CheckResult {
    match process::which(tool) {
        Some(path) => CheckResult::pass_with(tool, &path),
        None => {
            let msg = format!(
                "Not found. Install '{}' package. {}",
                package, purpose
            );
            if required {
                CheckResult::fail(tool, &msg)
            } else {
                CheckResult::warn(tool, &msg)
            }
        }
    }
}
