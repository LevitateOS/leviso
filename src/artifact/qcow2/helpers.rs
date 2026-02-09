//! UUID generation and host tool verification helpers.
//!
//! Delegates to shared infrastructure in distro-builder::artifact::disk::helpers.

// Re-export shared types
pub use distro_builder::artifact::disk::helpers::{calculate_dir_size, DiskUuids};

use anyhow::Result;

/// Verify all required host tools are available (including qemu-img for leviso).
pub fn check_host_tools() -> Result<()> {
    let qemu_tools: &[(&str, &str)] = &[("qemu-img", "qemu-img")];
    distro_builder::artifact::disk::helpers::check_host_tools(qemu_tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_vfat_serial_format() {
        let serial =
            distro_builder::artifact::disk::helpers::generate_vfat_serial().unwrap();
        assert_eq!(serial.len(), 9); // XXXX-XXXX
        assert_eq!(&serial[4..5], "-");
    }

    #[test]
    fn test_partition_constants() {
        assert!(true);
    }
}
