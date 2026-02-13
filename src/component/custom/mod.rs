//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module here handles a specific domain of custom operations.
//!
//! NOTE: This is split from the original 1,283-line custom.rs for maintainability.
//! Each module is ~100-250 lines focused on a single domain.

// Submodules colocated with their configuration files
mod etc; // src/component/custom/etc/ - contains etc/mod.rs and etc/files/
mod firmware;
mod live; // src/component/custom/live/ - contains live/mod.rs and live/overlay/
mod modules;
mod packages; // src/component/custom/packages/ - contains packages/mod.rs and packages/files/
mod pam;

use anyhow::Result;

use super::CustomOp;
use crate::build::context::BuildContext;
use distro_builder::LicenseTracker;

// Re-export public API
pub use live::{create_live_overlay_at, read_test_instrumentation};

/// Execute a custom operation.
///
/// Some operations copy content that requires license tracking. The tracker
/// is used to register packages for license compliance.
pub fn execute(ctx: &BuildContext, op: CustomOp, tracker: &LicenseTracker) -> Result<()> {
    match op {
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
        CustomOp::InstallCheckpointTests => install_checkpoint_tests(ctx),
    }
}

/// Install docs-tui (levitate-docs) to staging.
///
/// AUTOMATICALLY REBUILDS the docs-TUI before copying to ensure latest version.
/// Copies the binary AND its library dependencies to staging.
/// The levitate-docs binary is compiled with Bun and links against glibc,
/// so we must ensure libpthread.so.0, libdl.so.2, libm.so.6 etc. are present.
fn install_docs_tui(ctx: &BuildContext) -> Result<()> {
    use crate::build::libdeps::copy_library;
    use anyhow::{bail, Context};
    use leviso_elf::{get_all_dependencies, make_executable};
    use std::fs;
    use std::process::Command;

    let monorepo_dir = ctx
        .base_dir
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
        .args([
            "build",
            "--compile",
            "--minify",
            "--outfile",
            "levitate-docs",
            "src/index.ts",
        ])
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
    fs::copy(&docs_tui_binary, &dest).with_context(|| "Failed to copy levitate-docs to staging")?;
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

    println!(
        "  Copying {} library dependencies for levitate-docs...",
        libs.len()
    );
    for lib_name in &libs {
        copy_library(ctx, lib_name, None)
            .with_context(|| format!("levitate-docs requires missing library '{}'", lib_name))?;
    }

    Ok(())
}

/// Install checkpoint test scripts to the rootfs staging directory.
///
/// Source (monorepo): `testing/install-tests/test-scripts/`
/// Destination (in rootfs/ISO): `/usr/local/bin/checkpoint-*.sh`
/// Libraries: `/usr/local/lib/checkpoint-tests/`
fn install_checkpoint_tests(ctx: &BuildContext) -> Result<()> {
    use std::fs;

    let monorepo_root = ctx
        .base_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine monorepo root"))?;
    let test_scripts_src = monorepo_root.join("testing/install-tests/test-scripts");

    if !test_scripts_src.exists() {
        anyhow::bail!(
            "Test scripts not found at: {}\n\
             Expected checkpoint test scripts in testing/install-tests/test-scripts/",
            test_scripts_src.display()
        );
    }

    // Destination: /usr/local/bin/ for scripts, /usr/local/lib/checkpoint-tests/ for libraries
    let bin_dst = ctx.staging.join("usr/local/bin");
    let lib_dst = ctx.staging.join("usr/local/lib/checkpoint-tests");

    fs::create_dir_all(&bin_dst)?;
    fs::create_dir_all(&lib_dst)?;

    // Copy all .sh scripts to /usr/local/bin/
    let mut script_count = 0;
    for entry in fs::read_dir(&test_scripts_src)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|ext| ext == "sh") {
            let filename = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            let dst = bin_dst.join(filename);

            fs::copy(&path, &dst)?;

            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dst)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dst, perms)?;
            }

            script_count += 1;
        }
    }

    // Copy lib/ directory to /usr/local/lib/checkpoint-tests/
    let lib_src = test_scripts_src.join("lib");
    if lib_src.exists() {
        for entry in fs::read_dir(&lib_src)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let filename = path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                let dst = lib_dst.join(filename);

                fs::copy(&path, &dst)?;

                // Make library files executable (they may be sourced)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&dst, perms)?;
                }
            }
        }
    }

    println!(
        "  Installed {} checkpoint test scripts to /usr/local/bin/",
        script_count
    );
    println!("  Installed checkpoint test libraries to /usr/local/lib/checkpoint-tests/");

    Ok(())
}
