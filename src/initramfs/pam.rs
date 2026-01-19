//! PAM authentication setup.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::context::BuildContext;

/// PAM modules required for login.
const PAM_MODULES: &[&str] = &[
    "pam_permit.so",
    "pam_deny.so",
    "pam_unix.so",
    "pam_rootok.so",
    "pam_env.so",
    "pam_limits.so",
    "pam_nologin.so",
    "pam_securetty.so",
    "pam_shells.so",
    "pam_succeed_if.so",
];

/// Set up PAM for login/agetty.
pub fn setup_pam(ctx: &BuildContext) -> Result<()> {
    println!("Setting up PAM...");

    // Create PAM directories
    let pam_d = ctx.initramfs.join("etc/pam.d");
    let security_dir = ctx.initramfs.join("lib64/security");
    fs::create_dir_all(&pam_d)?;
    fs::create_dir_all(&security_dir)?;

    // Copy PAM modules
    copy_pam_modules(ctx, &security_dir)?;

    // Create PAM configs
    create_pam_configs(&pam_d)?;

    // Create auth files
    create_auth_files(&ctx.initramfs)?;

    println!("  Created PAM configuration");

    Ok(())
}

/// Copy PAM module .so files from rootfs.
fn copy_pam_modules(ctx: &BuildContext, security_dir: &Path) -> Result<()> {
    let pam_src = ctx.rootfs.join("usr/lib64/security");

    for module in PAM_MODULES {
        let src = pam_src.join(module);
        if src.exists() {
            let dst = security_dir.join(module);
            fs::copy(&src, &dst)
                .with_context(|| format!("Failed to copy PAM module: {}", module))?;
            println!("  Copied {}", module);
        }
    }

    Ok(())
}

/// Create PAM configuration files.
fn create_pam_configs(pam_d: &Path) -> Result<()> {
    // Minimal PAM config for login (permissive for live environment)
    fs::write(
        pam_d.join("login"),
        r#"#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_permit.so
account    required     pam_permit.so
password   required     pam_permit.so
session    required     pam_permit.so
"#,
    )?;

    // System-auth (referenced by other PAM configs)
    fs::write(
        pam_d.join("system-auth"),
        r#"#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_permit.so
account    required     pam_permit.so
password   required     pam_permit.so
session    required     pam_permit.so
"#,
    )?;

    Ok(())
}

/// Create authentication-related files.
fn create_auth_files(initramfs: &Path) -> Result<()> {
    // /etc/securetty (terminals where root can login)
    fs::write(
        initramfs.join("etc/securetty"),
        "tty1\ntty2\ntty3\ntty4\ntty5\ntty6\nttyS0\n",
    )?;

    // Empty /etc/shadow for root (no password = allow login)
    fs::write(initramfs.join("etc/shadow"), "root::0::::::\n")?;

    // /etc/shells (required for login)
    fs::write(initramfs.join("etc/shells"), "/bin/bash\n/bin/sh\n")?;

    Ok(())
}
