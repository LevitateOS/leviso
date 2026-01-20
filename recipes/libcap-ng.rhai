// libcap-ng - Next-generation capabilities library
// Required by: audit-libs

let name = "libcap-ng";
let version = "0.8";
let description = "Next-generation POSIX capabilities library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/libcap-ng-0*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
