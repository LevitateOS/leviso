//! PAM configuration for installed system.

use anyhow::Result;
use std::fs;

use super::context::BuildContext;
use super::libdeps::copy_dir_tree;

/// Set up PAM configuration for installed system.
pub fn setup_pam(ctx: &BuildContext) -> Result<()> {
    println!("Setting up PAM configuration...");

    let pam_dir = ctx.staging.join("etc/pam.d");
    fs::create_dir_all(&pam_dir)?;

    // system-auth
    fs::write(pam_dir.join("system-auth"), PAM_SYSTEM_AUTH)?;

    // password-auth (same as system-auth for simple setup)
    fs::write(pam_dir.join("password-auth"), PAM_SYSTEM_AUTH)?;

    // login
    fs::write(pam_dir.join("login"), PAM_LOGIN)?;

    // passwd, su, sudo, chpasswd, other, systemd-user
    fs::write(pam_dir.join("passwd"), "auth include system-auth\naccount include system-auth\npassword substack system-auth\n")?;
    fs::write(pam_dir.join("su"), "auth sufficient pam_rootok.so\nauth required pam_unix.so\naccount sufficient pam_rootok.so\naccount required pam_unix.so\nsession required pam_unix.so\n")?;
    fs::write(pam_dir.join("sudo"), "auth include system-auth\naccount include system-auth\npassword include system-auth\nsession optional pam_keyinit.so revoke\nsession required pam_limits.so\n")?;
    fs::write(pam_dir.join("chpasswd"), "auth sufficient pam_rootok.so\nauth required pam_unix.so\naccount required pam_unix.so\npassword include system-auth\n")?;
    fs::write(pam_dir.join("other"), "auth required pam_deny.so\naccount required pam_deny.so\npassword required pam_deny.so\nsession required pam_deny.so\n")?;
    fs::write(pam_dir.join("systemd-user"), "account include system-auth\nsession required pam_loginuid.so\nsession optional pam_keyinit.so force revoke\nsession include system-auth\n")?;

    println!("  Created PAM configuration files");
    Ok(())
}

const PAM_SYSTEM_AUTH: &str = "\
auth required pam_env.so
auth sufficient pam_unix.so try_first_pass nullok
auth required pam_deny.so
account required pam_unix.so
password requisite pam_pwquality.so try_first_pass local_users_only retry=3
password sufficient pam_unix.so try_first_pass use_authtok nullok sha512 shadow
password required pam_deny.so
session optional pam_keyinit.so revoke
session required pam_limits.so
session required pam_unix.so
";

const PAM_LOGIN: &str = "\
auth requisite pam_nologin.so
auth include system-auth
account required pam_access.so
account include system-auth
password include system-auth
session required pam_loginuid.so
session optional pam_keyinit.so force revoke
session include system-auth
session required pam_namespace.so
session optional pam_lastlog.so showfailed
session optional pam_motd.so
";

/// Copy PAM modules from source rootfs.
pub fn copy_pam_modules(ctx: &BuildContext) -> Result<()> {
    println!("Copying PAM modules...");
    let count = copy_dir_tree(ctx, "usr/lib64/security")?;
    println!("  Copied {} PAM modules", count);
    Ok(())
}

/// Create PAM security configuration files.
pub fn create_security_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating security configuration...");

    let security_dir = ctx.staging.join("etc/security");
    fs::create_dir_all(&security_dir)?;

    fs::write(security_dir.join("limits.conf"), "\
*               soft    core            0
*               hard    nofile          1048576
*               soft    nofile          1024
root            soft    nofile          1048576
")?;

    fs::write(security_dir.join("access.conf"), "+:root:LOCAL\n+:ALL:ALL\n")?;
    fs::write(security_dir.join("namespace.conf"), "# Polyinstantiation config\n")?;
    fs::write(security_dir.join("pam_env.conf"), "# Environment variables\n")?;
    fs::write(security_dir.join("pwquality.conf"), "minlen = 8\nminclass = 1\n")?;

    println!("  Created security configuration");
    Ok(())
}
