// glibc - GNU C Library
// Foundation library for all userspace applications

let name = "glibc";
let version = "2.39";
let description = "GNU C Library - core runtime for all applications";
let installed = false;
let deps = [];

fn acquire() {
    // Copy RPMs from configured RPM_PATH (mounted Rocky ISO)
    copy(`${RPM_PATH}/glibc-2*.rpm`);
    copy(`${RPM_PATH}/glibc-common-2*.rpm`);
    copy(`${RPM_PATH}/glibc-minimal-langpack-2*.rpm`);
}

fn build() {
    // Pre-built RPMs - nothing to build
}

fn install() {
    rpm_install();
}
