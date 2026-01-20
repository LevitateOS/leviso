// openssl - Cryptographic library
// Required by: many network tools, kmod

let name = "openssl";
let version = "3.2";
let description = "Cryptographic library";
let installed = false;
let deps = ["glibc", "zlib"];

fn acquire() {
    copy(`${RPM_PATH}/openssl-libs-3*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
