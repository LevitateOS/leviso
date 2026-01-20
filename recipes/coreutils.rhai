// coreutils - GNU Core Utilities
// Provides: ls, cat, cp, mv, rm, mkdir, chmod, chown, etc.

let name = "coreutils";
let version = "9.5";
let description = "GNU Core Utilities (ls, cat, cp, mv, rm, etc.)";
let installed = false;
let deps = ["glibc", "libselinux", "libcap", "libacl", "libattr"];

fn acquire() {
    copy(`${RPM_PATH}/coreutils-9*.rpm`);
    copy(`${RPM_PATH}/coreutils-common-9*.rpm`);
}

fn build() {
    // Pre-built RPM
}

fn install() {
    rpm_install();
}
