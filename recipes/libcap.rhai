// libcap - POSIX capabilities library
// Required by: coreutils, systemd

let name = "libcap";
let version = "2.70";
let description = "POSIX capabilities library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/libcap-2*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
