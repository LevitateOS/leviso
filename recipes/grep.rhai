// grep - Pattern matching utility
// Provides: grep, egrep, fgrep

let name = "grep";
let version = "3.11";
let description = "Pattern matching utility";
let installed = false;
let deps = ["glibc", "pcre2"];

fn acquire() {
    copy(`${RPM_PATH}/grep-3*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
