// kmod - Kernel module utilities
// Provides: insmod, rmmod, modprobe, lsmod, depmod

let name = "kmod";
let version = "33";
let description = "Kernel module utilities";
let installed = false;
let deps = ["glibc", "xz-libs", "zlib", "openssl-libs"];

fn acquire() {
    copy(`${RPM_PATH}/kmod-33*.rpm`);
    copy(`${RPM_PATH}/kmod-libs-33*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
