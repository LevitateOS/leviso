// ncurses - Terminal handling library
// Required by: bash, util-linux, vim, less, etc.

let name = "ncurses";
let version = "6.5";
let description = "Terminal handling library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/ncurses-libs-6*.rpm`);
    copy(`${RPM_PATH}/ncurses-base-6*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
