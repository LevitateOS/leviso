// pcre2 - Perl-compatible regular expressions
// Required by: grep, libselinux

let name = "pcre2";
let version = "10.44";
let description = "Perl-compatible regular expression library";
let installed = false;
let deps = ["glibc"];

fn acquire() {
    copy(`${RPM_PATH}/pcre2-10*.rpm`);
}

fn build() {}

fn install() {
    rpm_install();
}
