// zlib - Compression library
// Required by: many packages

let name = "zlib";
let version = "1.3";
let description = "Compression library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/zlib-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
