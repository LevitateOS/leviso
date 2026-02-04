//! User and group management.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use distro_spec::levitate::ROOT_SHELL;

/// Read a UID from the rootfs passwd file.
///
/// Returns:
/// - Ok(Some((uid, gid))) if user found
/// - Ok(None) if user not found or file doesn't exist
/// - Err if file exists but is corrupted/unreadable
pub fn read_uid_from_rootfs(rootfs: &Path, username: &str) -> Result<Option<(u32, u32)>> {
    let passwd_path = rootfs.join("etc/passwd");

    // File not existing is fine - user just doesn't exist
    if !passwd_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&passwd_path)
        .with_context(|| format!("Failed to read passwd file at {}", passwd_path.display()))?;

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 4 && parts[0] == username {
            let uid: u32 = parts[2].parse().with_context(|| {
                format!(
                    "Corrupted passwd file: invalid UID '{}' for user '{}' at {}",
                    parts[2],
                    username,
                    passwd_path.display()
                )
            })?;
            let gid: u32 = parts[3].parse().with_context(|| {
                format!(
                    "Corrupted passwd file: invalid GID '{}' for user '{}' at {}",
                    parts[3],
                    username,
                    passwd_path.display()
                )
            })?;
            return Ok(Some((uid, gid)));
        }
    }
    Ok(None)
}

/// Read a GID from the rootfs group file.
///
/// Returns:
/// - Ok(Some(gid)) if group found
/// - Ok(None) if group not found or file doesn't exist
/// - Err if file exists but is corrupted/unreadable
pub fn read_gid_from_rootfs(rootfs: &Path, groupname: &str) -> Result<Option<u32>> {
    let group_path = rootfs.join("etc/group");

    // File not existing is fine - group just doesn't exist
    if !group_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&group_path)
        .with_context(|| format!("Failed to read group file at {}", group_path.display()))?;

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && parts[0] == groupname {
            let gid: u32 = parts[2].parse().with_context(|| {
                format!(
                    "Corrupted group file: invalid GID '{}' for group '{}' at {}",
                    parts[2],
                    groupname,
                    group_path.display()
                )
            })?;
            return Ok(Some(gid));
        }
    }
    Ok(None)
}

/// Ensure a user exists in passwd file.
pub fn ensure_user(
    source: &Path,
    staging: &Path,
    username: &str,
    default_uid: u32,
    default_gid: u32,
    home: &str,
    shell: &str,
) -> Result<()> {
    let passwd_path = staging.join("etc/passwd");

    // Read existing passwd file
    // - If file doesn't exist, start with empty string (first user)
    // - If file exists but unreadable, FAIL FAST (don't silently overwrite)
    let mut passwd = if passwd_path.exists() {
        fs::read_to_string(&passwd_path)
            .with_context(|| format!("Failed to read passwd file at {}", passwd_path.display()))?
    } else {
        String::new()
    };

    if !passwd.contains(&format!("{}:", username)) {
        // Try to get UID/GID from source rootfs, fall back to defaults if user doesn't exist
        let (uid, gid) =
            read_uid_from_rootfs(source, username)?.unwrap_or((default_uid, default_gid));
        let entry = format!(
            "{}:x:{}:{}:{}:{}:{}\n",
            username, uid, gid, username, home, shell
        );
        passwd.push_str(&entry);
        fs::write(&passwd_path, passwd)
            .with_context(|| format!("Failed to write passwd for user {}", username))?;
    }
    Ok(())
}

/// Ensure a group exists in group file.
pub fn ensure_group(
    source: &Path,
    staging: &Path,
    groupname: &str,
    default_gid: u32,
) -> Result<()> {
    let group_path = staging.join("etc/group");

    // Read existing group file
    // - If file doesn't exist, start with empty string (first group)
    // - If file exists but unreadable, FAIL FAST (don't silently overwrite)
    let mut group = if group_path.exists() {
        fs::read_to_string(&group_path)
            .with_context(|| format!("Failed to read group file at {}", group_path.display()))?
    } else {
        String::new()
    };

    if !group.contains(&format!("{}:", groupname)) {
        // Try to get GID from source rootfs, fall back to default if group doesn't exist
        let gid = read_gid_from_rootfs(source, groupname)?.unwrap_or(default_gid);
        let entry = format!("{}:x:{}:\n", groupname, gid);
        group.push_str(&entry);
        fs::write(&group_path, group)
            .with_context(|| format!("Failed to write group for {}", groupname))?;
    }
    Ok(())
}

/// Create initial passwd and group files for root.
#[allow(dead_code)] // Used by integration tests
pub fn create_root_user(staging: &Path) -> Result<()> {
    fs::write(
        staging.join("etc/passwd"),
        format!("root:x:0:0:root:/root:{}\n", ROOT_SHELL),
    )?;
    fs::write(staging.join("etc/group"), "root:x:0:\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_mock_rootfs(rootfs: &Path) {
        fs::create_dir_all(rootfs.join("etc")).unwrap();
        fs::write(
            rootfs.join("etc/passwd"),
            "root:x:0:0:root:/root:/bin/bash\ndbus:x:81:81:System message bus:/:/sbin/nologin\n",
        )
        .unwrap();
        fs::write(rootfs.join("etc/group"), "root:x:0:\ndbus:x:81:\n").unwrap();
    }

    #[test]
    fn test_read_uid_from_rootfs_exists() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        create_mock_rootfs(rootfs);

        let result = read_uid_from_rootfs(rootfs, "dbus").unwrap();
        assert!(result.is_some());
        let (uid, gid) = result.unwrap();
        assert_eq!(uid, 81);
        assert_eq!(gid, 81);
    }

    #[test]
    fn test_read_uid_from_rootfs_not_found() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        create_mock_rootfs(rootfs);

        let result = read_uid_from_rootfs(rootfs, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_gid_from_rootfs_exists() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        create_mock_rootfs(rootfs);

        let result = read_gid_from_rootfs(rootfs, "dbus").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 81);
    }

    #[test]
    fn test_ensure_user_creates_entry() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        let initramfs = temp.path().join("initramfs");
        create_mock_rootfs(rootfs);
        fs::create_dir_all(initramfs.join("etc")).unwrap();

        // Start with empty passwd
        fs::write(initramfs.join("etc/passwd"), "").unwrap();

        ensure_user(
            rootfs,
            &initramfs,
            "testuser",
            1000,
            1000,
            "/home/test",
            "/bin/bash",
        )
        .expect("ensure_user should succeed");

        let content = fs::read_to_string(initramfs.join("etc/passwd")).unwrap();
        assert!(content.contains("testuser:x:1000:1000"));
    }

    #[test]
    fn test_ensure_user_idempotent() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        let initramfs = temp.path().join("initramfs");
        create_mock_rootfs(rootfs);
        fs::create_dir_all(initramfs.join("etc")).unwrap();

        // Start with existing user
        fs::write(
            initramfs.join("etc/passwd"),
            "testuser:x:1000:1000:testuser:/home:/bin/bash\n",
        )
        .unwrap();

        // Call ensure_user again
        ensure_user(
            rootfs,
            &initramfs,
            "testuser",
            1000,
            1000,
            "/home",
            "/bin/bash",
        )
        .expect("ensure_user should succeed");

        // Should not duplicate
        let content = fs::read_to_string(initramfs.join("etc/passwd")).unwrap();
        let count = content
            .lines()
            .filter(|line| line.starts_with("testuser:"))
            .count();
        assert_eq!(count, 1, "User should not be duplicated");
    }

    #[test]
    fn test_ensure_group_creates_entry() {
        let temp = TempDir::new().unwrap();
        let rootfs = temp.path();
        let initramfs = temp.path().join("initramfs");
        create_mock_rootfs(rootfs);
        fs::create_dir_all(initramfs.join("etc")).unwrap();

        // Start with empty group file
        fs::write(initramfs.join("etc/group"), "").unwrap();

        ensure_group(rootfs, &initramfs, "testgroup", 1000).expect("ensure_group should succeed");

        let content = fs::read_to_string(initramfs.join("etc/group")).unwrap();
        assert!(content.contains("testgroup:x:1000:"));
    }

    #[test]
    fn test_create_root_user() {
        let temp = TempDir::new().unwrap();
        let initramfs = temp.path();
        fs::create_dir_all(initramfs.join("etc")).unwrap();

        create_root_user(initramfs).expect("create_root_user should succeed");

        let passwd = fs::read_to_string(initramfs.join("etc/passwd")).unwrap();
        let group = fs::read_to_string(initramfs.join("etc/group")).unwrap();

        assert!(passwd.contains("root:x:0:0:root:/root:"));
        assert!(group.contains("root:x:0:"));
    }
}
