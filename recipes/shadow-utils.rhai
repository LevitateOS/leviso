// shadow-utils - User/group management utilities
// Provides: useradd, userdel, passwd, groupadd, etc.

let name = "shadow-utils";
let version = "4.16";
let description = "User and group management utilities";
let installed = false;
let deps = ["glibc", "libselinux", "libsemanage", "audit-libs", "pam"];

fn acquire() {
    copy(`${RPM_PATH}/shadow-utils-4*.rpm`);
}

fn build() {
    // Pre-built RPM
}

fn install() {
    rpm_install();
}
