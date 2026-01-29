//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module here handles a specific domain of custom operations.
//!
//! NOTE: This is split from the original 1,283-line custom.rs for maintainability.
//! Each module is ~100-250 lines focused on a single domain.

mod etc;
mod filesystem;
mod firmware;
mod live;
mod modules;
mod packages;
mod pam;

use anyhow::Result;

use super::CustomOp;
use crate::build::context::BuildContext;
use crate::build::licenses::LicenseTracker;

// Re-export public API
pub use live::create_live_overlay_at;

/// Execute a custom operation.
///
/// Some operations copy content that requires license tracking. The tracker
/// is used to register packages for license compliance.
pub fn execute(ctx: &BuildContext, op: CustomOp, tracker: &LicenseTracker) -> Result<()> {
    match op {
        // Filesystem operations (no content copying)
        CustomOp::CreateFhsSymlinks => filesystem::create_fhs_symlinks(ctx),

        // Live overlay
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::InstallTools => live::install_tools(ctx),
        CustomOp::SetupLiveSystemdConfigs => live::setup_live_systemd_configs(ctx),

        // Firmware - register linux-firmware package
        CustomOp::CopyWifiFirmware => {
            tracker.register_package("linux-firmware");
            firmware::copy_wifi_firmware(ctx)
        }
        CustomOp::CopyAllFirmware => {
            tracker.register_package("linux-firmware");
            tracker.register_package("microcode_ctl");
            firmware::copy_all_firmware(ctx)
        }

        // Kernel modules - register kernel package
        CustomOp::RunDepmod => modules::run_depmod(ctx),
        CustomOp::CopyModules => {
            tracker.register_package("kernel");
            modules::copy_modules(ctx)
        }

        // /etc configuration
        CustomOp::CreateEtcFiles => etc::create_etc_files(ctx),
        CustomOp::CopyTimezoneData => {
            tracker.register_package("tzdata");
            etc::copy_timezone_data(ctx)
        }
        CustomOp::CopyLocales => {
            // Locale archive is from glibc, already tracked via binaries
            etc::copy_locales(ctx)
        }
        CustomOp::CreateSshHostKeys => etc::create_ssh_host_keys(ctx),

        // PAM and security (config files only, no content copying)
        CustomOp::CreatePamFiles => pam::create_pam_files(ctx),
        CustomOp::CreateSecurityConfig => pam::create_security_config(ctx),
        CustomOp::DisableSelinux => pam::disable_selinux(ctx),

        // Package manager and bootloader
        CustomOp::CopySystemdBootEfi => {
            // systemd-boot is part of systemd, already tracked
            packages::copy_systemd_boot_efi(ctx)
        }
        CustomOp::CopyKeymaps => {
            tracker.register_package("kbd");
            packages::copy_keymaps(ctx)
        }
        CustomOp::CopyRecipe => packages::copy_recipe(ctx),
        CustomOp::SetupRecipeConfig => packages::setup_recipe_config(ctx),
        CustomOp::CopyDocsTui => install_docs_tui(ctx),
    }
}

/// Install docs-tui (levitate-docs) to staging.
///
/// AUTOMATICALLY REBUILDS the docs-TUI before copying to ensure latest version.
/// Copies the binary AND its library dependencies to staging.
/// The levitate-docs binary is compiled with Bun and links against glibc,
/// so we must ensure libpthread.so.0, libdl.so.2, libm.so.6 etc. are present.
fn install_docs_tui(ctx: &BuildContext) -> Result<()> {
    use anyhow::{bail, Context};
    use leviso_elf::{get_all_dependencies, make_executable};
    use std::fs;
    use std::process::Command;
    use crate::build::libdeps::copy_library;

    let monorepo_dir = ctx.base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| ctx.base_dir.to_path_buf());

    let bin_dir = ctx.staging.join("usr/bin");
    fs::create_dir_all(&bin_dir)?;

    let dest = bin_dir.join("levitate-docs");

    let docs_tui_dir = monorepo_dir.join("docs/tui");
    let docs_tui_binary = docs_tui_dir.join("levitate-docs");

    // ALWAYS rebuild docs-TUI to ensure latest version
    println!("  Rebuilding levitate-docs...");
    let status = Command::new("bun")
        .args(["build", "--compile", "--minify", "--outfile", "levitate-docs", "src/index.ts"])
        .current_dir(&docs_tui_dir)
        .status()
        .context("Failed to run bun build for levitate-docs")?;

    if !status.success() {
        bail!(
            "Failed to build levitate-docs. Check bun output above.\n\
             \n\
             Make sure bun is installed: curl -fsSL https://bun.sh/install | bash"
        );
    }

    if !docs_tui_binary.exists() {
        bail!(
            "levitate-docs binary not found after rebuild at {}.\n\
             This is a bug - bun build succeeded but binary not created.",
            docs_tui_binary.display()
        );
    }

    // Copy the binary
    fs::copy(&docs_tui_binary, &dest)
        .with_context(|| "Failed to copy levitate-docs to staging")?;
    make_executable(&dest)?;
    let size_mb = fs::metadata(&dest)?.len() as f64 / 1_000_000.0;
    println!("  Installed levitate-docs ({:.1} MB)", size_mb);

    // CRITICAL: Copy library dependencies from Rocky rootfs
    // The levitate-docs binary was compiled with Bun and needs glibc compat libs:
    // - libpthread.so.0 (glibc 2.34+ compat stub)
    // - libdl.so.2 (glibc 2.34+ compat stub)
    // - libm.so.6 (math library)
    // Without these, levitate-docs crashes with "cannot open shared object file"
    let extra_lib_paths: &[&str] = &[];
    let libs = get_all_dependencies(&ctx.source, &docs_tui_binary, extra_lib_paths)
        .with_context(|| "Failed to get levitate-docs library dependencies")?;

    println!("  Copying {} library dependencies for levitate-docs...", libs.len());
    for lib_name in &libs {
        copy_library(ctx, lib_name, None)
            .with_context(|| format!("levitate-docs requires missing library '{}'", lib_name))?;
    }

    Ok(())
}
