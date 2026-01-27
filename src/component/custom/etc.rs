//! /etc configuration file creation.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use leviso_elf::copy_dir_recursive;

use crate::build::context::BuildContext;

// Static config files from profile/etc/
const PASSWD: &str = include_str!("../../../profile/etc/passwd");
const SHADOW: &str = include_str!("../../../profile/etc/shadow");
const GROUP: &str = include_str!("../../../profile/etc/group");
const GSHADOW: &str = include_str!("../../../profile/etc/gshadow");
const FSTAB: &str = include_str!("../../../profile/etc/fstab");
const LOGIN_DEFS: &str = include_str!("../../../profile/etc/login.defs");
const SUDOERS: &str = include_str!("../../../profile/etc/sudoers");
const SUDO_CONF: &str = include_str!("../../../profile/etc/sudo.conf");
const PROFILE: &str = include_str!("../../../profile/etc/profile");
const BASHRC: &str = include_str!("../../../profile/etc/bashrc");
const NSSWITCH: &str = include_str!("../../../profile/etc/nsswitch.conf");
const SHELLS: &str = include_str!("../../../profile/etc/shells");
const XDG_SH: &str = include_str!("../../../profile/etc/profile.d/xdg.sh");
const HOSTS: &str = include_str!("../../../profile/etc/hosts");
const ADJTIME: &str = include_str!("../../../profile/etc/adjtime");
const LOCALE_CONF: &str = include_str!("../../../profile/etc/locale.conf");
const VCONSOLE_CONF: &str = include_str!("../../../profile/etc/vconsole.conf");
const SKEL_BASHRC: &str = include_str!("../../../profile/etc/skel/.bashrc");
const SKEL_BASH_PROFILE: &str = include_str!("../../../profile/etc/skel/.bash_profile");
const ROOT_BASHRC: &str = include_str!("../../../profile/root/.bashrc");
const ROOT_BASH_PROFILE: &str = include_str!("../../../profile/root/.bash_profile");

/// Create all /etc configuration files.
pub fn create_etc_files(ctx: &BuildContext) -> Result<()> {
    println!("Creating /etc configuration files...");

    create_passwd_files(ctx)?;
    create_system_identity(ctx)?;
    create_filesystem_config(ctx)?;
    create_auth_config(ctx)?;
    create_locale_config(ctx)?;
    create_network_config(ctx)?;
    create_shell_config(ctx)?;
    create_nsswitch(ctx)?;
    create_tmpfiles_configs(ctx)?;
    copy_ld_so_conf(ctx)?;

    println!("  Created /etc configuration files");
    Ok(())
}

/// Create custom tmpfiles.d configs for runtime directories.
fn create_tmpfiles_configs(ctx: &BuildContext) -> Result<()> {
    let tmpfiles_dir = ctx.staging.join("usr/lib/tmpfiles.d");

    // sshd needs /run/sshd for privilege separation
    // Can't put it in the rootfs image since /run is tmpfs
    fs::write(
        tmpfiles_dir.join("sshd.conf"),
        "# /run/sshd is needed by sshd for privilege separation\nd /run/sshd 0755 root root -\n",
    )?;

    Ok(())
}

/// Generate SSH host keys for the rootfs.
///
/// Pre-generates SSH host keys so sshd can start immediately without relying
/// on sshd-keygen@.service. This fixes a reproducibility issue where the
/// service doesn't always start correctly, leaving sshd unable to accept
/// connections.
///
/// SECURITY NOTE: For live ISO, shared keys are acceptable since the ISO is
/// public and read-only. For installed systems, these keys should be regenerated
/// during installation (recstrap handles this).
///
/// This was previously documented in KNOWLEDGE_install-test-debugging.md as
/// a manual workaround (R4). Now codified in the build system.
pub fn create_ssh_host_keys(ctx: &BuildContext) -> Result<()> {
    println!("Generating SSH host keys...");

    let ssh_dir = ctx.staging.join("etc/ssh");
    fs::create_dir_all(&ssh_dir)?;

    // Set directory permissions (755)
    fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o755))?;

    // Generate all three key types used by modern sshd
    let key_types = [
        ("rsa", 3072),     // RSA with minimum recommended key size
        ("ecdsa", 256),    // ECDSA with P-256 curve
        ("ed25519", 0),    // Ed25519 (fixed size, no bits param needed)
    ];

    for (key_type, bits) in key_types {
        let key_path = ssh_dir.join(format!("ssh_host_{}_key", key_type));
        let pub_key_path = ssh_dir.join(format!("ssh_host_{}_key.pub", key_type));

        // Check if BOTH private and public keys exist (idempotency)
        // Only skip if both exist - partial state means we need to regenerate
        if key_path.exists() && pub_key_path.exists() {
            println!("  {} key pair already exists, skipping", key_type);
            continue;
        }

        // Remove any partial state before generating
        let _ = fs::remove_file(&key_path);
        let _ = fs::remove_file(&pub_key_path);

        let mut cmd = Command::new("ssh-keygen");
        cmd.arg("-t").arg(key_type)
            .arg("-f").arg(&key_path)
            .arg("-N").arg("")  // Empty passphrase
            .arg("-q");         // Quiet mode

        // Add bits parameter for RSA and ECDSA
        if bits > 0 {
            cmd.arg("-b").arg(bits.to_string());
        }

        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("Failed to generate SSH {} host key", key_type);
        }

        // Verify both files were created
        if !key_path.exists() {
            anyhow::bail!("SSH {} private key was not created", key_type);
        }
        if !pub_key_path.exists() {
            anyhow::bail!("SSH {} public key was not created", key_type);
        }

        // Set correct permissions on private key (600) and public key (644)
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        fs::set_permissions(&pub_key_path, fs::Permissions::from_mode(0o644))?;

        println!("  Generated {} key pair", key_type);
    }

    println!("  SSH host keys ready (sshd can start immediately)");
    Ok(())
}

fn create_passwd_files(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(etc.join("passwd"), PASSWD)?;
    fs::write(etc.join("shadow"), SHADOW)?;

    let mut perms = fs::metadata(etc.join("shadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("shadow"), perms)?;

    fs::write(etc.join("group"), GROUP)?;
    fs::write(etc.join("gshadow"), GSHADOW)?;

    let mut perms = fs::metadata(etc.join("gshadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("gshadow"), perms)?;

    Ok(())
}

fn create_system_identity(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    let name = std::env::var("OS_NAME").unwrap_or_else(|_| "LevitateOS".to_string());
    let id = std::env::var("OS_ID").unwrap_or_else(|_| "levitateos".to_string());
    let id_like = std::env::var("OS_ID_LIKE").unwrap_or_else(|_| "fedora".to_string());
    let version = std::env::var("OS_VERSION").unwrap_or_else(|_| "1.0".to_string());
    let version_id = std::env::var("OS_VERSION_ID").unwrap_or_else(|_| "1".to_string());
    let home_url = std::env::var("OS_HOME_URL").unwrap_or_else(|_| "https://levitateos.org".to_string());
    let bug_url = std::env::var("OS_BUG_REPORT_URL")
        .unwrap_or_else(|_| "https://github.com/levitateos/levitateos/issues".to_string());

    let hostname = std::env::var("OS_HOSTNAME").unwrap_or_else(|_| id.clone());
    fs::write(etc.join("hostname"), format!("{}\n", hostname))?;
    fs::write(etc.join("machine-id"), "")?;

    fs::write(
        etc.join("os-release"),
        format!(
            r#"NAME="{name}"
ID={id}
ID_LIKE={id_like}
VERSION="{version}"
VERSION_ID={version_id}
PRETTY_NAME="{name} {version}"
HOME_URL="{home_url}"
BUG_REPORT_URL="{bug_url}"
"#
        ),
    )?;

    Ok(())
}

fn create_filesystem_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(etc.join("fstab"), FSTAB)?;

    let mtab = etc.join("mtab");
    if !mtab.exists() && !mtab.is_symlink() {
        std::os::unix::fs::symlink("/proc/self/mounts", &mtab)?;
    }

    Ok(())
}

fn create_auth_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(etc.join("shells"), SHELLS)?;
    fs::write(etc.join("login.defs"), LOGIN_DEFS)?;
    fs::write(etc.join("sudoers"), SUDOERS)?;

    let mut perms = fs::metadata(etc.join("sudoers"))?.permissions();
    perms.set_mode(0o440);
    fs::set_permissions(etc.join("sudoers"), perms)?;

    fs::create_dir_all(etc.join("sudoers.d"))?;
    fs::write(etc.join("sudo.conf"), SUDO_CONF)?;

    Ok(())
}

fn create_locale_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    let localtime = etc.join("localtime");
    if !localtime.exists() && !localtime.is_symlink() {
        std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime)?;
    }

    fs::write(etc.join("adjtime"), ADJTIME)?;
    fs::write(etc.join("locale.conf"), LOCALE_CONF)?;
    fs::write(etc.join("vconsole.conf"), VCONSOLE_CONF)?;

    Ok(())
}

fn create_network_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(etc.join("hosts"), HOSTS)?;

    let resolv = etc.join("resolv.conf");
    if !resolv.exists() && !resolv.is_symlink() {
        std::os::unix::fs::symlink("/run/systemd/resolve/stub-resolv.conf", &resolv)?;
    }

    Ok(())
}

fn create_shell_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(etc.join("profile"), PROFILE)?;

    fs::create_dir_all(etc.join("profile.d"))?;
    fs::write(etc.join("profile.d/xdg.sh"), XDG_SH)?;
    fs::write(etc.join("bashrc"), BASHRC)?;

    let root_home = ctx.staging.join("root");
    fs::write(root_home.join(".bashrc"), ROOT_BASHRC)?;
    fs::write(root_home.join(".bash_profile"), ROOT_BASH_PROFILE)?;

    fs::create_dir_all(etc.join("skel"))?;
    fs::write(etc.join("skel/.bashrc"), SKEL_BASHRC)?;
    fs::write(etc.join("skel/.bash_profile"), SKEL_BASH_PROFILE)?;

    for xdg_dir in [".config", ".local/share", ".local/state", ".cache"] {
        let dir = etc.join("skel").join(xdg_dir);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(".keep"), "")?;
    }

    Ok(())
}

fn create_nsswitch(ctx: &BuildContext) -> Result<()> {
    fs::write(ctx.staging.join("etc/nsswitch.conf"), NSSWITCH)?;
    Ok(())
}

/// Copy timezone data from source to staging.
pub fn copy_timezone_data(ctx: &BuildContext) -> Result<()> {
    println!("Copying timezone data...");

    let src = ctx.source.join("usr/share/zoneinfo");
    let dst = ctx.staging.join("usr/share/zoneinfo");
    fs::create_dir_all(&dst)?;

    if src.exists() {
        copy_dir_recursive(&src, &dst)?;
        println!("  Copied all timezone data");
    }

    Ok(())
}

/// Copy locale archive from source to staging.
pub fn copy_locales(ctx: &BuildContext) -> Result<()> {
    println!("Copying locales...");

    let archive_src = ctx.source.join("usr/lib/locale/locale-archive");
    let archive_dst = ctx.staging.join("usr/lib/locale/locale-archive");

    if archive_src.exists() {
        fs::create_dir_all(archive_dst.parent().unwrap())?;
        fs::copy(&archive_src, &archive_dst)?;
        println!("  Copied locale-archive");
    }

    Ok(())
}

/// Copy dynamic linker configuration from source to staging.
pub fn copy_ld_so_conf(ctx: &BuildContext) -> Result<()> {
    // Copy ld.so.conf
    let src = ctx.source.join("etc/ld.so.conf");
    let dst = ctx.staging.join("etc/ld.so.conf");
    if src.exists() && !dst.exists() {
        fs::copy(&src, &dst)?;
    }

    // Copy ld.so.conf.d directory if it exists
    let src_dir = ctx.source.join("etc/ld.so.conf.d");
    let dst_dir = ctx.staging.join("etc/ld.so.conf.d");
    if src_dir.exists() {
        fs::create_dir_all(&dst_dir)?;
        copy_dir_recursive(&src_dir, &dst_dir)?;
    }

    Ok(())
}
