//! OpenSSH setup - copies binaries and configs from extracted RPMs.
//!
//! SSH is available but NOT enabled by default (security - root has empty password).
//! Users can start it with: `systemctl start sshd`

use anyhow::{bail, Result};
use std::fs;

use super::context::BuildContext;
use super::libdeps::{copy_binary_with_libs, copy_dir_tree, copy_file, copy_sbin_binary_with_libs, copy_systemd_units};

/// SSH server binaries (from openssh-server RPM).
const SERVER_BINARIES: &[&str] = &["sshd"];

/// SSH client binaries (from openssh-clients RPM).
const CLIENT_BINARIES: &[&str] = &["ssh", "scp", "sftp", "ssh-keygen", "ssh-add", "ssh-agent"];

/// SSH systemd units.
const UNITS: &[&str] = &[
    "sshd.service",
    "sshd.socket",
    "sshd@.service",
    "sshd-keygen.target",
    "sshd-keygen@.service",
];

/// Set up OpenSSH (server + client).
pub fn setup_openssh(ctx: &BuildContext) -> Result<()> {
    println!("Setting up OpenSSH...");

    // Copy server binary
    for bin in SERVER_BINARIES {
        if !copy_sbin_binary_with_libs(ctx, bin)? {
            bail!("{} not found - openssh-server RPM not extracted?", bin);
        }
    }

    // Copy client binaries
    for bin in CLIENT_BINARIES {
        if !copy_binary_with_libs(ctx, bin, "usr/bin")? {
            bail!("{} not found - openssh-clients RPM not extracted?", bin);
        }
    }

    // Copy helper binaries from /usr/libexec/openssh/
    copy_dir_tree(ctx, "usr/libexec/openssh")?;

    // Copy configs from /etc/ssh/
    copy_dir_tree(ctx, "etc/ssh")?;

    // Copy PAM config
    if !copy_file(ctx, "etc/pam.d/sshd")? {
        bail!("etc/pam.d/sshd not found");
    }

    // Copy systemd units
    copy_systemd_units(ctx, UNITS)?;

    // Copy sysconfig (optional)
    let _ = copy_file(ctx, "etc/sysconfig/sshd");

    // Copy crypto-policies (Rocky/RHEL specific)
    copy_dir_tree(ctx, "etc/crypto-policies")?;
    copy_dir_tree(ctx, "usr/share/crypto-policies")?;

    // Create required directories
    fs::create_dir_all(ctx.staging.join("var/empty/sshd"))?;
    fs::create_dir_all(ctx.staging.join("run/sshd"))?;

    println!("  OpenSSH configured (not enabled by default)");
    Ok(())
}
