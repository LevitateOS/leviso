//! Host tool availability checks.

use std::path::Path;

use distro_builder::process;

use super::types::CheckResult;

/// Check host tools are installed.
pub fn check_host_tools() -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Required tools with package hints
    let required_tools = [
        ("mkfs.erofs", "erofs-utils", "Required to create EROFS filesystem (1.8+ for zstd)"),
        ("xorriso", "xorriso", "Required to create ISO image"),
        ("mkfs.fat", "dosfstools", "Required for EFI partition"),
        ("readelf", "binutils", "Required for library dependency detection"),
        ("ukify", "systemd-ukify", "Required for UKI (Unified Kernel Image) creation"),
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

    // Check for systemd-boot EFI binary (required for UKI boot)
    let systemd_boot_path = distro_spec::levitate::SYSTEMD_BOOT_EFI;
    if Path::new(systemd_boot_path).exists() {
        results.push(CheckResult::pass_with("systemd-boot", systemd_boot_path));
    } else {
        results.push(CheckResult::fail(
            "systemd-boot",
            &format!(
                "Not found at {}. Install: sudo dnf install systemd-boot",
                systemd_boot_path
            ),
        ));
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
