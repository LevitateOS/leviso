//! Show command - displays information.

use anyhow::Result;
use std::path::Path;

use distro_spec::levitate::{
    INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME,
};

use crate::config::Config;
use crate::rebuild;
use crate::recipe;
use distro_builder::process::Cmd;

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
pub fn cmd_show(base_dir: &Path, target: ShowTarget, config: &Config) -> Result<()> {
    match target {
        ShowTarget::Config => {
            config.print();
            println!();
            print_dependency_status(base_dir);
        }
        ShowTarget::Rootfs => {
            let output_dir =
                distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
            let rootfs = output_dir.join(ROOTFS_NAME);
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
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let bzimage = output_dir.join("kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = output_dir.join("staging/boot/vmlinuz");
    let rootfs = output_dir.join(ROOTFS_NAME);
    let initramfs = output_dir.join(INITRAMFS_LIVE_OUTPUT);
    let install_initramfs = output_dir.join(INITRAMFS_INSTALLED_OUTPUT);
    let iso = output_dir.join(ISO_FILENAME);

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

    // Install initramfs
    let install_initramfs_rebuild = rebuild::install_initramfs_needs_rebuild(base_dir);
    print!("Install initramfs: ");
    if !install_initramfs.exists() {
        println!("MISSING → will build");
    } else if install_initramfs_rebuild {
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
    let any_work = kernel_compile
        || kernel_install
        || rootfs_rebuild
        || initramfs_rebuild
        || install_initramfs_rebuild
        || iso_rebuild;
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
        println!("  Linux: FOUND (tarball-extracted in downloads/)");
    } else {
        println!("  Linux: NOT FOUND (will download tarball from cdn.kernel.org)");
    }

    // Rocky
    let iso_path = base_dir.join("downloads/Rocky-10.1-x86_64-dvd1.iso");
    if iso_path.exists() {
        println!("  Rocky: FOUND at {}", iso_path.display());
    } else {
        println!("  Rocky: NOT FOUND (will download via recipe)");
    }
}
