// libbpf - BPF library
// Required by: iproute

let name = "libbpf";
let version = "1.4";
let description = "BPF library for network tools";
let installed = false;
let deps = ["glibc", "elfutils-libelf", "zlib"];

fn acquire() {
    copy(`${RPM_PATH}/libbpf-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
