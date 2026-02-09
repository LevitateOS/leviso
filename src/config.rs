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
        "kernel/net/core/failover.ko.xz", // Required by net_failover
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
                // Only log if there's something custom expected (don't spam on normal runs)
                // For now, silent default is acceptable for this non-critical value
                "-levitate".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_module_defaults_contain_essentials() {
        let modules = module_defaults::ESSENTIAL_MODULES;

        // Must have virtio for VM disk access
        assert!(modules.iter().any(|m| m.contains("virtio_blk")));
        // Must have ext4 for root filesystem
        assert!(modules.iter().any(|m| m.contains("ext4")));
        // Must have FAT for EFI partition
        assert!(modules
            .iter()
            .any(|m| m.contains("fat") || m.contains("vfat")));
    }

    #[test]
    #[serial]
    fn test_config_all_modules_includes_extras() {
        // Set env var directly
        env::set_var(
            "EXTRA_MODULES",
            "kernel/drivers/nvme/host/nvme.ko.xz,kernel/fs/xfs/xfs.ko.xz",
        );

        let config = Config::load();

        // Clean up before assertions
        env::remove_var("EXTRA_MODULES");

        let all_modules = config.all_modules();

        // Should include defaults
        assert!(all_modules.iter().any(|m| m.contains("virtio_blk")));
        assert!(all_modules.iter().any(|m| m.contains("ext4")));

        // Should include extras
        assert!(all_modules.iter().any(|m| m.contains("nvme")));
        assert!(all_modules.iter().any(|m| m.contains("xfs")));
    }

    #[test]
    #[serial]
    fn test_config_empty_extra_modules() {
        // Set empty env var
        env::set_var("EXTRA_MODULES", "");

        let config = Config::load();

        // Clean up
        env::remove_var("EXTRA_MODULES");

        // Extra modules should be empty
        assert!(config.extra_modules.is_empty());

        // But all_modules should still have defaults
        let all_modules = config.all_modules();
        assert!(!all_modules.is_empty());
    }
}
