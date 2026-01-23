//! PAM and security configuration.

use anyhow::Result;
use std::fs;

use crate::build::context::BuildContext;

// PAM configs from profile/etc/pam.d/
const PAM_SYSTEM_AUTH: &str = include_str!("../../../profile/etc/pam.d/system-auth");
const PAM_LOGIN: &str = include_str!("../../../profile/etc/pam.d/login");
const PAM_POSTLOGIN: &str = include_str!("../../../profile/etc/pam.d/postlogin");
const PAM_SSHD: &str = include_str!("../../../profile/etc/pam.d/sshd");
const PAM_RUNUSER: &str = include_str!("../../../profile/etc/pam.d/runuser");
const PAM_RUNUSER_L: &str = include_str!("../../../profile/etc/pam.d/runuser-l");
const PAM_CROND: &str = include_str!("../../../profile/etc/pam.d/crond");
const PAM_REMOTE: &str = include_str!("../../../profile/etc/pam.d/remote");
const LIMITS_CONF: &str = include_str!("../../../profile/etc/security/limits.conf");

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
    fs::write(pam_dir.join("su"), "auth sufficient pam_rootok.so\nauth include system-auth\naccount sufficient pam_rootok.so\naccount include system-auth\npassword include system-auth\nsession include system-auth\n")?;
    fs::write(pam_dir.join("su-l"), "auth include su\naccount include su\npassword include su\nsession optional pam_keyinit.so force revoke\nsession include su\n")?;
    fs::write(pam_dir.join("sudo"), "auth include system-auth\naccount include system-auth\npassword include system-auth\nsession optional pam_keyinit.so revoke\nsession required pam_limits.so\n-session optional pam_systemd.so\n")?;

    // System services
    fs::write(pam_dir.join("crond"), PAM_CROND)?;
    fs::write(pam_dir.join("systemd-user"), "account include system-auth\nsession required pam_loginuid.so\nsession optional pam_keyinit.so force revoke\n-session optional pam_systemd.so\nsession include system-auth\n")?;

    // Password management
    fs::write(pam_dir.join("passwd"), "auth include system-auth\naccount include system-auth\npassword substack system-auth\n")?;
    fs::write(pam_dir.join("chpasswd"), "auth sufficient pam_rootok.so\nauth include system-auth\naccount include system-auth\npassword include system-auth\n")?;
    fs::write(pam_dir.join("chfn"), "auth sufficient pam_rootok.so\nauth include system-auth\naccount include system-auth\npassword include system-auth\nsession include system-auth\n")?;
    fs::write(pam_dir.join("chsh"), "auth sufficient pam_rootok.so\nauth include system-auth\naccount include system-auth\npassword include system-auth\nsession include system-auth\n")?;

    // Fallback for unconfigured services
    fs::write(pam_dir.join("other"), "auth required pam_deny.so\naccount required pam_deny.so\npassword required pam_deny.so\nsession required pam_deny.so\n")?;

    println!("  Created PAM configuration files");
    Ok(())
}

/// Create security configuration files.
pub fn create_security_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating security configuration...");

    let security_dir = ctx.staging.join("etc/security");
    fs::create_dir_all(&security_dir)?;

    fs::write(security_dir.join("limits.conf"), LIMITS_CONF)?;
    fs::write(security_dir.join("access.conf"), "+:root:LOCAL\n+:ALL:ALL\n")?;
    fs::write(security_dir.join("namespace.conf"), "# Polyinstantiation config\n")?;
    fs::write(security_dir.join("pam_env.conf"), "# Environment variables\n")?;
    fs::write(security_dir.join("pwquality.conf"), "minlen = 8\nminclass = 1\n")?;

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
