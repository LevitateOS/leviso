//! LevitateOS DistroConfig implementation.
//!
//! Implements the distro-contract traits for LevitateOS configuration.

use distro_contract::{
    context::{DistroConfig, InitSystem},
    KernelInstallConfig,
};
use distro_spec::levitate::*;

/// LevitateOS configuration - implements DistroConfig trait.
pub struct LevitateOsConfig;

impl KernelInstallConfig for LevitateOsConfig {
    fn module_install_path(&self) -> &str {
        "usr/lib/modules"
    }

    fn kernel_filename(&self) -> &str {
        "vmlinuz"
    }
}

impl DistroConfig for LevitateOsConfig {
    fn os_name(&self) -> &str {
        "LevitateOS"
    }

    fn os_id(&self) -> &str {
        "levitateos"
    }

    fn iso_label(&self) -> &str {
        ISO_LABEL
    }

    fn boot_modules(&self) -> &[&str] {
        BOOT_MODULES
    }

    fn default_shell(&self) -> &str {
        DEFAULT_SHELL
    }

    fn init_system(&self) -> InitSystem {
        InitSystem::Systemd
    }
}
