//! Filesystem operations for building LevitateOS.
//!
//! These functions take paths directly and are used by integration tests.
//! For production code, see `component::custom::filesystem` which uses `BuildContext`.

// These functions are used by tests in tests/ directory (external test crate).
// The pub visibility is intentional but clippy sees them as unused from main crate.
#![allow(dead_code)]

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Create essential FHS directory structure.
///
/// Creates the basic directory layout needed for a Linux filesystem:
/// - /usr/{bin,sbin,lib,lib64}
/// - /var/{log,tmp,cache,spool}
/// - /etc, /etc/systemd/system
/// - /tmp, /root, /home, /mnt
/// - /run, /run/lock
/// - /proc, /sys, /dev, /dev/pts
/// - Merged usr symlinks: /bin -> usr/bin, /sbin -> usr/sbin, etc.
pub fn create_fhs_structure(root: &Path) -> Result<()> {
    // Create directories
    let dirs = [
        "usr/bin",
        "usr/sbin",
        "usr/lib",
        "usr/lib64",
        "usr/lib/systemd/system",
        "usr/lib64/systemd",
        "var/log",
        "var/tmp",
        "var/cache",
        "var/spool",
        "etc",
        "etc/systemd/system",
        "tmp",
        "root",
        "home",
        "mnt",
        "run",
        "run/lock",
        "proc",
        "sys",
        "dev",
        "dev/pts",
    ];

    for dir in &dirs {
        let path = root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path)
                .with_context(|| format!("Failed to create {}", path.display()))?;
        }
    }

    // Create merged usr symlinks
    let symlinks = [
        ("bin", "usr/bin"),
        ("sbin", "usr/sbin"),
        ("lib", "usr/lib"),
        ("lib64", "usr/lib64"),
    ];

    for (link, target) in &symlinks {
        let link_path = root.join(link);
        if link_path.exists() && !link_path.is_symlink() {
            fs::remove_dir_all(&link_path)?;
        }
        if !link_path.exists() {
            std::os::unix::fs::symlink(target, &link_path)
                .with_context(|| format!("Failed to create /{} symlink", link))?;
        }
    }

    Ok(())
}

/// Create /var symlinks for merged /usr layout.
///
/// Creates:
/// - /var/run -> /run
/// - /var/lock -> /run/lock
pub fn create_var_symlinks(root: &Path) -> Result<()> {
    // Ensure /var exists
    let var_dir = root.join("var");
    if !var_dir.exists() {
        fs::create_dir_all(&var_dir).context("Failed to create /var")?;
    }

    // /var/run -> /run
    let var_run = root.join("var/run");
    if !var_run.exists() && !var_run.is_symlink() {
        std::os::unix::fs::symlink("/run", &var_run)
            .context("Failed to create /var/run symlink")?;
    }

    // /var/lock -> /run/lock
    let var_lock = root.join("var/lock");
    if !var_lock.exists() && !var_lock.is_symlink() {
        std::os::unix::fs::symlink("/run/lock", &var_lock)
            .context("Failed to create /var/lock symlink")?;
    }

    Ok(())
}

/// Create /bin/sh -> bash symlink.
///
/// Also creates merged /usr symlinks if they don't exist:
/// - /bin -> usr/bin
/// - /sbin -> usr/sbin
pub fn create_sh_symlink(root: &Path) -> Result<()> {
    // Ensure usr/bin exists
    let usr_bin = root.join("usr/bin");
    if !usr_bin.exists() {
        fs::create_dir_all(&usr_bin).context("Failed to create /usr/bin")?;
    }

    // /bin -> usr/bin (merged usr)
    let bin_link = root.join("bin");
    if bin_link.exists() && !bin_link.is_symlink() {
        fs::remove_dir_all(&bin_link)?;
    }
    if !bin_link.exists() {
        std::os::unix::fs::symlink("usr/bin", &bin_link)
            .context("Failed to create /bin symlink")?;
    }

    // /usr/bin/sh -> bash
    let sh_link = root.join("usr/bin/sh");
    if !sh_link.exists() && !sh_link.is_symlink() {
        std::os::unix::fs::symlink("bash", &sh_link)
            .context("Failed to create /usr/bin/sh symlink")?;
    }

    Ok(())
}

/// Create shell configuration files.
///
/// Creates basic /etc/profile, /etc/bashrc, and /root/.bashrc.
pub fn create_shell_config(root: &Path) -> Result<()> {
    let etc_dir = root.join("etc");
    if !etc_dir.exists() {
        fs::create_dir_all(&etc_dir).context("Failed to create /etc")?;
    }

    // /etc/profile
    let profile = etc_dir.join("profile");
    if !profile.exists() {
        fs::write(
            &profile,
            r#"# /etc/profile - System-wide shell profile

export PATH="/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin"
export EDITOR="${EDITOR:-vi}"
export PAGER="${PAGER:-less}"

# Load profile.d scripts
if [ -d /etc/profile.d ]; then
    for script in /etc/profile.d/*.sh; do
        [ -r "$script" ] && . "$script"
    done
fi
"#,
        )
        .context("Failed to write /etc/profile")?;
    }

    // /etc/bashrc
    let etc_bashrc = etc_dir.join("bashrc");
    if !etc_bashrc.exists() {
        fs::write(
            &etc_bashrc,
            r#"# /etc/bashrc - System-wide bash configuration

# If not running interactively, don't do anything
[[ $- != *i* ]] && return

# Prompt
PS1='[\u@\h \W]\$ '

# History
HISTSIZE=1000
HISTFILESIZE=2000
HISTCONTROL=ignoreboth

# Aliases
alias ls='ls --color=auto'
alias ll='ls -la'
alias grep='grep --color=auto'
"#,
        )
        .context("Failed to write /etc/bashrc")?;
    }

    // /root/.bashrc
    let root_dir = root.join("root");
    if !root_dir.exists() {
        fs::create_dir_all(&root_dir).context("Failed to create /root")?;
    }
    let root_bashrc = root_dir.join(".bashrc");
    if !root_bashrc.exists() {
        fs::write(
            &root_bashrc,
            r#"# ~/.bashrc - Root user bash configuration

# Source global bashrc
if [ -f /etc/bashrc ]; then
    . /etc/bashrc
fi

# Root-specific settings
PS1='[\u@\h \W]# '
"#,
        )
        .context("Failed to write /root/.bashrc")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_fhs_structure() {
        let temp = TempDir::new().unwrap();
        create_fhs_structure(temp.path()).unwrap();

        assert!(temp.path().join("usr/bin").exists());
        assert!(temp.path().join("usr/sbin").exists());
        assert!(temp.path().join("etc").exists());
        assert!(temp.path().join("var/log").exists());
    }

    #[test]
    fn test_create_var_symlinks() {
        let temp = TempDir::new().unwrap();
        create_fhs_structure(temp.path()).unwrap();
        create_var_symlinks(temp.path()).unwrap();

        assert!(temp.path().join("var/run").is_symlink());
        assert!(temp.path().join("var/lock").is_symlink());
    }

    #[test]
    fn test_create_sh_symlink() {
        let temp = TempDir::new().unwrap();
        create_fhs_structure(temp.path()).unwrap();
        create_sh_symlink(temp.path()).unwrap();

        assert!(temp.path().join("bin").is_symlink());
        assert!(temp.path().join("usr/bin/sh").is_symlink());
    }

    #[test]
    fn test_create_shell_config() {
        let temp = TempDir::new().unwrap();
        create_fhs_structure(temp.path()).unwrap();
        create_shell_config(temp.path()).unwrap();

        assert!(temp.path().join("etc/profile").exists());
        assert!(temp.path().join("etc/bashrc").exists());
    }
}
