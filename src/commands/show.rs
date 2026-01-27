//! Show command - displays information.

use anyhow::Result;
use std::path::Path;

use distro_spec::levitate::{INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME};

use crate::config::Config;
use crate::recipe;
use distro_builder::process::Cmd;
use crate::rebuild;

/// Show target for the show command.
pub enum ShowTarget {
    /// Show configuration
    Config,
    /// Show rootfs (EROFS) contents
    Rootfs,
    /// Show build status
    Status,
}

/// Execute the show command.
pub fn cmd_show(
    base_dir: &Path,
    target: ShowTarget,
    config: &Config,
) -> Result<()> {
    match target {
        ShowTarget::Config => {
            config.print();
            println!();
            print_dependency_status(base_dir);
        }
        ShowTarget::Rootfs => {
            let rootfs = base_dir.join("output").join(ROOTFS_NAME);
            if !rootfs.exists() {
                anyhow::bail!("Rootfs not found. Run 'leviso build rootfs' first.");
            }
            // Use fsck.erofs to show EROFS image info
            println!("=== EROFS Rootfs Info ===\n");
            Cmd::new("fsck.erofs")
                .arg("--print-comp-cfgs")
                .arg_path(&rootfs)
                .error_msg("fsck.erofs failed. Install: sudo dnf install erofs-utils")
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
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
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

    // Rootfs (EROFS)
    let rootfs_rebuild = rebuild::rootfs_needs_rebuild(base_dir);
    print!("Rootfs (EROFS):    ");
    if !rootfs.exists() {
        println!("MISSING → will build");
    } else if rootfs_rebuild {
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
    let any_work = kernel_compile || kernel_install || rootfs_rebuild || initramfs_rebuild || iso_rebuild;
    if any_work {
        println!("Run 'leviso build' to rebuild stale/missing artifacts.");
    } else {
        println!("All artifacts up to date. Nothing to rebuild.");
    }

    Ok(())
}

/// Print dependency status (replaces resolver.print_status()).
fn print_dependency_status(base_dir: &Path) {
    let monorepo = base_dir.parent().unwrap_or(base_dir);

    println!("Dependency Status:");
    println!("  Base dir:     {}", base_dir.display());
    println!("  Monorepo dir: {}", monorepo.display());
    println!();

    // Linux
    if recipe::has_linux_source(base_dir) {
        let submodule = monorepo.join("linux");
        let downloaded = base_dir.join("downloads/linux");
        if submodule.join("Makefile").exists() {
            println!("  Linux: FOUND at {} (submodule)", submodule.display());
        } else if downloaded.join("Makefile").exists() {
            println!("  Linux: FOUND at {} (downloaded)", downloaded.display());
        }
    } else {
        println!("  Linux: NOT FOUND (will download via recipe)");
    }

    // Rocky
    let iso_path = base_dir.join("downloads/Rocky-10.1-x86_64-dvd1.iso");
    if iso_path.exists() {
        println!("  Rocky: FOUND at {}", iso_path.display());
    } else {
        println!("  Rocky: NOT FOUND (will download via recipe)");
    }
}
