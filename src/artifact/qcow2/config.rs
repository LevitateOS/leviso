//! Configuration operations for qcow2 VM setup.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use distro_builder::process::ensure_exists;
use distro_spec::levitate::DEFAULT_HOSTNAME;
use distro_spec::shared::partitions::{EFI_FILESYSTEM, ROOT_FILESYSTEM};
use crate::component::custom::read_test_instrumentation;

/// Prepare a modified rootfs for qcow2 with qcow2-specific configuration.
///
/// Copies rootfs-staging to a work directory and applies qcow2-specific changes:
/// - Generate fstab with correct UUIDs
/// - Set empty root password
/// - Configure machine-id for first-boot
/// - Set hostname
/// - Enable services
/// - Remove SSH host keys
pub fn prepare_qcow2_rootfs(
    base_dir: &Path,
    source: &Path,
    target: &Path,
    uuids: &super::helpers::DiskUuids,
) -> Result<()> {
    // Copy rootfs-staging to work directory
    println!("  Copying rootfs-staging...");
    let status = Command::new("cp")
        .args(["-a"])
        .arg(source)
        .arg(target)
        .status()
        .context("Failed to copy rootfs")?;

    if !status.success() {
        anyhow::bail!("Failed to copy rootfs-staging");
    }

    // Apply qcow2-specific configuration
    println!("  Generating /etc/fstab...");
    generate_fstab(target, &uuids.efi_fs_uuid, &uuids.root_fs_uuid)?;

    println!("  Setting empty root password...");
    set_empty_root_password(target)?;

    println!("  Configuring machine-id...");
    configure_machine_id(target)?;

    println!("  Setting hostname...");
    set_hostname(target)?;

    println!("  Enabling services...");
    enable_services(target)?;

    println!("  Installing test instrumentation...");
    install_test_instrumentation(target)?;

    println!("  Removing SSH host keys...");
    regenerate_ssh_keys(target)?;

    // Copy kernel to rootfs /boot (for reference, actual boot uses EFI partition)
    let kernel_src = base_dir.join("output/staging/boot/vmlinuz");
    if kernel_src.exists() {
        let boot_dir = target.join("boot");
        fs::create_dir_all(&boot_dir)?;
        fs::copy(&kernel_src, boot_dir.join("vmlinuz"))?;
    }

    Ok(())
}

/// Generate /etc/fstab with proper UUID entries.
fn generate_fstab(root_dir: &Path, efi_uuid: &str, root_uuid: &str) -> Result<()> {
    let fstab_path = root_dir.join("etc/fstab");

    // Ensure etc directory exists
    fs::create_dir_all(root_dir.join("etc"))?;

    // Note: vfat (EFI partition) has pass 0 because it doesn't support fsck
    // Root ext4 has pass 1 as it's checked first
    let fstab_content = format!(
        "# /etc/fstab - static file system information\n\
         # <device>                                <mount>  <type>  <options>  <dump> <pass>\n\
         UUID={:<36}  /        {}    defaults   0      1\n\
         UUID={:<36}  /boot    {}     defaults   0      0\n",
        root_uuid,
        ROOT_FILESYSTEM,
        efi_uuid,
        EFI_FILESYSTEM,
    );

    fs::write(&fstab_path, fstab_content)?;
    Ok(())
}

/// Set empty root password (like live ISO).
fn set_empty_root_password(root_dir: &Path) -> Result<()> {
    let shadow_path = root_dir.join("etc/shadow");
    ensure_exists(&shadow_path, "/etc/shadow")?;

    let content = fs::read_to_string(&shadow_path)?;
    let mut new_lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("root:") {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() >= 3 {
                new_lines.push(format!("root::{}", parts[2]));
            } else {
                new_lines.push(line.to_string());
            }
        } else {
            new_lines.push(line.to_string());
        }
    }

    fs::write(&shadow_path, new_lines.join("\n") + "\n")?;
    Ok(())
}

/// Configure machine-id for first-boot regeneration.
fn configure_machine_id(root_dir: &Path) -> Result<()> {
    let machine_id_path = root_dir.join("etc/machine-id");
    fs::write(&machine_id_path, "")?;
    Ok(())
}

/// Set default hostname for the VM.
fn set_hostname(root_dir: &Path) -> Result<()> {
    let hostname_path = root_dir.join("etc/hostname");
    fs::write(&hostname_path, format!("{}\n", DEFAULT_HOSTNAME))?;
    Ok(())
}

/// Enable essential services for the VM.
fn enable_services(root_dir: &Path) -> Result<()> {
    let wants_dir = root_dir.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&wants_dir)?;

    let services = [
        ("NetworkManager.service", "/usr/lib/systemd/system/NetworkManager.service"),
        ("sshd.service", "/usr/lib/systemd/system/sshd.service"),
        ("chronyd.service", "/usr/lib/systemd/system/chronyd.service"),
    ];

    for (name, target) in services {
        let link_path = wants_dir.join(name);

        if link_path.symlink_metadata().is_ok() && !link_path.exists() {
            fs::remove_file(&link_path)?;
        }

        if !link_path.exists() {
            let service_path = root_dir.join(target.trim_start_matches('/'));
            if service_path.exists() {
                std::os::unix::fs::symlink(target, &link_path)?;
            }
        }
    }

    // Enable serial console for VM testing (serial-getty@ttyS0.service)
    // This is required for rootfs-tests to interact with the VM
    let getty_wants_dir = root_dir.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants_dir)?;
    let serial_link = getty_wants_dir.join("serial-getty@ttyS0.service");
    if !serial_link.exists() {
        std::os::unix::fs::symlink(
            "/usr/lib/systemd/system/serial-getty@.service",
            &serial_link,
        )?;
    }

    // Create drop-in for serial-getty with autologin
    // Standard approach from: https://wiki.archlinux.org/title/Getty
    let dropin_dir = root_dir.join("etc/systemd/system/serial-getty@ttyS0.service.d");
    fs::create_dir_all(&dropin_dir)?;
    // Name file to sort AFTER serial-getty@.service.d/local.conf which exists in rootfs
    let dropin_file = dropin_dir.join("zz-autologin.conf");
    let dropin_content = "[Service]\n\
ExecStart=\n\
ExecStart=-/sbin/agetty --autologin root --keep-baud 115200,57600,38400,9600 - $TERM\n";
    fs::write(&dropin_file, dropin_content)?;
    println!("    Created autologin drop-in at: {}", dropin_file.display());
    println!("    Drop-in content:\n{}", dropin_content);

    Ok(())
}

/// Install test instrumentation for rootfs-tests compatibility.
/// This script emits ___SHELL_READY___ on serial console, which the test
/// harness uses to know when the shell is ready for commands.
fn install_test_instrumentation(root_dir: &Path) -> Result<()> {
    let profile_d = root_dir.join("etc/profile.d");
    fs::create_dir_all(&profile_d)?;
    let instrumentation = read_test_instrumentation()?;
    fs::write(profile_d.join("00-levitate-test.sh"), instrumentation)?;
    Ok(())
}

/// Remove existing SSH host keys (systemd will regenerate on first boot).
fn regenerate_ssh_keys(root_dir: &Path) -> Result<()> {
    let ssh_dir = root_dir.join("etc/ssh");
    if ssh_dir.exists() {
        for entry in fs::read_dir(&ssh_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("ssh_host_") &&
               (name_str.ends_with("_key") || name_str.ends_with("_key.pub")) {
                fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

// TEAM_151: Extracted configuration functions into dedicated module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fstab_format() {
        let temp_dir = std::env::temp_dir().join("qcow2-test-fstab");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("etc")).unwrap();

        generate_fstab(
            &temp_dir,
            "1234-5678",
            "abcd-efgh-ijkl-mnop",
        ).unwrap();

        let content = fs::read_to_string(temp_dir.join("etc/fstab")).unwrap();
        assert!(content.contains("vfat") && content.contains("0      0"));
        assert!(content.contains("ext4") && content.contains("0      1"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_set_empty_root_password() {
        let temp_dir = std::env::temp_dir().join("qcow2-test-shadow");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("etc")).unwrap();

        let shadow_content = "root:!:19000:0:99999:7:::\nbin:*:19000:0:99999:7:::\n";
        fs::write(temp_dir.join("etc/shadow"), shadow_content).unwrap();

        set_empty_root_password(&temp_dir).unwrap();

        let new_content = fs::read_to_string(temp_dir.join("etc/shadow")).unwrap();
        assert!(new_content.starts_with("root::19000"), "Got: {}", new_content);
        assert!(new_content.contains("bin:*:19000"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_configure_machine_id() {
        let temp_dir = std::env::temp_dir().join("qcow2-test-machine-id");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("etc")).unwrap();

        configure_machine_id(&temp_dir).unwrap();

        let content = fs::read_to_string(temp_dir.join("etc/machine-id")).unwrap();
        assert!(content.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_set_hostname() {
        let temp_dir = std::env::temp_dir().join("qcow2-test-hostname");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("etc")).unwrap();

        set_hostname(&temp_dir).unwrap();

        let content = fs::read_to_string(temp_dir.join("etc/hostname")).unwrap();
        assert_eq!(content.trim(), "levitateos");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
