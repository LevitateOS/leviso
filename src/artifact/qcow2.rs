//! qcow2 VM disk image builder (sudo-free).
//!
//! Creates bootable qcow2 disk images for local VM use.
//! The image is built without requiring root privileges.
//!
//! Build process:
//! 1. Generate UUIDs for partitions upfront
//! 2. Prepare rootfs staging directory with qcow2-specific config
//! 3. Create EFI partition image with mkfs.vfat + mtools
//! 4. Create root partition image with mkfs.ext4 -d (populates from directory)
//! 5. Create disk image with GPT partition table (sfdisk works on files)
//! 6. Splice partition images into disk at correct offsets
//! 7. Convert raw to qcow2 with compression
//!
//! Key insight: We use rootfs-staging/ directly (the source for EROFS),
//! so we don't need to extract EROFS which would require mounting.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use distro_builder::process::{ensure_exists, find_first_existing, Cmd};
use distro_spec::levitate::boot::{boot_entry_with_partuuid, default_loader_config};
use distro_spec::shared::{
    partitions::{EFI_FILESYSTEM, ROOT_FILESYSTEM},
    QCOW2_IMAGE_FILENAME,
};

// =============================================================================
// Constants
// =============================================================================

/// EFI partition size in MB
const EFI_SIZE_MB: u64 = 1024;

/// Sector size in bytes
const SECTOR_SIZE: u64 = 512;

/// GPT and partition alignment (1MB alignment is standard)
const ALIGNMENT_MB: u64 = 1;

/// First partition starts at this offset (1MB for GPT + alignment)
const FIRST_PARTITION_OFFSET_SECTORS: u64 = 2048; // 1MB / 512

// =============================================================================
// Host Tool Verification
// =============================================================================

/// Required host tools for sudo-free qcow2 building.
const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("qemu-img", "qemu-img"),
    ("sfdisk", "util-linux"),
    ("mkfs.vfat", "dosfstools"),
    ("mkfs.ext4", "e2fsprogs"),
    ("mcopy", "mtools"),
    ("mmd", "mtools"),
    ("uuidgen", "util-linux"),
    ("dd", "coreutils"),
];

/// Verify all required host tools are available.
fn check_host_tools() -> Result<()> {
    let mut missing = Vec::new();

    for (tool, package) in REQUIRED_TOOLS {
        let result = Cmd::new("which").arg(tool).allow_fail().run();
        if result.is_err() || !result.unwrap().success() {
            missing.push(format!("  {} (install: {})", tool, package));
        }
    }

    if !missing.is_empty() {
        bail!(
            "Missing required tools:\n{}\n\nInstall them first.",
            missing.join("\n")
        );
    }

    Ok(())
}

// =============================================================================
// UUID Generation
// =============================================================================

/// Generated UUIDs for the disk image.
struct DiskUuids {
    /// Filesystem UUID for root partition (ext4)
    root_fs_uuid: String,
    /// Filesystem UUID for EFI partition (vfat serial)
    efi_fs_uuid: String,
    /// GPT partition UUID for root partition (used in boot entry)
    root_part_uuid: String,
}

impl DiskUuids {
    /// Generate new random UUIDs.
    fn generate() -> Result<Self> {
        Ok(Self {
            root_fs_uuid: generate_uuid()?,
            efi_fs_uuid: generate_vfat_serial()?,
            root_part_uuid: generate_uuid()?,
        })
    }
}

/// Generate a random UUID using uuidgen.
fn generate_uuid() -> Result<String> {
    let output = Command::new("uuidgen")
        .output()
        .context("Failed to run uuidgen")?;

    if !output.status.success() {
        bail!("uuidgen failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_lowercase())
}

/// Generate a random FAT32 volume serial (8 hex chars, e.g., "ABCD-1234").
fn generate_vfat_serial() -> Result<String> {
    let output = Command::new("uuidgen")
        .output()
        .context("Failed to run uuidgen")?;

    if !output.status.success() {
        bail!("uuidgen failed");
    }

    // Take first 8 hex chars and format as XXXX-XXXX
    let uuid = String::from_utf8_lossy(&output.stdout);
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).take(8).collect();
    if hex.len() < 8 {
        bail!("Failed to generate vfat serial");
    }
    Ok(format!("{}-{}", &hex[0..4].to_uppercase(), &hex[4..8].to_uppercase()))
}

// =============================================================================
// Disk Image Building (Sudo-Free)
// =============================================================================

/// Build a qcow2 VM disk image without requiring root.
///
/// # Arguments
/// * `base_dir` - The leviso base directory (contains output/, downloads/)
/// * `disk_size_gb` - Disk size in GB (sparse allocation)
pub fn build_qcow2(base_dir: &Path, disk_size_gb: u32) -> Result<()> {
    println!("=== Building qcow2 VM Image (sudo-free) ===\n");

    // Step 1: Verify host tools
    println!("Checking host tools...");
    check_host_tools()?;

    let output_dir = base_dir.join("output");
    let staging_dir = output_dir.join("rootfs-staging");
    let qcow2_path = output_dir.join(QCOW2_IMAGE_FILENAME);

    // Step 2: Verify rootfs-staging exists (source for rootfs)
    ensure_exists(&staging_dir, "rootfs-staging").with_context(|| {
        "Run 'cargo run -- build rootfs' first to create rootfs-staging."
    })?;

    // Step 3: Generate UUIDs upfront
    println!("Generating partition UUIDs...");
    let uuids = DiskUuids::generate()?;
    println!("  Root FS UUID: {}", uuids.root_fs_uuid);
    println!("  EFI FS UUID:  {}", uuids.efi_fs_uuid);
    println!("  Root PARTUUID: {}", uuids.root_part_uuid);

    // Step 4: Create temporary work directory
    let work_dir = output_dir.join("qcow2-work");
    if work_dir.exists() {
        fs::remove_dir_all(&work_dir)?;
    }
    fs::create_dir_all(&work_dir)?;

    // Step 5: Prepare modified rootfs for qcow2
    println!("\nPreparing rootfs for qcow2...");
    let qcow2_staging = work_dir.join("rootfs");
    prepare_qcow2_rootfs(base_dir, &staging_dir, &qcow2_staging, &uuids)?;

    // Step 6: Create EFI partition image
    println!("\nCreating EFI partition image...");
    let efi_image = work_dir.join("efi.img");
    create_efi_partition(base_dir, &efi_image, &uuids, &qcow2_staging)?;

    // Step 7: Create root partition image
    println!("\nCreating root partition image (this may take a while)...");
    let root_image = work_dir.join("root.img");
    let root_size_mb = (disk_size_gb as u64 * 1024) - EFI_SIZE_MB - (ALIGNMENT_MB * 2);
    create_root_partition(&qcow2_staging, &root_image, root_size_mb, &uuids)?;

    // Step 8: Assemble the disk image
    println!("\nAssembling disk image...");
    let raw_path = work_dir.join("disk.raw");
    assemble_disk(&raw_path, &efi_image, &root_image, disk_size_gb, &uuids)?;

    // Step 9: Convert to qcow2
    println!("\nConverting to qcow2 (with compression)...");
    convert_to_qcow2(&raw_path, &qcow2_path)?;

    // Step 10: Cleanup work directory
    println!("Cleaning up...");
    fs::remove_dir_all(&work_dir)?;

    println!("\n=== qcow2 Image Built ===");
    println!("  Output: {}", qcow2_path.display());
    if let Ok(meta) = fs::metadata(&qcow2_path) {
        println!("  Size: {} MB (sparse)", meta.len() / 1024 / 1024);
    }
    println!("\nTo boot:");
    println!("  qemu-system-x86_64 -enable-kvm -m 4G -cpu host \\");
    println!("    -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/ovmf/OVMF_CODE.fd \\");
    println!("    -drive file={},format=qcow2 \\", qcow2_path.display());
    println!("    -device virtio-vga -device virtio-net-pci,netdev=net0 \\");
    println!("    -netdev user,id=net0");

    Ok(())
}

/// Prepare a modified rootfs for qcow2 with qcow2-specific configuration.
///
/// Copies rootfs-staging to a work directory and applies qcow2-specific changes:
/// - Generate fstab with correct UUIDs
/// - Set empty root password
/// - Configure machine-id for first-boot
/// - Set hostname
/// - Enable services
/// - Remove SSH host keys
fn prepare_qcow2_rootfs(
    base_dir: &Path,
    source: &Path,
    target: &Path,
    uuids: &DiskUuids,
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
        bail!("Failed to copy rootfs-staging");
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

/// Create the EFI partition image using mkfs.vfat and mtools.
fn create_efi_partition(
    base_dir: &Path,
    image_path: &Path,
    uuids: &DiskUuids,
    rootfs: &Path,
) -> Result<()> {
    // Create sparse image file
    let size_bytes = EFI_SIZE_MB * 1024 * 1024;
    {
        let file = fs::File::create(image_path)?;
        file.set_len(size_bytes)?;
    }

    // Format as FAT32 with specific volume ID
    // Volume ID format: XXXXXXXX (8 hex digits, no dash)
    let vol_id = uuids.efi_fs_uuid.replace('-', "");
    let status = Command::new("mkfs.vfat")
        .args(["-F", "32", "-n", "EFI", "-i", &vol_id])
        .arg(image_path)
        .status()
        .context("Failed to run mkfs.vfat")?;

    if !status.success() {
        bail!("mkfs.vfat failed");
    }

    // Create directory structure using mtools
    // mtools uses -i to specify the image file
    mtools_mkdir(image_path, "EFI")?;
    mtools_mkdir(image_path, "EFI/BOOT")?;
    mtools_mkdir(image_path, "EFI/systemd")?;
    mtools_mkdir(image_path, "loader")?;
    mtools_mkdir(image_path, "loader/entries")?;

    // Copy systemd-boot EFI binary
    let boot_candidates = [
        rootfs.join("usr/lib/systemd/boot/efi/systemd-bootx64.efi"),
        PathBuf::from("/usr/lib/systemd/boot/efi/systemd-bootx64.efi"),
    ];
    let systemd_boot_src = find_first_existing(&boot_candidates).ok_or_else(|| {
        anyhow::anyhow!(
            "systemd-boot EFI binary not found.\n\
             Install systemd-boot-unsigned or systemd-ukify package."
        )
    })?;

    mtools_copy(image_path, systemd_boot_src, "EFI/BOOT/BOOTX64.EFI")?;
    mtools_copy(image_path, systemd_boot_src, "EFI/systemd/systemd-bootx64.efi")?;

    // Write loader.conf
    let loader_config = default_loader_config();
    let loader_conf_content = loader_config.to_loader_conf();
    mtools_write_file(image_path, "loader/loader.conf", &loader_conf_content)?;

    // Write boot entry
    let boot_entry = boot_entry_with_partuuid(&uuids.root_part_uuid);
    let entry_content = boot_entry.to_entry_file();
    let entry_filename = format!("loader/entries/{}.conf", boot_entry.filename);
    mtools_write_file(image_path, &entry_filename, &entry_content)?;

    // Copy kernel and initramfs
    let output_dir = base_dir.join("output");
    let staging_dir = output_dir.join("staging");

    let kernel_src = staging_dir.join("boot/vmlinuz");
    ensure_exists(&kernel_src, "Kernel")?;
    mtools_copy(image_path, &kernel_src, "vmlinuz")?;

    // Copy install initramfs (REQUIRED - live initramfs cannot boot installed systems)
    // The live initramfs is designed for ISO boot (mounts EROFS from CDROM).
    // The install initramfs is designed for disk boot (uses systemd to mount root partition).
    let initramfs_src = output_dir.join("initramfs-installed.img");
    if !initramfs_src.exists() {
        anyhow::bail!(
            "Install initramfs not found: {}\n\n\
             The qcow2 image requires the install initramfs (systemd-based).\n\
             The live initramfs (busybox-based) cannot boot an installed system.\n\n\
             Run 'cargo run -- build' to build all artifacts first.",
            initramfs_src.display()
        );
    }

    mtools_copy(image_path, &initramfs_src, "initramfs.img")?;

    Ok(())
}

/// Create a directory in a FAT image using mmd.
fn mtools_mkdir(image: &Path, dir: &str) -> Result<()> {
    let status = Command::new("mmd")
        .args(["-i"])
        .arg(image)
        .arg(format!("::{}", dir))
        .status()
        .context("Failed to run mmd")?;

    // mmd returns error if directory exists, which is fine
    if !status.success() {
        // Ignore "directory exists" errors
    }
    Ok(())
}

/// Copy a file into a FAT image using mcopy.
fn mtools_copy(image: &Path, src: &Path, dest: &str) -> Result<()> {
    let status = Command::new("mcopy")
        .args(["-i"])
        .arg(image)
        .arg(src)
        .arg(format!("::{}", dest))
        .status()
        .with_context(|| format!("Failed to copy {} to {}", src.display(), dest))?;

    if !status.success() {
        bail!("mcopy failed: {} -> {}", src.display(), dest);
    }
    Ok(())
}

/// Write content to a file in a FAT image.
fn mtools_write_file(image: &Path, dest: &str, content: &str) -> Result<()> {
    // Write to temp file first, then mcopy
    let temp = std::env::temp_dir().join(format!("mtools-{}", std::process::id()));
    fs::write(&temp, content)?;

    let result = mtools_copy(image, &temp, dest);
    let _ = fs::remove_file(&temp);
    result
}

/// Create the root partition image using mkfs.ext4 -d.
fn create_root_partition(
    rootfs: &Path,
    image_path: &Path,
    size_mb: u64,
    uuids: &DiskUuids,
) -> Result<()> {
    // Create sparse image file
    let size_bytes = size_mb * 1024 * 1024;
    {
        let file = fs::File::create(image_path)?;
        file.set_len(size_bytes)?;
    }

    // Create ext4 filesystem populated from rootfs directory
    // -d populates from directory without mounting
    // -U sets the UUID
    // -L sets the label
    let status = Command::new("mkfs.ext4")
        .args(["-q", "-L", "root"])
        .args(["-U", &uuids.root_fs_uuid])
        .args(["-d"])
        .arg(rootfs)
        .arg(image_path)
        .status()
        .context("Failed to run mkfs.ext4")?;

    if !status.success() {
        bail!("mkfs.ext4 -d failed. Check that e2fsprogs supports -d flag.");
    }

    Ok(())
}

/// Assemble the final disk image from partition images.
fn assemble_disk(
    disk_path: &Path,
    efi_image: &Path,
    root_image: &Path,
    disk_size_gb: u32,
    uuids: &DiskUuids,
) -> Result<()> {
    let disk_size_bytes = (disk_size_gb as u64) * 1024 * 1024 * 1024;

    // Create sparse disk image
    {
        let file = fs::File::create(disk_path)?;
        file.set_len(disk_size_bytes)?;
    }

    // Write GPT partition table
    // We specify the partition UUID for the root partition
    // sfdisk requires explicit field names for uuid
    let efi_size_sectors = (EFI_SIZE_MB * 1024 * 1024) / SECTOR_SIZE;
    let root_start_sector = FIRST_PARTITION_OFFSET_SECTORS + efi_size_sectors;
    let sfdisk_script = format!(
        "label: gpt\n\
         start={}, size={}, type=U, bootable\n\
         start={}, type=L, uuid={}\n",
        FIRST_PARTITION_OFFSET_SECTORS,
        efi_size_sectors,
        root_start_sector,
        uuids.root_part_uuid.to_uppercase()
    );

    let mut child = Command::new("sfdisk")
        .arg(disk_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to run sfdisk")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(sfdisk_script.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        bail!("sfdisk failed to create partition table");
    }

    // Calculate partition offsets
    // EFI partition: starts at sector 2048 (1MB), size = EFI_SIZE_MB
    let efi_offset_bytes = FIRST_PARTITION_OFFSET_SECTORS * SECTOR_SIZE;
    let efi_size_bytes = EFI_SIZE_MB * 1024 * 1024;

    // Root partition: starts right after EFI (aligned to 1MB)
    let root_offset_sectors = FIRST_PARTITION_OFFSET_SECTORS + efi_size_sectors;
    let root_offset_bytes = root_offset_sectors * SECTOR_SIZE;

    // Copy EFI partition image into disk
    println!("  Writing EFI partition at offset {}...", efi_offset_bytes);
    let status = Command::new("dd")
        .args(["if=".to_string() + &efi_image.to_string_lossy()])
        .args(["of=".to_string() + &disk_path.to_string_lossy()])
        .args(["bs=1M", "conv=notrunc"])
        .arg(format!("seek={}", efi_offset_bytes / (1024 * 1024)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run dd for EFI partition")?;

    if !status.success() {
        bail!("dd failed for EFI partition");
    }

    // Copy root partition image into disk
    println!("  Writing root partition at offset {}...", root_offset_bytes);
    let status = Command::new("dd")
        .args(["if=".to_string() + &root_image.to_string_lossy()])
        .args(["of=".to_string() + &disk_path.to_string_lossy()])
        .args(["bs=1M", "conv=notrunc"])
        .arg(format!("seek={}", root_offset_bytes / (1024 * 1024)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run dd for root partition")?;

    if !status.success() {
        bail!("dd failed for root partition");
    }

    Ok(())
}

/// Convert raw disk to qcow2 with compression.
fn convert_to_qcow2(raw_path: &Path, qcow2_path: &Path) -> Result<()> {
    // Remove existing qcow2 if present
    if qcow2_path.exists() {
        fs::remove_file(qcow2_path)?;
    }

    Cmd::new("qemu-img")
        .args(["convert", "-f", "raw", "-O", "qcow2", "-c"])
        .arg_path(raw_path)
        .arg_path(qcow2_path)
        .error_msg("qemu-img convert failed")
        .run()?;
    Ok(())
}

// =============================================================================
// Configuration Helpers
// =============================================================================

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
    use distro_spec::levitate::DEFAULT_HOSTNAME;
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
    use crate::component::custom::read_test_instrumentation;

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

// =============================================================================
// Verification
// =============================================================================

/// Verify the qcow2 image using fsdbg static checks.
pub fn verify_qcow2(base_dir: &Path) -> Result<()> {
    let qcow2_path = base_dir.join("output").join(QCOW2_IMAGE_FILENAME);
    ensure_exists(&qcow2_path, "qcow2 image")?;

    println!("\n=== Verifying qcow2 Image ===");
    println!("  Image: {}", qcow2_path.display());

    // Basic size check
    let metadata = fs::metadata(&qcow2_path)?;
    let size_mb = metadata.len() / 1024 / 1024;

    if size_mb < 100 {
        bail!(
            "qcow2 image seems too small ({} MB). Build may have failed.",
            size_mb
        );
    }
    println!("  Size: {} MB", size_mb);

    // Note: Full static verification requires mounting (sudo).
    // For now, we just check the file exists and has reasonable size.
    // Users can run `fsdbg verify output/levitateos-x86_64.qcow2 --type qcow2` manually
    // if they want detailed verification.
    println!("  [OK] Basic verification passed");
    println!("\n  For detailed verification, run:");
    println!("    sudo fsdbg verify {} --type qcow2", qcow2_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_tools_list() {
        assert!(!REQUIRED_TOOLS.is_empty());
        for (tool, package) in REQUIRED_TOOLS {
            assert!(!tool.is_empty());
            assert!(!package.is_empty());
        }
    }

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

    #[test]
    fn test_generate_vfat_serial_format() {
        // Can only test format, not randomness
        let serial = generate_vfat_serial().unwrap();
        assert_eq!(serial.len(), 9); // XXXX-XXXX
        assert_eq!(&serial[4..5], "-");
    }

    #[test]
    fn test_partition_constants() {
        // Verify constants are sensible
        assert_eq!(EFI_SIZE_MB, 1024);
        assert_eq!(SECTOR_SIZE, 512);
        assert_eq!(FIRST_PARTITION_OFFSET_SECTORS, 2048); // 1MB
    }
}
