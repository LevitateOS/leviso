//! UKI (Unified Kernel Image) builder.
//!
//! Builds UKIs using `ukify` from systemd. UKIs combine kernel + initramfs + cmdline
//! into a single signed PE binary for simplified boot and Secure Boot support.

use anyhow::Result;
use std::path::{Path, PathBuf};

use distro_builder::process::Cmd;
use distro_spec::levitate::{
    EFI_DEBUG, SELINUX_DISABLE, SERIAL_CONSOLE, VGA_CONSOLE, UKI_ENTRIES, UKI_INSTALLED_ENTRIES,
    OS_NAME, OS_ID, OS_VERSION,
};

/// Build a UKI from kernel + initramfs + cmdline.
///
/// Uses `ukify` from systemd. No fallbacks - install systemd-ukify or fail.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image (vmlinuz)
/// * `initramfs` - Path to the initramfs image
/// * `cmdline` - Kernel command line string
/// * `output` - Path for the output .efi file
pub fn build_uki(
    kernel: &Path,
    initramfs: &Path,
    cmdline: &str,
    output: &Path,
) -> Result<()> {
    println!("  Building UKI: {}", output.display());

    // Write cmdline to temp file (ukify reads from file with @ prefix)
    let cmdline_file = output.with_extension("cmdline");
    std::fs::write(&cmdline_file, cmdline)?;

    // Write os-release to temp file (for branding in boot menu)
    // Without this, ukify uses the host's os-release (e.g., Ultramarine)
    let os_release_file = output.with_extension("os-release");
    let os_release_content = format!(
        "NAME=\"{}\"\n\
         ID={}\n\
         VERSION=\"{}\"\n\
         PRETTY_NAME=\"{} {}\"\n",
        OS_NAME, OS_ID, OS_VERSION, OS_NAME, OS_VERSION
    );
    std::fs::write(&os_release_file, &os_release_content)?;

    let result = Cmd::new("ukify")
        .arg("build")
        .args(["--linux", &kernel.to_string_lossy()])
        .args(["--initrd", &initramfs.to_string_lossy()])
        .args(["--cmdline", &format!("@{}", cmdline_file.display())])
        .args(["--os-release", &format!("@{}", os_release_file.display())])
        .args(["--output", &output.to_string_lossy()])
        .error_msg("ukify failed. Install systemd-ukify: sudo dnf install systemd-ukify")
        .run();

    // Cleanup temp files regardless of result
    let _ = std::fs::remove_file(&cmdline_file);
    let _ = std::fs::remove_file(&os_release_file);

    result?;
    Ok(())
}

/// Build all UKIs for the live ISO (normal, emergency, debug).
///
/// Creates one UKI for each entry defined in `UKI_ENTRIES`.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image
/// * `initramfs` - Path to the initramfs image
/// * `output_dir` - Directory to write UKIs to
/// * `iso_label` - ISO volume label for root= parameter
///
/// # Returns
///
/// Vector of paths to the created UKI files.
pub fn build_live_ukis(
    kernel: &Path,
    initramfs: &Path,
    output_dir: &Path,
    iso_label: &str,
) -> Result<Vec<PathBuf>> {
    println!("Building UKIs for live ISO...");

    // Base cmdline used for all entries
    // efi=debug helps diagnose UKI boot issues by showing EFI stub activity
    let base_cmdline = format!(
        "root=LABEL={} {} {} {} {}",
        iso_label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, EFI_DEBUG
    );

    let mut outputs = Vec::new();

    for entry in UKI_ENTRIES {
        let cmdline = if entry.extra_cmdline.is_empty() {
            base_cmdline.clone()
        } else {
            format!("{} {}", base_cmdline, entry.extra_cmdline)
        };

        let output = output_dir.join(entry.filename);
        build_uki(kernel, initramfs, &cmdline, &output)?;
        outputs.push(output);
    }

    println!("  Created {} UKIs", outputs.len());
    Ok(outputs)
}

/// Build UKIs for installed systems.
///
/// These UKIs use the full initramfs and boot from disk (not ISO).
/// Users copy these to /boot/EFI/Linux/ during installation.
/// systemd-boot auto-discovers UKIs in that directory.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image
/// * `initramfs` - Path to the full initramfs (not the tiny live one!)
/// * `output_dir` - Directory to write UKIs to
///
/// # Cmdline
///
/// Uses `root=LABEL=root rw` - the user must partition with this label.
/// This can be edited at boot time via systemd-boot if needed.
///
/// # Returns
///
/// Vector of paths to the created UKI files.
pub fn build_installed_ukis(
    kernel: &Path,
    initramfs: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    println!("Building UKIs for installed systems...");

    // Base cmdline for installed systems
    // Uses root=LABEL=root - user must label their root partition accordingly
    // Can be edited at boot time if needed (systemd-boot allows editing)
    // efi=debug helps diagnose UKI boot issues by showing EFI stub activity
    let base_cmdline = format!(
        "root=LABEL=root rw {} {} {} {}",
        SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, EFI_DEBUG
    );

    let mut outputs = Vec::new();

    for entry in UKI_INSTALLED_ENTRIES {
        let cmdline = if entry.extra_cmdline.is_empty() {
            base_cmdline.clone()
        } else {
            format!("{} {}", base_cmdline, entry.extra_cmdline)
        };

        let output = output_dir.join(entry.filename);
        build_uki(kernel, initramfs, &cmdline, &output)?;
        outputs.push(output);
    }

    println!("  Created {} installed UKIs", outputs.len());
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_cmdline_format() {
        let label = "TESTISO";
        let cmdline = format!(
            "root=LABEL={} {} {} {} {}",
            label, SERIAL_CONSOLE, VGA_CONSOLE, SELINUX_DISABLE, EFI_DEBUG
        );

        assert!(cmdline.contains("root=LABEL=TESTISO"));
        assert!(cmdline.contains("console=ttyS0"));
        assert!(cmdline.contains("console=tty0"));
        assert!(cmdline.contains("selinux=0"));
        assert!(cmdline.contains("efi=debug"));
    }

    #[test]
    fn test_uki_entries_defined() {
        // Verify all expected entries exist
        assert!(UKI_ENTRIES.len() >= 3);
        assert!(UKI_ENTRIES.iter().any(|e| e.filename == "levitateos-live.efi"));
        assert!(UKI_ENTRIES.iter().any(|e| e.filename == "levitateos-emergency.efi"));
        assert!(UKI_ENTRIES.iter().any(|e| e.filename == "levitateos-debug.efi"));
    }
}
