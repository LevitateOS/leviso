//! QEMU + OVMF testing dependencies via recipe.
//!
//! Downloads pre-built RPMs from Rocky mirrors and extracts them.
//! Used by `leviso test` and `leviso run`.

use super::find_recipe;
use anyhow::Result;
use distro_builder::process::ensure_exists;
use std::path::Path;

/// Ensure QEMU and OVMF are available for boot testing.
///
/// Downloads pre-built RPMs from Rocky mirrors if not already on the system.
pub fn ensure_qemu(base_dir: &Path) -> Result<()> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/qemu.rhai");

    ensure_exists(&recipe_path, "QEMU recipe").map_err(|_| {
        anyhow::anyhow!(
            "QEMU recipe not found at: {}\n\
             Expected qemu.rhai in leviso/deps/",
            recipe_path.display()
        )
    })?;

    let recipes_dir = monorepo_dir.join("distro-builder/recipes");
    let recipe_bin = find_recipe(&monorepo_dir)?;

    recipe_bin.run_with_recipes_path(&recipe_path, &downloads_dir, Some(&recipes_dir))?;

    // Prepend extracted tools to PATH and set OVMF_PATH
    // deps resolution installs to BUILD_DIR/.tools/ (not .deps/name/.tools/)
    let tools_prefix = downloads_dir.join(".tools");
    let tools_bin = tools_prefix.join("usr/bin");
    let tools_libexec = tools_prefix.join("usr/libexec");
    if tools_bin.exists() || tools_libexec.exists() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let mut new_paths = Vec::new();
        for dir in [&tools_bin, &tools_libexec] {
            if dir.exists() && !current_path.contains(&dir.to_string_lossy().to_string()) {
                new_paths.push(dir.to_string_lossy().to_string());
            }
        }
        if !new_paths.is_empty() {
            new_paths.push(current_path);
            unsafe {
                std::env::set_var("PATH", new_paths.join(":"));
            }
        }
    }

    // Set OVMF_PATH so recqemu::find_ovmf() can find extracted firmware
    let ovmf = tools_prefix.join("usr/share/edk2/ovmf/OVMF_CODE.fd");
    if ovmf.exists() {
        unsafe {
            std::env::set_var("OVMF_PATH", ovmf.to_string_lossy().as_ref());
        }
    }

    // Set LD_LIBRARY_PATH so QEMU finds extracted shared libs (capstone, pixman, slirp)
    let tools_lib64 = tools_prefix.join("usr/lib64");
    if tools_lib64.exists() {
        let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        if !current.contains(&tools_lib64.to_string_lossy().to_string()) {
            let new_val = if current.is_empty() {
                tools_lib64.to_string_lossy().to_string()
            } else {
                format!("{}:{}", tools_lib64.display(), current)
            };
            unsafe {
                std::env::set_var("LD_LIBRARY_PATH", &new_val);
            }
        }
    }

    Ok(())
}
