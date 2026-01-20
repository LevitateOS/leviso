// xz - XZ compression utilities and library
// Provides: xz, xzcat, unxz

let name = "xz";
let version = "5.6";
let description = "XZ compression utilities";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/xz-5*.rpm`);
    copy(`${RPM_PATH}/xz-libs-5*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
