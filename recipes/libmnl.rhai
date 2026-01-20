// libmnl - Netlink library
// Required by: iproute

let name = "libmnl";
let version = "1.0";
let description = "Minimalistic Netlink library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/libmnl-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
