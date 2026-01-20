// bash - GNU Bourne-Again SHell
// Default interactive shell

let name = "bash";
let version = "5.2";
let description = "GNU Bourne-Again Shell";
let installed = false;
let deps = ["glibc", "ncurses", "readline"];

fn acquire() {
    copy(`${RPM_PATH}/bash-5*.rpm`);
}

fn build() {
    // Pre-built RPM
}

fn install() {
    rpm_install();
}
