// sed - Stream editor
// Essential text processing tool

let name = "sed";
let version = "4.9";
let description = "Stream editor";
let installed = false;
let deps = ["glibc", "libselinux", "libacl"];

fn acquire() {
    copy(`${RPM_PATH}/sed-4*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
