//! Kernel building and installation.
//!
//! LevitateOS targets MODERN HARDWARE with decent specs:
//! - 8GB+ RAM (16GB+ recommended)
//! - x86-64-v3 CPUs (Haswell 2013+)
//! - NVMe SSDs, modern GPUs, WiFi 6, etc.
//!
//! This is NOT an embedded/minimal kernel. Enable everything a daily driver needs.
//!
//! Kernel config is in `kconfig` at the project root.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Build the kernel from source.
pub fn build_kernel(kernel_source: &Path, output_dir: &Path, base_dir: &Path) -> Result<String> {
    println!("Building kernel from {}...", kernel_source.display());

    if !kernel_source.exists() {
        bail!(
            "Kernel source not found at {}\nRun: git submodule update --init linux",
            kernel_source.display()
        );
    }

    if !kernel_source.join("Makefile").exists() {
        bail!("Invalid kernel source - no Makefile found");
    }

    // Read our kconfig
    let kconfig_path = base_dir.join("kconfig");
    if !kconfig_path.exists() {
        bail!(
            "Kernel config not found at {}\nExpected kconfig file in project root.",
            kconfig_path.display()
        );
    }
    let kconfig = fs::read_to_string(&kconfig_path)
        .with_context(|| format!("Failed to read {}", kconfig_path.display()))?;

    fs::create_dir_all(output_dir)?;
    let build_dir = output_dir.join("kernel-build");
    fs::create_dir_all(&build_dir)?;

    let config_path = build_dir.join(".config");

    // Start with x86_64 defconfig
    println!("  Generating base config from defconfig...");
    let defconfig = Command::new("make")
        .args(["-C", kernel_source.to_str().unwrap()])
        .arg(format!("O={}", build_dir.display()))
        .arg("x86_64_defconfig")
        .output()
        .context("Failed to run make defconfig")?;

    if !defconfig.status.success() {
        bail!("make defconfig failed:\n{}", String::from_utf8_lossy(&defconfig.stderr));
    }

    // Apply our custom options
    println!("  Applying LevitateOS kernel config from kconfig...");
    apply_kernel_config(&config_path, &kconfig)?;

    // Resolve dependencies
    println!("  Resolving config dependencies...");
    let olddefconfig = Command::new("make")
        .args(["-C", kernel_source.to_str().unwrap()])
        .arg(format!("O={}", build_dir.display()))
        .arg("olddefconfig")
        .output()
        .context("Failed to run make olddefconfig")?;

    if !olddefconfig.status.success() {
        bail!("make olddefconfig failed:\n{}", String::from_utf8_lossy(&olddefconfig.stderr));
    }

    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    // Build kernel
    println!("  Building kernel (this will take a while)...");
    let build = Command::new("make")
        .args(["-C", kernel_source.to_str().unwrap()])
        .arg(format!("O={}", build_dir.display()))
        .arg(format!("-j{}", cpus))
        .output()
        .context("Failed to run make")?;

    if !build.status.success() {
        bail!("Kernel build failed:\n{}", String::from_utf8_lossy(&build.stderr));
    }

    // Build modules
    println!("  Building modules...");
    let modules = Command::new("make")
        .args(["-C", kernel_source.to_str().unwrap()])
        .arg(format!("O={}", build_dir.display()))
        .arg(format!("-j{}", cpus))
        .arg("modules")
        .output()
        .context("Failed to build modules")?;

    if !modules.status.success() {
        bail!("Module build failed:\n{}", String::from_utf8_lossy(&modules.stderr));
    }

    let version = get_kernel_version(&build_dir)?;
    println!("  Kernel version: {}", version);

    Ok(version)
}

/// Apply kernel configuration options from kconfig content.
fn apply_kernel_config(config_path: &Path, kconfig: &str) -> Result<()> {
    let mut config = fs::read_to_string(config_path).unwrap_or_default();

    for line in kconfig.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, _value)) = line.split_once('=') {
            let pattern = format!("{}=", key);
            let pattern_not = format!("# {} is not set", key);
            config = config
                .lines()
                .filter(|l| !l.starts_with(&pattern) && !l.starts_with(&pattern_not))
                .collect::<Vec<_>>()
                .join("\n");

            config.push('\n');
            config.push_str(line);
        }
    }

    fs::write(config_path, config)?;
    Ok(())
}

/// Get the kernel version from the build directory.
fn get_kernel_version(build_dir: &Path) -> Result<String> {
    let release_path = build_dir.join("include/config/kernel.release");
    if release_path.exists() {
        return Ok(fs::read_to_string(&release_path)?.trim().to_string());
    }

    let makefile = build_dir.join("Makefile");
    if makefile.exists() {
        let content = fs::read_to_string(&makefile)?;
        let mut version = String::new();
        let mut patchlevel = String::new();
        let mut sublevel = String::new();
        let mut extraversion = String::new();

        for line in content.lines() {
            if let Some(v) = line.strip_prefix("VERSION = ") {
                version = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("PATCHLEVEL = ") {
                patchlevel = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("SUBLEVEL = ") {
                sublevel = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("EXTRAVERSION = ") {
                extraversion = v.trim().to_string();
            }
        }

        if !version.is_empty() && !patchlevel.is_empty() {
            return Ok(format!("{}.{}.{}{}", version, patchlevel, sublevel, extraversion));
        }
    }

    bail!("Could not determine kernel version")
}

/// Install kernel and modules to staging directory.
pub fn install_kernel(kernel_source: &Path, build_output: &Path, staging: &Path) -> Result<String> {
    let build_dir = build_output.join("kernel-build");

    let vmlinux = build_dir.join("arch/x86/boot/bzImage");
    if !vmlinux.exists() {
        bail!("Kernel not built. Run build_kernel() first.\nExpected: {}", vmlinux.display());
    }

    let version = get_kernel_version(&build_dir)?;
    println!("Installing kernel {} to staging...", version);

    let boot_dir = staging.join("boot");
    let modules_dir = staging.join("usr/lib/modules").join(&version);
    fs::create_dir_all(&boot_dir)?;
    fs::create_dir_all(&modules_dir)?;

    let kernel_dest = boot_dir.join("vmlinuz");
    fs::copy(&vmlinux, &kernel_dest)?;
    println!("  Installed /boot/vmlinuz");

    println!("  Installing modules to /usr/lib/modules/{}...", version);
    let modules_install = Command::new("make")
        .args(["-C", kernel_source.to_str().unwrap()])
        .arg(format!("O={}", build_dir.display()))
        .arg(format!("INSTALL_MOD_PATH={}", staging.display()))
        .arg("modules_install")
        .output()
        .context("Failed to install modules")?;

    if !modules_install.status.success() {
        bail!("Module install failed:\n{}", String::from_utf8_lossy(&modules_install.stderr));
    }

    let _ = fs::remove_file(modules_dir.join("source"));
    let _ = fs::remove_file(modules_dir.join("build"));

    let module_count = walkdir::WalkDir::new(&modules_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "ko" || ext == "xz").unwrap_or(false))
        .count();
    println!("  Installed {} kernel modules", module_count);

    Ok(version)
}
