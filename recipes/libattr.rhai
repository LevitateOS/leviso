// libattr - Extended attributes library
// Required by: libacl

let name = "libattr";
let version = "2.5";
let description = "Extended filesystem attributes library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/libattr-2*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
