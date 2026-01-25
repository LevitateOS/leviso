//! Live ISO overlay operations.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use distro_spec::levitate::LIVE_ISSUE_MESSAGE;

use crate::build::context::BuildContext;

const MOTD: &str = include_str!("../../../profile/etc/motd");

// Live overlay files (applied only during live boot, not to installed systems)
const LIVE_CONSOLE_AUTOLOGIN: &str =
    include_str!("../../../profile/live-overlay/etc/systemd/system/console-autologin.service");
const LIVE_SERIAL_CONSOLE: &str =
    include_str!("../../../profile/live-overlay/etc/systemd/system/serial-console.service");
// SECURITY: Empty root password is INTENTIONAL for archiso-like live boot behavior.
// This allows passwordless login on the live ISO only. Installed systems use locked
// root (root:!:...) via recstrap, which copies the base /etc/shadow, not this overlay.
const LIVE_SHADOW: &str = include_str!("../../../profile/live-overlay/etc/shadow");
const LIVE_DOCS_SH: &str = include_str!("../../../profile/live-overlay/etc/profile.d/live-docs.sh");
// Test mode instrumentation (00- prefix ensures it runs before live-docs.sh)
const LIVE_TEST_MODE: &str =
    include_str!("../../../profile/live-overlay/etc/profile.d/00-levitate-test.sh");

/// Create live overlay directory with autologin, serial console, empty root password.
///
/// This is called by iso.rs during ISO creation. The overlay is applied ONLY
/// during live boot, not extracted to installed systems.
pub fn create_live_overlay_at(output_dir: &Path) -> Result<()> {
    println!("Creating live overlay directory...");

    let overlay_dir = output_dir.join("live-overlay");
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    let systemd_dir = overlay_dir.join("etc/systemd/system");
    let getty_wants = systemd_dir.join("getty.target.wants");
    let multi_user_wants = systemd_dir.join("multi-user.target.wants");

    fs::create_dir_all(&getty_wants)?;
    fs::create_dir_all(&multi_user_wants)?;
    fs::create_dir_all(overlay_dir.join("etc"))?;

    // Console autologin service (Conflicts=getty@tty1.service ensures no conflict)
    fs::write(
        systemd_dir.join("console-autologin.service"),
        LIVE_CONSOLE_AUTOLOGIN,
    )?;

    std::os::unix::fs::symlink(
        "../console-autologin.service",
        getty_wants.join("console-autologin.service"),
    )?;

    // Serial console service
    fs::write(
        systemd_dir.join("serial-console.service"),
        LIVE_SERIAL_CONSOLE,
    )?;

    std::os::unix::fs::symlink(
        "../serial-console.service",
        multi_user_wants.join("serial-console.service"),
    )?;

    // Shadow file with empty root password
    fs::write(overlay_dir.join("etc/shadow"), LIVE_SHADOW)?;

    fs::set_permissions(
        overlay_dir.join("etc/shadow"),
        fs::Permissions::from_mode(0o600),
    )?;

    // Profile.d scripts
    let profile_d = overlay_dir.join("etc/profile.d");
    fs::create_dir_all(&profile_d)?;
    // Test mode instrumentation (00- prefix = runs first)
    fs::write(profile_d.join("00-levitate-test.sh"), LIVE_TEST_MODE)?;
    // Auto-launch tmux with docs-tui for interactive users
    fs::write(profile_d.join("live-docs.sh"), LIVE_DOCS_SH)?;

    println!("  Created live overlay");
    Ok(())
}

/// Create live overlay (wrapper for BuildContext).
pub fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    create_live_overlay_at(&ctx.output)
}

/// Create welcome message (MOTD) for live environment.
pub fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    fs::write(ctx.staging.join("etc/motd"), MOTD)?;
    fs::write(ctx.staging.join("etc/issue"), LIVE_ISSUE_MESSAGE)?;
    Ok(())
}

/// Copy installation tools (recstrap, recfstab, recchroot) to staging.
pub fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    use leviso_deps::DependencyResolver;

    let resolver = DependencyResolver::new(&ctx.base_dir)?;

    let (recstrap, recfstab, recchroot) = resolver
        .all_tools()
        .context("Installation tools are REQUIRED - the ISO cannot install itself without them")?;

    for tool in [&recstrap, &recfstab, &recchroot] {
        let dst = ctx.staging.join("usr/bin").join(tool.tool.name());
        fs::copy(&tool.path, &dst)?;
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        println!(
            "  Copied {} to /usr/bin/{} (from {:?})",
            tool.tool.name(),
            tool.tool.name(),
            tool.source
        );
    }

    Ok(())
}

/// Set up live systemd configurations (volatile journal, no suspend).
pub fn setup_live_systemd_configs(ctx: &BuildContext) -> Result<()> {
    println!("Setting up live systemd configs...");

    let journald_dir = ctx.staging.join("etc/systemd/journald.conf.d");
    fs::create_dir_all(&journald_dir)?;
    fs::write(
        journald_dir.join("volatile.conf"),
        "[Journal]\nStorage=volatile\nRuntimeMaxUse=64M\n",
    )?;

    let logind_dir = ctx.staging.join("etc/systemd/logind.conf.d");
    fs::create_dir_all(&logind_dir)?;
    fs::write(
        logind_dir.join("do-not-suspend.conf"),
        "[Login]\nHandleSuspendKey=ignore\nHandleHibernateKey=ignore\n\
         HandleLidSwitch=ignore\nHandleLidSwitchExternalPower=ignore\nIdleAction=ignore\n",
    )?;

    println!("  Created live systemd configs");
    Ok(())
}
