// e2fsprogs - Ext2/3/4 filesystem utilities
// Provides: mkfs.ext4, fsck.ext4, tune2fs

let name = "e2fsprogs";
let version = "1.47";
let description = "Ext2/3/4 filesystem utilities";
let installed = false;
let deps = ["glibc", "e2fsprogs-libs"];

fn acquire() {
    copy(`${RPM_PATH}/e2fsprogs-1*.rpm`);
    copy(`${RPM_PATH}/e2fsprogs-libs-1*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
