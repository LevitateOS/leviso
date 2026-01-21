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
cd /root
"#,
    )?;

    fs::write(
        initramfs.join("root/.bashrc"),
        r#"
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export PS1='root@leviso:\w# '
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
