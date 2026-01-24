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

// Re-export public API
pub use live::create_live_overlay_at;

/// Execute a custom operation.
pub fn execute(ctx: &BuildContext, op: CustomOp) -> Result<()> {
    match op {
        // Filesystem operations
        CustomOp::CreateFhsSymlinks => filesystem::create_fhs_symlinks(ctx),

        // Live overlay
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::CopyRecstrap => live::copy_recstrap(ctx),
        CustomOp::SetupLiveSystemdConfigs => live::setup_live_systemd_configs(ctx),

        // Firmware
        CustomOp::CopyWifiFirmware => firmware::copy_wifi_firmware(ctx),
        CustomOp::CopyAllFirmware => firmware::copy_all_firmware(ctx),

        // Kernel modules
        CustomOp::RunDepmod => modules::run_depmod(ctx),
        CustomOp::CopyModules => modules::copy_modules(ctx),

        // /etc configuration
        CustomOp::CreateEtcFiles => etc::create_etc_files(ctx),
        CustomOp::CopyTimezoneData => etc::copy_timezone_data(ctx),
        CustomOp::CopyLocales => etc::copy_locales(ctx),

        // PAM and security
        CustomOp::CreatePamFiles => pam::create_pam_files(ctx),
        CustomOp::CreateSecurityConfig => pam::create_security_config(ctx),
        CustomOp::DisableSelinux => pam::disable_selinux(ctx),

        // Package manager and bootloader
        CustomOp::CopyDracutModules => packages::copy_dracut_modules(ctx),
        CustomOp::CopySystemdBootEfi => packages::copy_systemd_boot_efi(ctx),
        CustomOp::CopyKeymaps => packages::copy_keymaps(ctx),
        CustomOp::CopyRecipe => packages::copy_recipe(ctx),
        CustomOp::SetupRecipeConfig => packages::setup_recipe_config(ctx),
        CustomOp::CreateDracutConfig => packages::create_dracut_config(ctx),
        CustomOp::CopyDocsTui => packages::copy_docs_tui(ctx),
    }
}
