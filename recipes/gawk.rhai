// gawk - GNU awk
// Text processing language

let name = "gawk";
let version = "5.3";
let description = "GNU awk text processing";
let installed = false;
let deps = ["glibc", "readline"];

fn acquire() {
    copy(`${RPM_PATH}/gawk-5*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
