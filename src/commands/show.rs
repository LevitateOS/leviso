//! Show command - displays information.

use anyhow::Result;
use std::path::Path;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, SQUASHFS_NAME};

use crate::config::Config;
use distro_builder::process::Cmd;
use crate::rebuild;
use leviso_deps::DependencyResolver;

/// Show target for the show command.
pub enum ShowTarget {
    /// Show configuration
    Config,
    /// Show squashfs contents
    Squashfs,
    /// Show build status
    Status,
}

/// Execute the show command.
pub fn cmd_show(
    base_dir: &Path,
    target: ShowTarget,
    config: &Config,
    resolver: &DependencyResolver,
) -> Result<()> {
    match target {
        ShowTarget::Config => {
            config.print();
            println!();
            resolver.print_status();
        }
        ShowTarget::Squashfs => {
            let squashfs = base_dir.join("output/filesystem.squashfs");
            if !squashfs.exists() {
                anyhow::bail!("Squashfs not found. Run 'leviso build squashfs' first.");
            }
            // Use unsquashfs -l to list contents
            Cmd::new("unsquashfs")
                .args(["-l"])
                .arg_path(&squashfs)
                .error_msg("unsquashfs failed. Install: sudo dnf install squashfs-tools")
                .run_interactive()?;
        }
        ShowTarget::Status => {
            show_build_status(base_dir)?;
        }
    }
    Ok(())
}

/// Show what will be rebuilt on next `leviso build`.
fn show_build_status(base_dir: &Path) -> Result<()> {
    println!("=== Build Status ===\n");

    // Check each artifact
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");
    let squashfs = base_dir.join("output").join(SQUASHFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    let iso = base_dir.join("output").join(ISO_FILENAME);

    // Kernel
    let kernel_compile = rebuild::kernel_needs_compile(base_dir);
    let kernel_install = rebuild::kernel_needs_install(base_dir);
    print!("Kernel (compile):  ");
    if !bzimage.exists() {
        println!("MISSING → will build");
    } else if kernel_compile {
        println!("STALE → will rebuild");
    } else {
        println!("OK (up to date)");
    }

    print!("Kernel (install):  ");
    if !vmlinuz.exists() {
        println!("MISSING → will install");
    } else if kernel_install {
        println!("STALE → will reinstall");
    } else {
        println!("OK (up to date)");
    }

    // Squashfs
    let squashfs_rebuild = rebuild::squashfs_needs_rebuild(base_dir);
    print!("Squashfs:          ");
    if !squashfs.exists() {
        println!("MISSING → will build");
    } else if squashfs_rebuild {
        println!("STALE → will rebuild");
    } else {
        println!("OK (up to date)");
    }

    // Initramfs
    let initramfs_rebuild = rebuild::initramfs_needs_rebuild(base_dir);
    print!("Initramfs:         ");
    if !initramfs.exists() {
        println!("MISSING → will build");
    } else if initramfs_rebuild {
        println!("STALE → will rebuild");
    } else {
        println!("OK (up to date)");
    }

    // ISO
    let iso_rebuild = rebuild::iso_needs_rebuild(base_dir);
    print!("ISO:               ");
    if !iso.exists() {
        println!("MISSING → will build");
    } else if iso_rebuild {
        println!("STALE → will rebuild");
    } else {
        println!("OK (up to date)");
    }

    // Summary
    println!();
    let any_work = kernel_compile || kernel_install || squashfs_rebuild || initramfs_rebuild || iso_rebuild;
    if any_work {
        println!("Run 'leviso build' to rebuild stale/missing artifacts.");
    } else {
        println!("All artifacts up to date. Nothing to rebuild.");
    }

    Ok(())
}
