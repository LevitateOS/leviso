//! Filesystem structure creation.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::context::BuildContext;

/// Create FHS directory structure in initramfs with merged-usr layout.
/// This matches modern Fedora/Rocky expectations where /bin -> usr/bin, etc.
pub fn create_fhs_structure(initramfs: &Path) -> Result<()> {
    // Create usr directories first
    let usr_dirs = [
        "usr/bin",
        "usr/sbin",
        "usr/lib",
        "usr/lib64",
        "usr/lib/systemd/system",
        "usr/lib64/systemd",
    ];

    for dir in usr_dirs {
        fs::create_dir_all(initramfs.join(dir))
            .with_context(|| format!("Failed to create directory: {}", dir))?;
    }

    // Create merged-usr symlinks (required by modern systemd)
    let symlinks = [
        ("bin", "usr/bin"),
        ("sbin", "usr/sbin"),
        ("lib", "usr/lib"),
        ("lib64", "usr/lib64"),
    ];

    for (link, target) in symlinks {
        let link_path = initramfs.join(link);
        if !link_path.exists() && !link_path.is_symlink() {
            std::os::unix::fs::symlink(target, &link_path)
                .with_context(|| format!("Failed to create {} -> {} symlink", link, target))?;
        }
    }

    // Create other directories
    let dirs = [
        "etc",
        "proc",
        "sys",
        "dev",
        "dev/pts",
        "tmp",
        "root",
        "run",
        "run/lock",
        "var/log",
        "var/tmp",
        "etc/systemd/system",
        "mnt",
    ];

    for dir in dirs {
        fs::create_dir_all(initramfs.join(dir))
            .with_context(|| format!("Failed to create directory: {}", dir))?;
    }

    Ok(())
}

/// Create /var/run -> /run symlink.
pub fn create_var_symlinks(initramfs: &Path) -> Result<()> {
    let var_run = initramfs.join("var/run");
    // BUG FIX: Check if symlink already exists (was failing on recreation)
    if !var_run.exists() && !var_run.is_symlink() {
        std::os::unix::fs::symlink("/run", &var_run)
            .context("Failed to create /var/run symlink")?;
    }
    Ok(())
}

/// Create /bin/sh -> bash symlink.
pub fn create_sh_symlink(initramfs: &Path) -> Result<()> {
    let sh_link = initramfs.join("bin/sh");
    // BUG FIX: Check if symlink already exists
    if !sh_link.exists() && !sh_link.is_symlink() {
        std::os::unix::fs::symlink("bash", &sh_link).context("Failed to create /bin/sh symlink")?;
    }
    Ok(())
}

/// Copy keymaps directory recursively.
pub fn copy_keymaps(ctx: &BuildContext) -> Result<()> {
    let keymaps_src = ctx.rootfs.join("usr/lib/kbd/keymaps");
    let keymaps_dst = ctx.initramfs.join("usr/lib/kbd/keymaps");
    if keymaps_src.exists() {
        println!("Copying keymaps...");
        copy_dir_recursive(&keymaps_src, &keymaps_dst)?;
    }
    Ok(())
}

/// Copy a directory recursively.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

/// Create shell configuration files.
pub fn create_shell_config(initramfs: &Path) -> Result<()> {
    fs::write(
        initramfs.join("etc/profile"),
        r#"
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export PS1='root@leviso:\w# '

# Aliases for power management (live environment doesn't have full polkit)
alias poweroff='systemctl poweroff --force'
alias reboot='systemctl reboot --force'
alias halt='systemctl halt --force'

# Show welcome message on login
cat /etc/motd

cd /root
"#,
    )?;

    fs::write(
        initramfs.join("root/.bashrc"),
        r#"
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export PS1='root@leviso:\w# '

# Aliases for power management (live environment doesn't have full polkit)
alias poweroff='systemctl poweroff --force'
alias reboot='systemctl reboot --force'
alias halt='systemctl halt --force'
"#,
    )?;

    Ok(())
}

/// Copy init script from profile.
pub fn copy_init_script(ctx: &BuildContext) -> Result<()> {
    let init_src = ctx.base_dir.join("profile/init");
    let init_dst = ctx.initramfs.join("init");
    fs::copy(&init_src, &init_dst).context("Failed to copy init script")?;

    super::binary::make_executable(&init_dst)?;
    println!("Copied init script");

    Ok(())
}

/// Copy timezone data for timedatectl.
pub fn copy_zoneinfo(ctx: &BuildContext) -> Result<()> {
    let zoneinfo_src = ctx.rootfs.join("usr/share/zoneinfo");
    let zoneinfo_dst = ctx.initramfs.join("usr/share/zoneinfo");

    if zoneinfo_src.exists() {
        println!("Copying timezone data...");
        copy_dir_recursive(&zoneinfo_src, &zoneinfo_dst)?;

        // Set default timezone to UTC
        let localtime = ctx.initramfs.join("etc/localtime");
        if !localtime.exists() {
            std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime)
                .context("Failed to create /etc/localtime symlink")?;
        }

        // Create timezone config
        fs::write(ctx.initramfs.join("etc/timezone"), "UTC\n")?;

        println!("  Timezone data copied (default: UTC)");
    } else {
        println!("  Warning: Timezone data not found in rootfs");
    }

    Ok(())
}

/// Create welcome message shown on login.
pub fn create_welcome_message(initramfs: &Path) -> Result<()> {
    let motd = r#"
================================================================================
                         Welcome to LevitateOS Live
================================================================================

This is a live environment. To install LevitateOS to disk:

  1. Partition your disk:
     parted -s /dev/vda mklabel gpt
     parted -s /dev/vda mkpart EFI fat32 1MiB 513MiB
     parted -s /dev/vda set 1 esp on
     parted -s /dev/vda mkpart root ext4 513MiB 100%

  2. Format partitions:
     mkfs.fat -F32 /dev/vda1
     mkfs.ext4 -F /dev/vda2

  3. Mount and extract:
     mount /dev/vda2 /mnt && mkdir -p /mnt/boot && mount /dev/vda1 /mnt/boot
     mount /dev/sr0 /media/cdrom
     tar xpf /media/cdrom/levitateos-base.tar.xz -C /mnt

  4. Configure fstab (use your UUIDs from 'blkid'):
     nano /mnt/etc/fstab

  5. Install bootloader:
     bootctl --esp-path=/mnt/boot install

  6. Set root password:
     chroot /mnt passwd

  7. Reboot:
     umount -R /mnt && reboot

Useful commands:
  nmcli device status          - Show network interfaces
  nmcli device wifi list       - Scan for WiFi networks
  timedatectl list-timezones   - List available timezones
  lsblk                        - List block devices

Full documentation: Run 'levitate-docs' after installation
================================================================================
"#;

    fs::write(initramfs.join("etc/motd"), motd)?;

    // Also create issue file for pre-login prompt
    let issue = r#"
LevitateOS Live - \l

"#;
    fs::write(initramfs.join("etc/issue"), issue)?;

    println!("Created welcome message");

    Ok(())
}
