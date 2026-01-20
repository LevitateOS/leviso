// systemd - System and Service Manager
// Init system, service management, logging, networking

let name = "systemd";
let version = "257";
let description = "System and Service Manager";
let installed = false;
let deps = ["glibc", "libselinux", "libcap", "util-linux", "kmod"];

fn acquire() {
    copy(`${RPM_PATH}/systemd-257*.rpm`);
    copy(`${RPM_PATH}/systemd-libs-257*.rpm`);
    copy(`${RPM_PATH}/systemd-pam-257*.rpm`);
    copy(`${RPM_PATH}/systemd-udev-257*.rpm`);
    copy(`${RPM_PATH}/systemd-networkd-257*.rpm`);
    copy(`${RPM_PATH}/systemd-resolved-257*.rpm`);
}

fn build() {
    // Pre-built RPM
}

fn install() {
    rpm_install();
}
