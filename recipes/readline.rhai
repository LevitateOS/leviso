// readline - Line editing library
// Required by: bash, util-linux

let name = "readline";
let version = "8.2";
let description = "Line editing library for interactive shells";
let installed = false;
let deps = ["glibc", "ncurses"];

fn acquire() {
    copy(`${RPM_PATH}/readline-8*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
