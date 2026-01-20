// libselinux - SELinux runtime library
// Required by: coreutils, systemd, shadow-utils, util-linux

let name = "libselinux";
let version = "3.7";
let description = "SELinux runtime library";
let installed = false;
let deps = ["glibc", "pcre2"];

fn acquire() {
    copy(`${RPM_PATH}/libselinux-3*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
