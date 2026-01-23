//! Rebuild detection logic.
//!
//! Uses hash-based caching to skip rebuilding artifacts that haven't changed.
//! This provides faster incremental builds by detecting when inputs change.

use std::path::Path;

use distro_spec::levitate::{INITRAMFS_OUTPUT, ISO_FILENAME, SQUASHFS_NAME};

use crate::cache;

/// Check if kernel needs to be rebuilt.
///
/// Uses hash-based detection for kconfig changes.
/// Checks both bzImage (build output) and vmlinuz (installed).
pub fn kernel_needs_rebuild(base_dir: &Path) -> bool {
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");
    let kconfig = base_dir.join("kconfig");
    let hash_file = base_dir.join("output/.kconfig.hash");

    // Must have both bzImage and vmlinuz
    if !bzimage.exists() || !vmlinuz.exists() {
        return true;
    }

    // Check if kconfig content changed (not just mtime)
    let current_hash = match cache::hash_file(&kconfig) {
        Ok(h) => h,
        Err(_) => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &bzimage)
}

/// Check if squashfs needs to be rebuilt.
///
/// Uses hash of key input files rather than staging dir mtime.
pub fn squashfs_needs_rebuild(base_dir: &Path) -> bool {
    let squashfs = base_dir.join("output").join(SQUASHFS_NAME);
    let hash_file = base_dir.join("output/.squashfs-inputs.hash");

    if !squashfs.exists() {
        return true;
    }

    // Key files that affect squashfs content
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");
    let definitions = base_dir.join("src/component/definitions.rs");

    let inputs: Vec<&Path> = vec![&rootfs_marker, &definitions];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &squashfs)
}

/// Check if initramfs needs to be rebuilt.
pub fn initramfs_needs_rebuild(base_dir: &Path) -> bool {
    let initramfs = base_dir.join("output").join(INITRAMFS_OUTPUT);
    let hash_file = base_dir.join("output/.initramfs-inputs.hash");
    let init_script = base_dir.join("profile/init_tiny.template");
    let busybox = base_dir.join("downloads/busybox-static");

    if !initramfs.exists() {
        return true;
    }

    let inputs: Vec<&Path> = vec![&init_script, &busybox];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &initramfs)
}

/// Check if ISO needs to be rebuilt.
pub fn iso_needs_rebuild(base_dir: &Path) -> bool {
    let iso = base_dir.join("output").join(ISO_FILENAME);
    let squashfs = base_dir.join("output").join(SQUASHFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_OUTPUT);
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    if !iso.exists() {
        return true;
    }

    // ISO is rebuilt if any component is newer
    cache::is_newer(&squashfs, &iso)
        || cache::is_newer(&initramfs, &iso)
        || cache::is_newer(&vmlinuz, &iso)
}

/// Cache the kconfig hash after a successful kernel build.
pub fn cache_kconfig_hash(base_dir: &Path) {
    if let Ok(hash) = cache::hash_file(&base_dir.join("kconfig")) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.kconfig.hash"), &hash);
    }
}

/// Cache the squashfs input hash after a successful build.
pub fn cache_squashfs_hash(base_dir: &Path) {
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");
    let definitions = base_dir.join("src/component/definitions.rs");
    let inputs: Vec<&Path> = vec![&rootfs_marker, &definitions];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.squashfs-inputs.hash"), &hash);
    }
}

/// Cache the initramfs input hash after a successful build.
pub fn cache_initramfs_hash(base_dir: &Path) {
    let init_script = base_dir.join("profile/init_tiny.template");
    let busybox = base_dir.join("downloads/busybox-static");
    let inputs: Vec<&Path> = vec![&init_script, &busybox];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.initramfs-inputs.hash"), &hash);
    }
}
