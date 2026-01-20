// gzip - GNU compression utility
// Provides: gzip, gunzip, zcat

let name = "gzip";
let version = "1.13";
let description = "GNU compression utility";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/gzip-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
