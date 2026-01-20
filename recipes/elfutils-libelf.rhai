// elfutils-libelf - ELF library
// Required by: libbpf

let name = "elfutils-libelf";
let version = "0.192";
let description = "ELF library";
let installed = false;
let deps = ["glibc", "zlib", "xz"];

fn acquire() {
    copy(`${RPM_PATH}/elfutils-libelf-0*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
