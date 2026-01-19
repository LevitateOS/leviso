//! User and group management.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Read a UID from the rootfs passwd file.
/// Returns None if user not found.
pub fn read_uid_from_rootfs(rootfs: &Path, username: &str) -> Option<(u32, u32)> {
    let passwd_path = rootfs.join("etc/passwd");
    if let Ok(content) = fs::read_to_string(&passwd_path) {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 && parts[0] == username {
                let uid = parts[2].parse().ok()?;
                let gid = parts[3].parse().ok()?;
                return Some((uid, gid));
            }
        }
    }
    None
}

/// Read a GID from the rootfs group file.
/// Returns None if group not found.
pub fn read_gid_from_rootfs(rootfs: &Path, groupname: &str) -> Option<u32> {
    let group_path = rootfs.join("etc/group");
    if let Ok(content) = fs::read_to_string(&group_path) {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 && parts[0] == groupname {
                return parts[2].parse().ok();
            }
        }
    }
    None
}

/// Ensure a user exists in the initramfs passwd file.
/// Reads UID/GID from rootfs if available, otherwise uses provided defaults.
pub fn ensure_user(
    rootfs: &Path,
    initramfs: &Path,
    username: &str,
    default_uid: u32,
    default_gid: u32,
    home: &str,
    shell: &str,
) -> Result<()> {
    let passwd_path = initramfs.join("etc/passwd");
    let mut passwd = fs::read_to_string(&passwd_path).unwrap_or_default();

    if !passwd.contains(&format!("{}:", username)) {
        // Try to read from rootfs first (BUG FIX: was hardcoded)
        let (uid, gid) = read_uid_from_rootfs(rootfs, username).unwrap_or((default_uid, default_gid));
        let entry = format!("{}:x:{}:{}:{}:{}:{}\n", username, uid, gid, username, home, shell);
        passwd.push_str(&entry);
        fs::write(&passwd_path, passwd)
            .with_context(|| format!("Failed to write passwd for user {}", username))?;
    }

    Ok(())
}

/// Ensure a group exists in the initramfs group file.
/// Reads GID from rootfs if available, otherwise uses provided default.
pub fn ensure_group(
    rootfs: &Path,
    initramfs: &Path,
    groupname: &str,
    default_gid: u32,
) -> Result<()> {
    let group_path = initramfs.join("etc/group");
    let mut group = fs::read_to_string(&group_path).unwrap_or_default();

    if !group.contains(&format!("{}:", groupname)) {
        // Try to read from rootfs first (BUG FIX: was hardcoded)
        let gid = read_gid_from_rootfs(rootfs, groupname).unwrap_or(default_gid);
        let entry = format!("{}:x:{}:\n", groupname, gid);
        group.push_str(&entry);
        fs::write(&group_path, group)
            .with_context(|| format!("Failed to write group for {}", groupname))?;
    }

    Ok(())
}

/// Create initial passwd and group files for root.
pub fn create_root_user(initramfs: &Path) -> Result<()> {
    fs::write(
        initramfs.join("etc/passwd"),
        "root:x:0:0:root:/root:/bin/bash\n",
    )?;
    fs::write(initramfs.join("etc/group"), "root:x:0:\n")?;
    Ok(())
}
