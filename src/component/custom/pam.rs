//! PAM and security configuration.
//!
//! This module creates PAM and security configuration files using constants from distro-spec.
//! This ensures leviso uses the same PAM configs as the rest of the system.
//!
//! All PAM configuration content comes from distro_spec::shared::auth::pam (SINGLE SOURCE OF TRUTH).
//! This prevents drift between what distro-spec declares and what leviso actually builds.

use anyhow::Result;
use std::fs;

use crate::build::context::BuildContext;

// Import PAM and security configuration contents from distro-spec (SINGLE SOURCE OF TRUTH)
// This ensures leviso builds exactly what distro-spec expects
use distro_spec::shared::auth::{
    ACCESS_CONF,
    // Security configuration files
    LIMITS_CONF,
    NAMESPACE_CONF,
    PAM_CHFN,
    PAM_CHPASSWD,
    PAM_CHSH,
    PAM_CROND,
    PAM_ENV_CONF,
    PAM_LOGIN,
    PAM_OTHER,
    PAM_PASSWD,
    PAM_POSTLOGIN,
    PAM_REMOTE,
    PAM_RUNUSER,
    PAM_RUNUSER_L,
    PAM_SSHD,
    PAM_SU,
    PAM_SUDO,
    PAM_SU_L,
    PAM_SYSTEMD_USER,
    PAM_SYSTEM_AUTH,
    PWQUALITY_CONF,
};

/// Create PAM configuration files.
pub fn create_pam_files(ctx: &BuildContext) -> Result<()> {
    println!("Setting up PAM configuration...");

    let pam_dir = ctx.staging.join("etc/pam.d");
    fs::create_dir_all(&pam_dir)?;

    // Core authentication stacks
    fs::write(pam_dir.join("system-auth"), PAM_SYSTEM_AUTH)?;
    fs::write(pam_dir.join("password-auth"), PAM_SYSTEM_AUTH)?;
    fs::write(pam_dir.join("postlogin"), PAM_POSTLOGIN)?;

    // Login services
    fs::write(pam_dir.join("login"), PAM_LOGIN)?;
    fs::write(pam_dir.join("remote"), PAM_REMOTE)?;
    fs::write(pam_dir.join("sshd"), PAM_SSHD)?;

    // Privilege escalation
    fs::write(pam_dir.join("runuser"), PAM_RUNUSER)?;
    fs::write(pam_dir.join("runuser-l"), PAM_RUNUSER_L)?;
    fs::write(pam_dir.join("su"), PAM_SU)?;
    fs::write(pam_dir.join("su-l"), PAM_SU_L)?;
    fs::write(pam_dir.join("sudo"), PAM_SUDO)?;

    // System services
    fs::write(pam_dir.join("crond"), PAM_CROND)?;
    fs::write(pam_dir.join("systemd-user"), PAM_SYSTEMD_USER)?;

    // Password management
    fs::write(pam_dir.join("passwd"), PAM_PASSWD)?;
    fs::write(pam_dir.join("chpasswd"), PAM_CHPASSWD)?;
    fs::write(pam_dir.join("chfn"), PAM_CHFN)?;
    fs::write(pam_dir.join("chsh"), PAM_CHSH)?;

    // Fallback for unconfigured services
    fs::write(pam_dir.join("other"), PAM_OTHER)?;

    println!("  Created PAM configuration files");
    Ok(())
}

/// Create security configuration files.
///
/// Uses configuration constants from distro-spec::shared::auth::pam to ensure
/// consistency with the rest of the system. This prevents drift between what
/// distro-spec declares and what leviso builds.
pub fn create_security_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating security configuration...");

    let security_dir = ctx.staging.join("etc/security");
    fs::create_dir_all(&security_dir)?;

    // Write all security configuration files using constants from distro-spec
    fs::write(security_dir.join("limits.conf"), LIMITS_CONF)?;
    fs::write(security_dir.join("access.conf"), ACCESS_CONF)?;
    fs::write(security_dir.join("namespace.conf"), NAMESPACE_CONF)?;
    fs::write(security_dir.join("pam_env.conf"), PAM_ENV_CONF)?;
    fs::write(security_dir.join("pwquality.conf"), PWQUALITY_CONF)?;

    println!("  Created security configuration");
    Ok(())
}

/// Disable SELinux (LevitateOS doesn't ship SELinux policies).
pub fn disable_selinux(ctx: &BuildContext) -> Result<()> {
    let selinux_dir = ctx.staging.join("etc/selinux");
    fs::create_dir_all(&selinux_dir)?;

    fs::write(
        selinux_dir.join("config"),
        "# SELinux disabled - LevitateOS doesn't ship SELinux policies\n\
         SELINUX=disabled\n\
         SELINUXTYPE=targeted\n",
    )?;

    println!("  Disabled SELinux");
    Ok(())
}
