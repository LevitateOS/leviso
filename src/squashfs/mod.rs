//! Squashfs builder - creates the complete LevitateOS system image.
//!
//! The squashfs serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! # Architecture
//!
//! ```text
//! ISO Contents:
//! ├── boot/
//! │   ├── vmlinuz              # Kernel
//! │   └── initramfs.img        # Tiny (~5MB) - busybox + mount logic
//! ├── live/
//! │   └── filesystem.squashfs  # COMPLETE system (~350MB)
//! └── EFI/BOOT/
//!     ├── BOOTX64.EFI
//!     └── grub.cfg
//!
//! Live Boot Flow:
//! 1. GRUB loads kernel + tiny initramfs
//! 2. Tiny init mounts ISO by LABEL
//! 3. Mounts filesystem.squashfs read-only via loop device
//! 4. Creates overlay: squashfs (lower) + tmpfs (upper)
//! 5. switch_root to overlay
//! 6. systemd boots as PID 1
//! ```
//!
//! DESIGN: Live = Installed (same content, zero duplication)

pub mod pack;
pub mod system;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::build::BuildContext;

/// Build the complete squashfs system image.
///
/// This creates a filesystem.squashfs in output/ containing the complete
/// LevitateOS system ready for both live boot and installation.
pub fn build_squashfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Squashfs System Image ===\n");

    // 1. Check host tools
    check_host_tools()?;

    // 2. Set up paths
    let staging = base_dir.join("output/squashfs-root");
    let output = base_dir.join("output/filesystem.squashfs");

    // 3. Clean staging if exists
    if staging.exists() {
        println!("Cleaning previous staging directory...");
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    // 4. Build complete system into staging
    let ctx = BuildContext::new(base_dir, &staging)?;
    system::build_system(&ctx)?;

    // 5. Pack into squashfs
    pack::create_squashfs(&staging, &output)?;

    println!("\n=== Squashfs Build Complete ===");
    println!("  Output: {}", output.display());

    // Print size info
    if let Ok(meta) = fs::metadata(&output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Check that required host tools are available.
fn check_host_tools() -> Result<()> {
    let tools = [
        ("mksquashfs", "squashfs-tools"),
        ("readelf", "binutils"),
    ];

    for (tool, package) in tools {
        let status = Command::new("which")
            .arg(tool)
            .output()
            .context(format!("Failed to check for {}", tool))?;

        if !status.status.success() {
            bail!(
                "{} not found. Install {} package.\n\
                 On Fedora: sudo dnf install {}\n\
                 On Ubuntu: sudo apt install {}",
                tool,
                package,
                package,
                package
            );
        }
    }

    Ok(())
}
