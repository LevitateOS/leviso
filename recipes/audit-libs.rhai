// audit-libs - Linux audit library
// Required by: shadow-utils

let name = "audit-libs";
let version = "4.0";
let description = "Linux audit library";
let installed = false;
let deps = ["glibc", "libcap-ng"];

fn acquire() {
    copy(`${RPM_PATH}/audit-libs-4*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
