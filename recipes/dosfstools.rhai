// dosfstools - FAT filesystem utilities
// Provides: mkfs.fat, fsck.fat (needed for EFI partition)

let name = "dosfstools";
let version = "4.2";
let description = "FAT filesystem utilities";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/dosfstools-4*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
