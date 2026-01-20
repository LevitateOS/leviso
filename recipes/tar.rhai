// tar - Archive utility
// Essential for extracting tarballs

let name = "tar";
let version = "1.35";
let description = "Archive utility";
let installed = false;
let deps = ["glibc", "libselinux", "libacl"];

fn acquire() {
    copy(`${RPM_PATH}/tar-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
