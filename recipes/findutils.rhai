// findutils - File finding utilities
// Provides: find, xargs, locate

let name = "findutils";
let version = "4.10";
let description = "File finding utilities";
let installed = false;
let deps = ["glibc", "libselinux"];

fn acquire() {
    copy(`${RPM_PATH}/findutils-4*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
