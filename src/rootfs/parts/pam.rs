//! PAM configuration for installed system.
//!
//! Real PAM authentication (not permissive like live environment).
//! Uses pam_unix for local password authentication.

use anyhow::Result;
use std::fs;

use crate::rootfs::context::BuildContext;

/// Set up PAM configuration for installed system.
pub fn setup_pam(ctx: &BuildContext) -> Result<()> {
    println!("Setting up PAM configuration...");

    let pam_dir = ctx.staging.join("etc/pam.d");
    fs::create_dir_all(&pam_dir)?;

    // /etc/pam.d/system-auth - base authentication stack
    fs::write(
        pam_dir.join("system-auth"),
        r#"#%PAM-1.0
# System authentication configuration

auth        required      pam_env.so
auth        sufficient    pam_unix.so try_first_pass nullok
auth        required      pam_deny.so

account     required      pam_unix.so

password    requisite     pam_pwquality.so try_first_pass local_users_only retry=3 authtok_type=
password    sufficient    pam_unix.so try_first_pass use_authtok nullok sha512 shadow
password    required      pam_deny.so

session     optional      pam_keyinit.so revoke
session     required      pam_limits.so
session     required      pam_unix.so
"#,
    )?;

    // /etc/pam.d/password-auth - password authentication
    fs::write(
        pam_dir.join("password-auth"),
        r#"#%PAM-1.0
# Password authentication configuration

auth        required      pam_env.so
auth        sufficient    pam_unix.so try_first_pass nullok
auth        required      pam_deny.so

account     required      pam_unix.so

password    requisite     pam_pwquality.so try_first_pass local_users_only retry=3 authtok_type=
password    sufficient    pam_unix.so try_first_pass use_authtok nullok sha512 shadow
password    required      pam_deny.so

session     optional      pam_keyinit.so revoke
session     required      pam_limits.so
session     required      pam_unix.so
"#,
    )?;

    // /etc/pam.d/login - console login
    fs::write(
        pam_dir.join("login"),
        r#"#%PAM-1.0
# Login authentication configuration

auth       requisite    pam_nologin.so
auth       include      system-auth

account    required     pam_access.so
account    include      system-auth

password   include      system-auth

session    required     pam_loginuid.so
session    optional     pam_keyinit.so force revoke
session    include      system-auth
session    required     pam_namespace.so
session    optional     pam_lastlog.so showfailed
session    optional     pam_motd.so
"#,
    )?;

    // /etc/pam.d/passwd - password change
    fs::write(
        pam_dir.join("passwd"),
        r#"#%PAM-1.0
# Password change configuration

auth       include      system-auth
account    include      system-auth
password   substack     system-auth
"#,
    )?;

    // /etc/pam.d/su - su command
    fs::write(
        pam_dir.join("su"),
        r#"#%PAM-1.0
# su authentication configuration

auth       sufficient   pam_rootok.so
auth       required     pam_unix.so

account    sufficient   pam_rootok.so
account    required     pam_unix.so

session    required     pam_unix.so
"#,
    )?;

    // /etc/pam.d/sudo - sudo command
    fs::write(
        pam_dir.join("sudo"),
        r#"#%PAM-1.0
# sudo authentication configuration

auth       include      system-auth
account    include      system-auth
password   include      system-auth
session    optional     pam_keyinit.so revoke
session    required     pam_limits.so
"#,
    )?;

    // /etc/pam.d/chpasswd - batch password change
    fs::write(
        pam_dir.join("chpasswd"),
        r#"#%PAM-1.0
# chpasswd configuration

auth       sufficient   pam_rootok.so
auth       required     pam_unix.so

account    required     pam_unix.so

password   include      system-auth
"#,
    )?;

    // /etc/pam.d/other - fallback for unconfigured services
    fs::write(
        pam_dir.join("other"),
        r#"#%PAM-1.0
# Fallback PAM configuration

auth        required      pam_deny.so
account     required      pam_deny.so
password    required      pam_deny.so
session     required      pam_deny.so
"#,
    )?;

    // /etc/pam.d/systemd-user - systemd user sessions
    fs::write(
        pam_dir.join("systemd-user"),
        r#"#%PAM-1.0
# systemd user session configuration

account    include      system-auth
session    required     pam_loginuid.so
session    optional     pam_keyinit.so force revoke
session    include      system-auth
"#,
    )?;

    println!("  Created PAM configuration files");
    Ok(())
}

/// Copy PAM modules from source rootfs.
pub fn copy_pam_modules(ctx: &BuildContext) -> Result<()> {
    println!("Copying PAM modules...");

    let modules_src = ctx.source.join("usr/lib64/security");
    let modules_dst = ctx.staging.join("usr/lib64/security");

    if modules_src.exists() {
        fs::create_dir_all(&modules_dst)?;

        // Copy essential PAM modules
        let essential_modules = [
            "pam_unix.so",
            "pam_deny.so",
            "pam_permit.so",
            "pam_env.so",
            "pam_nologin.so",
            "pam_securetty.so",
            "pam_limits.so",
            "pam_access.so",
            "pam_namespace.so",
            "pam_lastlog.so",
            "pam_motd.so",
            "pam_keyinit.so",
            "pam_loginuid.so",
            "pam_rootok.so",
            "pam_pwquality.so",
            "pam_faillock.so",
            "pam_shells.so",
            "pam_succeed_if.so",
            "pam_systemd.so",
            "pam_systemd_home.so",
        ];

        for module in essential_modules {
            let src = modules_src.join(module);
            let dst = modules_dst.join(module);
            if src.exists() {
                fs::copy(&src, &dst)?;
            }
        }

        println!("  Copied PAM modules");
    }

    Ok(())
}

/// Create PAM security configuration files.
pub fn create_security_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating security configuration...");

    let security_dir = ctx.staging.join("etc/security");
    fs::create_dir_all(&security_dir)?;

    // /etc/security/limits.conf
    fs::write(
        security_dir.join("limits.conf"),
        r#"# /etc/security/limits.conf
#
# <domain>  <type>  <item>  <value>
#

# Default limits
*               soft    core            0
*               hard    nofile          1048576
*               soft    nofile          1024
root            soft    nofile          1048576
"#,
    )?;

    // /etc/security/access.conf
    fs::write(
        security_dir.join("access.conf"),
        r#"# /etc/security/access.conf
#
# Login access control table
#

# Allow root from console
+:root:LOCAL

# Allow all other users from anywhere (default)
+:ALL:ALL
"#,
    )?;

    // /etc/security/namespace.conf
    fs::write(
        security_dir.join("namespace.conf"),
        r#"# /etc/security/namespace.conf
#
# Polyinstantiation configuration
#

# $HOME    $HOME                        user      root
# /tmp     /tmp-inst/                   level     root
# /var/tmp /var/tmp/tmp-inst/           level     root
"#,
    )?;

    // /etc/security/pam_env.conf
    fs::write(
        security_dir.join("pam_env.conf"),
        r#"# /etc/security/pam_env.conf
#
# Environment variables for PAM sessions
#

# PATH is set in /etc/profile
"#,
    )?;

    // /etc/security/pwquality.conf
    fs::write(
        security_dir.join("pwquality.conf"),
        r#"# Password quality configuration
#
# Minimal requirements for passwords

# Minimum password length
minlen = 8

# Minimum number of character classes (uppercase, lowercase, digits, special)
minclass = 1
"#,
    )?;

    println!("  Created security configuration");
    Ok(())
}
