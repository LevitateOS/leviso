// libacl - Access control list library
// Required by: coreutils

let name = "libacl";
let version = "2.3";
let description = "Access control list library";
let installed = false;
let deps = ["glibc", "libattr"];

fn acquire() {
    copy(`${RPM_PATH}/libacl-2*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
