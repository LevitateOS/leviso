// libsemanage - SELinux policy management library
// Required by: shadow-utils

let name = "libsemanage";
let version = "3.7";
let description = "SELinux policy management library";
let installed = false;
let deps = ["glibc", "libselinux"];

fn acquire() {
    copy(`${RPM_PATH}/libsemanage-3*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
