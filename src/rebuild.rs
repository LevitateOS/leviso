//! Rebuild detection logic.
//!
//! Uses hash-based caching to skip rebuilding artifacts that haven't changed.
//! Each artifact defines its input files once, eliminating duplication between
//! needs_rebuild and cache_hash functions.

use std::path::{Path, PathBuf};

use distro_spec::levitate::{
    INITRAMFS_INSTALLED_OUTPUT, INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME,
};
use distro_spec::shared::QCOW2_IMAGE_FILENAME;

use distro_builder::cache;

/// An artifact that can be incrementally rebuilt.
pub struct Artifact {
    /// Path to the output file
    pub output: PathBuf,
    /// Path to the hash cache file
    pub hash_file: PathBuf,
    /// Input files that affect this artifact
    pub inputs: Vec<PathBuf>,
}

impl Artifact {
    /// Check if this artifact needs to be rebuilt.
    pub fn needs_rebuild(&self) -> bool {
        if !self.output.exists() {
            return true;
        }

        let input_refs: Vec<&Path> = self.inputs.iter().map(|p| p.as_path()).collect();
        let current_hash = match cache::hash_files(&input_refs) {
            Some(h) => h,
            None => return true,
        };

        cache::needs_rebuild(&current_hash, &self.hash_file, &self.output)
    }

    /// Cache the input hash after a successful build.
    pub fn cache_hash(&self) {
        let input_refs: Vec<&Path> = self.inputs.iter().map(|p| p.as_path()).collect();
        if let Some(hash) = cache::hash_files(&input_refs) {
            let _ = cache::write_cached_hash(&self.hash_file, &hash);
        }
    }
}

// ============================================================================
// Artifact definitions
// ============================================================================

/// Kernel compilation artifact (bzImage).
pub fn kernel_artifact(base_dir: &Path) -> Artifact {
    let kernel_makefile = base_dir.join("../linux/Makefile");
    let mut inputs = vec![base_dir.join("kconfig")];
    if kernel_makefile.exists() {
        inputs.push(kernel_makefile);
    }

    Artifact {
        output: base_dir.join("output/kernel-build/arch/x86/boot/bzImage"),
        hash_file: base_dir.join("output/.kernel-inputs.hash"),
        inputs,
    }
}

/// Rootfs (EROFS) artifact.
pub fn rootfs_artifact(base_dir: &Path) -> Artifact {
    let distro_spec_base = base_dir.join("../distro-spec/src/shared");

    Artifact {
        output: base_dir.join("output").join(ROOTFS_NAME),
        hash_file: base_dir.join("output/.rootfs-inputs.hash"),
        inputs: vec![
            // Rocky rootfs marker
            base_dir.join("downloads/rootfs/usr/bin/bash"),
            // Component source files
            base_dir.join("src/component/definitions.rs"),
            base_dir.join("src/component/mod.rs"),
            base_dir.join("src/component/custom/etc.rs"),
            base_dir.join("src/component/custom/pam.rs"),
            base_dir.join("src/component/custom/live.rs"),
            base_dir.join("src/component/custom/packages.rs"),
            base_dir.join("src/component/custom/filesystem.rs"),
            base_dir.join("src/component/custom/firmware.rs"),
            base_dir.join("src/component/custom/modules.rs"),
            // Build logic
            base_dir.join("src/build/licenses.rs"),
            base_dir.join("src/build/libdeps.rs"),
            // distro-spec definitions
            distro_spec_base.join("licenses.rs"),
            distro_spec_base.join("components.rs"),
            distro_spec_base.join("services.rs"),
            // Profile files - auth
            base_dir.join("profile/etc/shadow"),
            base_dir.join("profile/etc/passwd"),
            base_dir.join("profile/etc/group"),
            base_dir.join("profile/etc/gshadow"),
            base_dir.join("profile/etc/sudoers"),
            base_dir.join("profile/etc/motd"),
            // PAM files
            base_dir.join("profile/etc/pam.d/system-auth"),
            base_dir.join("profile/etc/pam.d/login"),
            base_dir.join("profile/etc/pam.d/sshd"),
            base_dir.join("profile/etc/pam.d/sudo"),
            base_dir.join("profile/etc/pam.d/su"),
            base_dir.join("profile/etc/pam.d/passwd"),
            base_dir.join("profile/etc/pam.d/chpasswd"),
            base_dir.join("profile/etc/security/limits.conf"),
            // Recipe config
            base_dir.join("profile/etc/recipe.conf"),
            base_dir.join("profile/etc/profile.d/recipe.sh"),
        ],
    }
}

/// Live initramfs artifact (tiny busybox-based).
pub fn initramfs_artifact(base_dir: &Path) -> Artifact {
    Artifact {
        output: base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT),
        hash_file: base_dir.join("output/.initramfs-inputs.hash"),
        inputs: vec![
            base_dir.join("profile/init_tiny.template"),
            base_dir.join("downloads/busybox-static"),
        ],
    }
}

/// Install initramfs artifact (systemd-based, copied to installed systems).
pub fn install_initramfs_artifact(base_dir: &Path) -> Artifact {
    let recinit_base = base_dir.join("../tools/recinit/src");
    let distro_spec_base = base_dir.join("../distro-spec/src/shared");

    Artifact {
        output: base_dir.join("output").join(INITRAMFS_INSTALLED_OUTPUT),
        hash_file: base_dir.join("output/.install-initramfs-inputs.hash"),
        inputs: vec![
            // recinit source files
            recinit_base.join("systemd.rs"),
            recinit_base.join("install.rs"),
            recinit_base.join("lib.rs"),
            recinit_base.join("elf.rs"),
            recinit_base.join("cpio.rs"),
            recinit_base.join("modules.rs"),
            // distro-spec components (ESSENTIAL_UNITS, BIN_UTILS, etc.)
            distro_spec_base.join("components.rs"),
            distro_spec_base.join("udev.rs"),
            // rootfs marker (source of binaries)
            base_dir.join("downloads/rootfs/usr/bin/bash"),
        ],
    }
}

/// qcow2 VM disk image artifact.
#[allow(dead_code)]
pub fn qcow2_artifact(base_dir: &Path) -> Artifact {
    Artifact {
        output: base_dir.join("output").join(QCOW2_IMAGE_FILENAME),
        hash_file: base_dir.join("output/.qcow2-inputs.hash"),
        inputs: vec![
            // Primary input: rootfs-staging directory marker
            base_dir.join("output/rootfs-staging/usr/bin/bash"),
            // Install initramfs (required for boot - changes trigger rebuild)
            base_dir.join("output").join(INITRAMFS_INSTALLED_OUTPUT),
            // qcow2-specific config
            base_dir.join("src/artifact/qcow2.rs"),
        ],
    }
}

// ============================================================================
// Public API (backwards compatible)
// ============================================================================

pub fn kernel_needs_compile(base_dir: &Path) -> bool {
    kernel_artifact(base_dir).needs_rebuild()
}

pub fn kernel_needs_install(base_dir: &Path) -> bool {
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    if !bzimage.exists() {
        return false;
    }
    if !vmlinuz.exists() {
        return true;
    }
    cache::is_newer(&bzimage, &vmlinuz)
}

pub fn rootfs_needs_rebuild(base_dir: &Path) -> bool {
    rootfs_artifact(base_dir).needs_rebuild()
}

pub fn initramfs_needs_rebuild(base_dir: &Path) -> bool {
    initramfs_artifact(base_dir).needs_rebuild()
}

pub fn install_initramfs_needs_rebuild(base_dir: &Path) -> bool {
    install_initramfs_artifact(base_dir).needs_rebuild()
}

pub fn iso_needs_rebuild(base_dir: &Path) -> bool {
    let iso = base_dir.join("output").join(ISO_FILENAME);
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    // Live overlay files affect ISO content
    let live_overlay = base_dir.join("profile/live-overlay");
    let live_shadow = live_overlay.join("etc/shadow");
    let live_autologin =
        live_overlay.join("etc/systemd/system/getty@tty1.service.d/autologin.conf");
    let live_serial =
        live_overlay.join("etc/systemd/system/serial-getty@.service.d/zz-autologin.conf");
    let live_docs = live_overlay.join("etc/profile.d/live-docs.sh");
    let live_test = live_overlay.join("etc/profile.d/00-levitate-test.sh");

    if !iso.exists() {
        return true;
    }

    !rootfs.exists()
        || !initramfs.exists()
        || !vmlinuz.exists()
        || cache::is_newer(&rootfs, &iso)
        || cache::is_newer(&initramfs, &iso)
        || cache::is_newer(&vmlinuz, &iso)
        || cache::is_newer(&live_shadow, &iso)
        || cache::is_newer(&live_autologin, &iso)
        || cache::is_newer(&live_serial, &iso)
        || cache::is_newer(&live_docs, &iso)
        || cache::is_newer(&live_test, &iso)
}

pub fn cache_kernel_hash(base_dir: &Path) {
    kernel_artifact(base_dir).cache_hash();
}

pub fn cache_rootfs_hash(base_dir: &Path) {
    rootfs_artifact(base_dir).cache_hash();
}

pub fn cache_initramfs_hash(base_dir: &Path) {
    initramfs_artifact(base_dir).cache_hash();
}

pub fn cache_install_initramfs_hash(base_dir: &Path) {
    install_initramfs_artifact(base_dir).cache_hash();
}

#[allow(dead_code)]
pub fn qcow2_needs_rebuild(base_dir: &Path) -> bool {
    qcow2_artifact(base_dir).needs_rebuild()
}

#[allow(dead_code)]
pub fn cache_qcow2_hash(base_dir: &Path) {
    qcow2_artifact(base_dir).cache_hash()
}
