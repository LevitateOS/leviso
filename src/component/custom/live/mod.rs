//! Live ISO overlay operations.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use distro_spec::levitate::LIVE_ISSUE_MESSAGE;
use distro_spec::shared::LEVITATE_CARGO_TOOLS;

use crate::build::context::BuildContext;
use crate::common::read_manifest_file;

/// Read test instrumentation file - used by both live ISO and qcow2
pub fn read_test_instrumentation() -> Result<String> {
    read_manifest_file("live/overlay", "etc/profile.d/00-levitate-test.sh")
}

/// Create live overlay directory with autologin, serial console, empty root password.
///
/// This is called by iso.rs during ISO creation. The overlay is applied ONLY
/// during live boot, not extracted to installed systems.
pub fn create_live_overlay_at(output_dir: &Path, _base_dir: &Path) -> Result<()> {
    println!("Creating live overlay directory...");

    let overlay_dir = output_dir.join("live-overlay");
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    fs::create_dir_all(overlay_dir.join("etc"))?;

    // Autologin drop-ins (standard approach — same as Arch ISO, Fedora CoreOS, etc.)
    // Override getty ExecStart to add --autologin root
    let tty1_dropin = overlay_dir.join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&tty1_dropin)?;
    fs::write(
        tty1_dropin.join("autologin.conf"),
        read_manifest_file(
            "live/overlay",
            "etc/systemd/system/getty@tty1.service.d/autologin.conf",
        )?,
    )?;

    // serial-getty: use the TEMPLATE drop-in dir (serial-getty@.service.d) so it
    // merges with the existing local.conf from the EROFS rootfs via overlayfs.
    // Named zz- to sort after local.conf and override ExecStart.
    let serial_dropin = overlay_dir.join("etc/systemd/system/serial-getty@.service.d");
    fs::create_dir_all(&serial_dropin)?;
    fs::write(
        serial_dropin.join("zz-autologin.conf"),
        read_manifest_file(
            "live/overlay",
            "etc/systemd/system/serial-getty@.service.d/zz-autologin.conf",
        )?,
    )?;

    // Shadow file with empty root password
    fs::write(
        overlay_dir.join("etc/shadow"),
        read_manifest_file("live/overlay", "etc/shadow")?,
    )?;

    fs::set_permissions(
        overlay_dir.join("etc/shadow"),
        fs::Permissions::from_mode(0o600),
    )?;

    // Profile.d scripts
    let profile_d = overlay_dir.join("etc/profile.d");
    fs::create_dir_all(&profile_d)?;
    // Test mode instrumentation (00- prefix = runs first)
    fs::write(
        profile_d.join("00-levitate-test.sh"),
        read_manifest_file("live/overlay", "etc/profile.d/00-levitate-test.sh")?,
    )?;
    // Auto-launch tmux with docs-tui for interactive users
    fs::write(
        profile_d.join("live-docs.sh"),
        read_manifest_file("live/overlay", "etc/profile.d/live-docs.sh")?,
    )?;

    println!("  Created live overlay");
    Ok(())
}

/// Create live overlay (wrapper for BuildContext).
pub fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    create_live_overlay_at(&ctx.output, &ctx.base_dir)
}

/// Create welcome message (MOTD) for live environment.
pub fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    fs::write(
        ctx.staging.join("etc/motd"),
        read_manifest_file("live/overlay", "etc/motd")?,
    )?;
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

    let monorepo_dir = ctx
        .base_dir
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
    println!(
        "  Rebuilding installation tools ({})...",
        LEVITATE_CARGO_TOOLS.join(", ")
    );
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
            tool,
            tool
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
