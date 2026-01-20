// util-linux - Miscellaneous system utilities
// Provides: mount, umount, fdisk, lsblk, blkid, chroot, etc.

let name = "util-linux";
let version = "2.40";
let description = "System utilities (mount, fdisk, lsblk, etc.)";
let installed = false;
let deps = ["glibc", "libselinux", "ncurses", "readline", "pam"];

fn acquire() {
    copy(`${RPM_PATH}/util-linux-2*.rpm`);
    copy(`${RPM_PATH}/util-linux-core-2*.rpm`);
}

fn build() {
    // Pre-built RPM
}

fn install() {
    rpm_install();
}
