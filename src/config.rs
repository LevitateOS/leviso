//! Configuration management for leviso.
//!
//! Build-time settings that aren't handled by the dependency resolver.
//! For dependency resolution (Linux, Rocky, tools), see `deps/` module.

use std::env;

/// Default essential modules for initramfs.
pub mod module_defaults {
    /// Default essential kernel modules.
    /// Format: paths relative to /lib/modules/<version>/
    pub const ESSENTIAL_MODULES: &[&str] = &[
        // Block device driver (for virtual disks)
        "kernel/drivers/block/virtio_blk.ko.xz",
        // ext4 filesystem and dependencies
        "kernel/fs/mbcache.ko.xz",
        "kernel/fs/jbd2/jbd2.ko.xz",
        "kernel/fs/ext4/ext4.ko.xz",
        // FAT/vfat filesystem for EFI partition
        "kernel/fs/fat/fat.ko.xz",
        "kernel/fs/fat/vfat.ko.xz",
        // SCSI/CD-ROM support (for installation media access)
        "kernel/drivers/scsi/virtio_scsi.ko.xz",
        "kernel/drivers/cdrom/cdrom.ko.xz",
        "kernel/drivers/scsi/sr_mod.ko.xz",
        // ISO 9660 filesystem (to mount installation media)
        "kernel/fs/isofs/isofs.ko.xz",
        // Network - virtio (VM networking)
        "kernel/net/core/failover.ko.xz",       // Required by net_failover
        "kernel/drivers/net/net_failover.ko.xz", // Required by virtio_net
        "kernel/drivers/net/virtio_net.ko.xz",
        // Network - common ethernet drivers
        "kernel/drivers/net/ethernet/intel/e1000/e1000.ko.xz",
        "kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko.xz",
        "kernel/drivers/net/ethernet/realtek/r8169.ko.xz",
    ];
}

/// Leviso build configuration.
///
/// For dependency paths (Linux source, Rocky ISO, tools), use `DependencyResolver`.
/// This struct only contains build-time settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Kernel version suffix (e.g., "-levitate")
    pub kernel_localversion: String,
    /// Additional kernel modules to include in initramfs
    pub extra_modules: Vec<String>,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Note: .env file is loaded by DependencyResolver via dotenvy.
    /// Call DependencyResolver::new() first to ensure .env is loaded.
    pub fn load() -> Self {
        // dotenvy should already be loaded by DependencyResolver
        // Log if .env file loading fails (file missing is OK, read errors are not)
        if let Err(e) = dotenvy::dotenv() {
            match e {
                dotenvy::Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::NotFound => {
                    // .env file not found - this is fine, not all projects use one
                }
                _ => {
                    eprintln!("  [WARN] Failed to load .env file: {}", e);
                }
            }
        }

        let kernel_localversion = match env::var("KERNEL_LOCALVERSION") {
            Ok(v) => v,
            Err(_) => {
                let default = "-levitate".to_string();
                // Only log if there's something custom expected (don't spam on normal runs)
                // For now, silent default is acceptable for this non-critical value
                default
            }
        };

        // Parse extra modules from comma-separated list
        let extra_modules = env::var("EXTRA_MODULES")
            .map(|s| {
                s.split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            kernel_localversion,
            extra_modules,
        }
    }

    /// Print configuration for debugging.
    pub fn print(&self) {
        println!("Build Configuration:");
        println!("  KERNEL_LOCALVERSION: {}", self.kernel_localversion);
        if !self.extra_modules.is_empty() {
            println!("  EXTRA_MODULES:");
            for module in &self.extra_modules {
                println!("    - {}", module);
            }
        }
    }

    /// Get all modules (defaults + extra).
    pub fn all_modules(&self) -> Vec<&str> {
        let mut modules: Vec<&str> = module_defaults::ESSENTIAL_MODULES.to_vec();
        for extra in &self.extra_modules {
            modules.push(extra);
        }
        modules
    }
}
