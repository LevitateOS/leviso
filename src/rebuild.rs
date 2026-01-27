//! Rebuild detection logic.
//!
//! Uses hash-based caching to skip rebuilding artifacts that haven't changed.
//! This provides faster incremental builds by detecting when inputs change.

use std::path::Path;

use distro_spec::levitate::{INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME};

use crate::cache;

/// Check if kernel needs to be compiled (bzImage).
///
/// Uses hash-based detection for kconfig and kernel source version changes.
/// Falls back to mtime comparison if hash file is missing.
pub fn kernel_needs_compile(base_dir: &Path) -> bool {
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let kconfig = base_dir.join("kconfig");
    // Also track kernel source version via Makefile (contains VERSION, PATCHLEVEL, SUBLEVEL)
    let kernel_makefile = base_dir.join("../linux/Makefile");
    let hash_file = base_dir.join("output/.kernel-inputs.hash");

    if !bzimage.exists() {
        return true;
    }

    // Hash both kconfig and kernel Makefile (for version detection)
    let inputs: Vec<&Path> = if kernel_makefile.exists() {
        vec![&kconfig, &kernel_makefile]
    } else {
        vec![&kconfig]
    };

    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &bzimage)
}

/// Check if kernel needs to be installed (vmlinuz + modules).
///
/// Returns true if bzImage exists but vmlinuz doesn't, or if bzImage is newer.
pub fn kernel_needs_install(base_dir: &Path) -> bool {
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    if !bzimage.exists() {
        return false; // Can't install what doesn't exist
    }

    if !vmlinuz.exists() {
        return true;
    }

    // Reinstall if bzImage is newer than vmlinuz
    cache::is_newer(&bzimage, &vmlinuz)
}

/// Check if rootfs (EROFS) needs to be rebuilt.
///
/// Uses hash of key input files. Falls back to mtime if hash file missing.
pub fn rootfs_needs_rebuild(base_dir: &Path) -> bool {
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
    let hash_file = base_dir.join("output/.rootfs-inputs.hash");

    if !rootfs.exists() {
        return true;
    }

    // Key files that affect rootfs content
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");
    // Track all component source files that affect rootfs content
    let definitions = base_dir.join("src/component/definitions.rs");
    let component_mod = base_dir.join("src/component/mod.rs");
    let custom_etc = base_dir.join("src/component/custom/etc.rs");
    let custom_pam = base_dir.join("src/component/custom/pam.rs");
    let custom_live = base_dir.join("src/component/custom/live.rs");
    let custom_packages = base_dir.join("src/component/custom/packages.rs");

    let inputs: Vec<&Path> = vec![
        &rootfs_marker,
        &definitions,
        &component_mod,
        &custom_etc,
        &custom_pam,
        &custom_live,
        &custom_packages,
    ];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &rootfs)
}

/// Check if initramfs needs to be rebuilt.
pub fn initramfs_needs_rebuild(base_dir: &Path) -> bool {
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
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

/// Check if install initramfs needs to be rebuilt.
///
/// This is the systemd-based initramfs copied to installed systems.
/// Uses hash of recinit source files since they determine initramfs content.
pub fn install_initramfs_needs_rebuild(base_dir: &Path) -> bool {
    let initramfs = base_dir.join("output").join(INITRAMFS_INSTALLED_OUTPUT);
    let hash_file = base_dir.join("output/.install-initramfs-inputs.hash");

    if !initramfs.exists() {
        return true;
    }

    // Track recinit source files that affect install initramfs generation
    let recinit_base = base_dir.join("../tools/recinit/src");
    let systemd_rs = recinit_base.join("systemd.rs");
    let install_rs = recinit_base.join("install.rs");
    let lib_rs = recinit_base.join("lib.rs");
    let elf_rs = recinit_base.join("elf.rs");
    let cpio_rs = recinit_base.join("cpio.rs");
    let modules_rs = recinit_base.join("modules.rs");
    // Also track rootfs marker to rebuild if rootfs changes
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");

    let inputs: Vec<&Path> = vec![
        &systemd_rs,
        &install_rs,
        &lib_rs,
        &elf_rs,
        &cpio_rs,
        &modules_rs,
        &rootfs_marker,
    ];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &initramfs)
}

/// Check if ISO needs to be rebuilt.
pub fn iso_needs_rebuild(base_dir: &Path) -> bool {
    let iso = base_dir.join("output").join(ISO_FILENAME);
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    if !iso.exists() {
        return true;
    }

    // ISO needs rebuild if any component is missing (will be built first)
    // or if any component is newer than the ISO
    !rootfs.exists()
        || !initramfs.exists()
        || !vmlinuz.exists()
        || cache::is_newer(&rootfs, &iso)
        || cache::is_newer(&initramfs, &iso)
        || cache::is_newer(&vmlinuz, &iso)
}

/// Cache the kernel input hash after a successful kernel build.
pub fn cache_kernel_hash(base_dir: &Path) {
    let kconfig = base_dir.join("kconfig");
    let kernel_makefile = base_dir.join("../linux/Makefile");
    let inputs: Vec<&Path> = if kernel_makefile.exists() {
        vec![&kconfig, &kernel_makefile]
    } else {
        vec![&kconfig]
    };
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.kernel-inputs.hash"), &hash);
    }
}

/// Cache the rootfs input hash after a successful build.
pub fn cache_rootfs_hash(base_dir: &Path) {
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");
    let definitions = base_dir.join("src/component/definitions.rs");
    let component_mod = base_dir.join("src/component/mod.rs");
    let custom_etc = base_dir.join("src/component/custom/etc.rs");
    let custom_pam = base_dir.join("src/component/custom/pam.rs");
    let custom_live = base_dir.join("src/component/custom/live.rs");
    let custom_packages = base_dir.join("src/component/custom/packages.rs");

    let inputs: Vec<&Path> = vec![
        &rootfs_marker,
        &definitions,
        &component_mod,
        &custom_etc,
        &custom_pam,
        &custom_live,
        &custom_packages,
    ];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.rootfs-inputs.hash"), &hash);
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

/// Cache the install initramfs input hash after a successful build.
pub fn cache_install_initramfs_hash(base_dir: &Path) {
    let recinit_base = base_dir.join("../tools/recinit/src");
    let systemd_rs = recinit_base.join("systemd.rs");
    let install_rs = recinit_base.join("install.rs");
    let lib_rs = recinit_base.join("lib.rs");
    let elf_rs = recinit_base.join("elf.rs");
    let cpio_rs = recinit_base.join("cpio.rs");
    let modules_rs = recinit_base.join("modules.rs");
    let rootfs_marker = base_dir.join("downloads/rootfs/usr/bin/bash");

    let inputs: Vec<&Path> = vec![
        &systemd_rs,
        &install_rs,
        &lib_rs,
        &elf_rs,
        &cpio_rs,
        &modules_rs,
        &rootfs_marker,
    ];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.install-initramfs-inputs.hash"), &hash);
    }
}
