// pam - Pluggable Authentication Modules
// Required by: shadow-utils, util-linux, systemd

let name = "pam";
let version = "1.7";
let description = "Pluggable Authentication Modules";
let installed = false;
let deps = ["glibc", "libselinux", "audit-libs"];

fn acquire() {
    copy(`${RPM_PATH}/pam-1*.rpm`);
    copy(`${RPM_PATH}/pam-libs-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
