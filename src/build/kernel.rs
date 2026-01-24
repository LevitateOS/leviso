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
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::process::Cmd;

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
    let config_hash_path = build_dir.join(".config.kconfig-hash");

    let kernel_src_str = kernel_source.to_string_lossy();
    let build_dir_arg = format!("O={}", build_dir.display());

    // Compute hash of our kconfig
    let kconfig_hash = {
        let mut hasher = Sha256::new();
        hasher.update(kconfig.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    // Check if we need to regenerate .config
    let need_config_regen = if config_path.exists() && config_hash_path.exists() {
        let cached_hash = fs::read_to_string(&config_hash_path).unwrap_or_default();
        cached_hash.trim() != kconfig_hash
    } else {
        true
    };

    if need_config_regen {
        // Start with x86_64 defconfig
        println!("  Generating base config from defconfig...");
        Cmd::new("make")
            .args(["-C", &kernel_src_str, &build_dir_arg, "x86_64_defconfig"])
            .error_msg("make defconfig failed")
            .run()?;

        // Apply our custom options
        println!("  Applying LevitateOS kernel config from kconfig...");
        apply_kernel_config(&config_path, &kconfig)?;

        // Resolve dependencies
        println!("  Resolving config dependencies...");
        Cmd::new("make")
            .args(["-C", &kernel_src_str, &build_dir_arg, "olddefconfig"])
            .error_msg("make olddefconfig failed")
            .run()?;

        // Cache the kconfig hash
        fs::write(&config_hash_path, &kconfig_hash)?;
    } else {
        println!("  [SKIP] Config unchanged, reusing existing .config");
    }

    // Always run olddefconfig to handle new kernel options without prompting
    // This is needed even when kconfig is unchanged because the kernel source
    // may have been updated with new config options.
    println!("  Resolving any new config options...");
    Cmd::new("make")
        .args(["-C", &kernel_src_str, &build_dir_arg, "olddefconfig"])
        .error_msg("make olddefconfig failed")
        .run()?;

    let cpus = match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(e) => {
            eprintln!("  [WARN] Could not detect CPU count ({}), using 4 cores", e);
            4
        }
    };
    let jobs_arg = format!("-j{}", cpus);

    // Build kernel (interactive - user sees progress)
    // make will skip files that are already up-to-date
    println!("  Building kernel...");
    Cmd::new("make")
        .args(["-C", &kernel_src_str, &build_dir_arg, &jobs_arg])
        .error_msg("Kernel build failed")
        .run_interactive()?;

    // Build modules (interactive - user sees progress)
    println!("  Building modules...");
    Cmd::new("make")
        .args(["-C", &kernel_src_str, &build_dir_arg, &jobs_arg, "modules"])
        .error_msg("Module build failed")
        .run_interactive()?;

    let version = get_kernel_version(&build_dir)?;
    println!("  Kernel version: {}", version);

    Ok(version)
}

/// Apply kernel configuration options from kconfig content.
fn apply_kernel_config(config_path: &Path, kconfig: &str) -> Result<()> {
    // FAIL FAST: If config file exists but is unreadable, that's a real error
    // Don't silently treat corrupted/unreadable config as empty
    let mut config = if config_path.exists() {
        fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read kernel config at {}", config_path.display()))?
    } else {
        String::new()
    };

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

    // Atomic Installation: Install to a temporary directory first
    let temp_staging = staging.parent().unwrap().join("staging.tmp");
    if temp_staging.exists() {
        fs::remove_dir_all(&temp_staging)?;
    }
    fs::create_dir_all(&temp_staging)?;

    let boot_dir = temp_staging.join("boot");
    let modules_dir = temp_staging.join("usr/lib/modules").join(&version);
    fs::create_dir_all(&boot_dir)?;
    fs::create_dir_all(&modules_dir)?;

    let kernel_dest = boot_dir.join("vmlinuz");
    fs::copy(&vmlinux, &kernel_dest)?;
    println!("  Installed /boot/vmlinuz");

    println!("  Installing modules to /usr/lib/modules/{}...", version);
    Cmd::new("make")
        .args(["-C", &kernel_source.to_string_lossy()])
        .arg(format!("O={}", build_dir.display()))
        .arg(format!("INSTALL_MOD_PATH={}", temp_staging.display()))
        .arg("modules_install")
        .error_msg("Module install failed")
        .run_interactive()?;

    // Fix UsrMerge: make modules_install puts files in /lib/modules,
    // but we want them in /usr/lib/modules.
    let lib_modules = temp_staging.join("lib/modules");
    let usr_lib_modules = temp_staging.join("usr/lib/modules");
    
    if lib_modules.exists() {
        println!("  Moving modules from lib/modules to usr/lib/modules...");
        // Ensure destination parent exists
        fs::create_dir_all(&usr_lib_modules)?;
        
        // Move the content (e.g., 6.12.0-levitate directory)
        for entry in fs::read_dir(&lib_modules)? {
            let entry = entry?;
            let name = entry.file_name();
            let src = entry.path();
            let dst = usr_lib_modules.join(&name);
            
            if dst.exists() {
                fs::remove_dir_all(&dst)?;
            }
            fs::rename(&src, &dst)?;
        }
        // Remove the empty lib/modules
        let _ = fs::remove_dir_all(&lib_modules);
        // Remove lib if empty
        let _ = fs::remove_dir(temp_staging.join("lib"));
    }

    let _ = fs::remove_file(modules_dir.join("source"));
    let _ = fs::remove_file(modules_dir.join("build"));

    let mut module_count = 0;
    let mut walk_errors = 0;
    for entry in walkdir::WalkDir::new(&modules_dir) {
        match entry {
            Ok(e) => {
                // Count files with kernel module extensions (.ko, .ko.xz, .ko.gz)
                if e.path()
                    .extension()
                    .map(|ext| ext == "ko" || ext == "xz" || ext == "gz")
                    .unwrap_or(false)
                {
                    module_count += 1;
                }
            }
            Err(e) => {
                walk_errors += 1;
                eprintln!("  [WARN] Error reading module entry: {}", e);
            }
        }
    }
    if walk_errors > 0 {
        eprintln!(
            "  [WARN] {} errors encountered while counting modules (count may be inaccurate)",
            walk_errors
        );
    }
    println!("  Installed {} kernel modules", module_count);

    // Final Integrity Check: Does the installed module directory match the version?
    if !temp_staging.join("usr/lib/modules").join(&version).exists() {
        bail!("Kernel installation failed: modules directory for version {} not found in staging", version);
    }

    // Atomic Swap: rename temp_staging to staging
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    fs::rename(&temp_staging, staging)?;

    Ok(version)
}
