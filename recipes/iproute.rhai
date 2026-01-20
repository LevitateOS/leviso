// iproute - Network configuration tools
// Provides: ip, ss, bridge

let name = "iproute";
let version = "6.11";
let description = "Network configuration tools (ip, ss)";
let installed = false;
let deps = ["glibc", "libselinux", "libbpf", "libmnl"];

fn acquire() {
    copy(`${RPM_PATH}/iproute-6*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
