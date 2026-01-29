//! Live ISO overlay operations.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use distro_spec::levitate::LIVE_ISSUE_MESSAGE;
use distro_spec::shared::LEVITATE_CARGO_TOOLS;

use crate::build::context::BuildContext;

/// Read a file from the colocated overlay directory (no relative path traversal)
fn read_profile_file_from_base(_base_dir: &Path, path: &str) -> Result<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest_dir is already at leviso/ root, so join directly to overlay directory
    let file_path = manifest_dir
        .join("src/component/custom/live/overlay")
        .join(path);
    fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read live overlay file from {}", file_path.display()))
}

/// Read a file from the colocated overlay directory
fn read_profile_file(_ctx: &BuildContext, path: &str) -> Result<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest_dir is already at leviso/ root, so join directly to overlay directory
    let file_path = manifest_dir
        .join("src/component/custom/live/overlay")
        .join(path);
    fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read live overlay file from {}", file_path.display()))
}

/// Read test instrumentation file - used by both live ISO and qcow2
pub fn read_test_instrumentation() -> Result<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest_dir is already at leviso/ root, so join directly to overlay directory
    let file_path = manifest_dir
        .join("src/component/custom/live/overlay/etc/profile.d/00-levitate-test.sh");
    fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read test instrumentation file from {}", file_path.display()))
}

/// Create live overlay directory with autologin, serial console, empty root password.
///
/// This is called by iso.rs during ISO creation. The overlay is applied ONLY
/// during live boot, not extracted to installed systems.
pub fn create_live_overlay_at(output_dir: &Path, base_dir: &Path) -> Result<()> {
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
        read_profile_file_from_base(base_dir, "etc/systemd/system/console-autologin.service")?,
    )?;

    std::os::unix::fs::symlink(
        "../console-autologin.service",
        getty_wants.join("console-autologin.service"),
    )?;

    // Serial console service
    fs::write(
        systemd_dir.join("serial-console.service"),
        read_profile_file_from_base(base_dir, "etc/systemd/system/serial-console.service")?,
    )?;

    std::os::unix::fs::symlink(
        "../serial-console.service",
        multi_user_wants.join("serial-console.service"),
    )?;

    // Shadow file with empty root password
    fs::write(overlay_dir.join("etc/shadow"), read_profile_file_from_base(base_dir, "etc/shadow")?)?;

    fs::set_permissions(
        overlay_dir.join("etc/shadow"),
        fs::Permissions::from_mode(0o600),
    )?;

    // Profile.d scripts
    let profile_d = overlay_dir.join("etc/profile.d");
    fs::create_dir_all(&profile_d)?;
    // Test mode instrumentation (00- prefix = runs first)
    fs::write(profile_d.join("00-levitate-test.sh"), read_profile_file_from_base(base_dir, "etc/profile.d/00-levitate-test.sh")?)?;
    // Auto-launch tmux with docs-tui for interactive users
    fs::write(profile_d.join("live-docs.sh"), read_profile_file_from_base(base_dir, "etc/profile.d/live-docs.sh")?)?;

    // Autologin wrapper script
    let usr_local_bin = overlay_dir.join("usr/local/bin");
    fs::create_dir_all(&usr_local_bin)?;
    let autologin_path = usr_local_bin.join("autologin-shell");
    fs::write(&autologin_path, read_profile_file_from_base(base_dir, "usr/local/bin/autologin-shell")?)?;
    fs::set_permissions(&autologin_path, fs::Permissions::from_mode(0o755))?;

    println!("  Created live overlay");
    Ok(())
}

/// Create live overlay (wrapper for BuildContext).
pub fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    create_live_overlay_at(&ctx.output, &ctx.base_dir)
}

/// Create welcome message (MOTD) for live environment.
pub fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    fs::write(ctx.staging.join("etc/motd"), read_profile_file(ctx, "etc/motd")?)?;
    fs::write(ctx.staging.join("etc/issue"), LIVE_ISSUE_MESSAGE)?;
    Ok(())
}

/// Install installation tools (recstrap, recfstab, recchroot) to staging.
///
/// AUTOMATICALLY REBUILDS tools before copying to ensure latest versions.
/// This prevents stale binaries from being included in the ISO.
pub fn install_tools(ctx: &BuildContext) -> Result<()> {
    use anyhow::{bail, Context};
    use leviso_elf::make_executable;
    use std::process::Command;

    let monorepo_dir = ctx.base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| ctx.base_dir.to_path_buf());

    let bin_dir = ctx.staging.join("usr/bin");
    fs::create_dir_all(&bin_dir)?;

    // Build args: cargo build --release -p recstrap -p recfstab -p recchroot
    let mut build_args: Vec<&str> = vec!["build", "--release"];
    for tool in LEVITATE_CARGO_TOOLS {
        build_args.push("-p");
        build_args.push(tool);
    }

    // ALWAYS rebuild tools to ensure latest version
    println!("  Rebuilding installation tools ({})...", LEVITATE_CARGO_TOOLS.join(", "));
    let status = Command::new("cargo")
        .args(&build_args)
        .current_dir(&monorepo_dir)
        .status()
        .context("Failed to run cargo build for installation tools")?;

    if !status.success() {
        bail!(
            "Failed to build installation tools. Check cargo output above.\n\
             \n\
             These tools are REQUIRED for the live ISO."
        );
    }
    println!("  Rebuilt installation tools successfully");

    for tool in LEVITATE_CARGO_TOOLS {
        let dest = bin_dir.join(tool);

        // Check workspace target (most common for workspace members)
        let workspace_binary = monorepo_dir.join("target/release").join(tool);
        if workspace_binary.exists() {
            fs::copy(&workspace_binary, &dest)
                .with_context(|| format!("Failed to copy {} to staging", tool))?;
            make_executable(&dest)?;
            println!("  Installed {} from workspace", tool);
            continue;
        }

        // Fallback: crate-local target
        let local_binary = monorepo_dir.join(format!("tools/{}/target/release/{}", tool, tool));
        if local_binary.exists() {
            fs::copy(&local_binary, &dest)
                .with_context(|| format!("Failed to copy {} to staging", tool))?;
            make_executable(&dest)?;
            println!("  Installed {} from local target", tool);
            continue;
        }

        bail!(
            "{} binary not found after rebuild. This is a bug.\n\
             Check that tools/{}/Cargo.toml exists and compiles.",
            tool, tool
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
