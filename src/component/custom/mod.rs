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
/// Copies directly to ctx.staging instead of hardcoded path.
fn install_docs_tui(ctx: &BuildContext) -> Result<()> {
    use anyhow::{bail, Context};
    use leviso_elf::make_executable;
    use std::fs;

    let monorepo_dir = ctx.base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| ctx.base_dir.to_path_buf());

    let bin_dir = ctx.staging.join("usr/bin");
    fs::create_dir_all(&bin_dir)?;

    let dest = bin_dir.join("levitate-docs");

    // Check for built binary in docs/tui
    let docs_tui_binary = monorepo_dir.join("docs/tui/levitate-docs");
    if docs_tui_binary.exists() {
        fs::copy(&docs_tui_binary, &dest)
            .with_context(|| "Failed to copy levitate-docs to staging")?;
        make_executable(&dest)?;
        let size_mb = fs::metadata(&dest)?.len() as f64 / 1_000_000.0;
        println!("  Installed levitate-docs ({:.1} MB)", size_mb);
        return Ok(());
    }

    bail!(
        "levitate-docs binary not found at {}.\n\
         \n\
         Build it:\n\
           cd docs/tui && bun build --compile --minify --outfile levitate-docs src/index.tsx\n\
         \n\
         The docs TUI shows installation instructions in the live ISO.",
        docs_tui_binary.display()
    );
}
