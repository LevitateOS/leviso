// base - LevitateOS Base System
// Meta-package that pulls in all required base system components
//
// Usage: recipe install base --deps

let name = "base";
let version = "1.0.0";
let description = "LevitateOS base system (meta-package)";
let installed = false;

// Core dependencies - these will all be installed when running:
// recipe install base --deps
let deps = [
    // Core libraries
    "glibc",
    "ncurses",
    "readline",
    "zlib",
    "xz",
    "openssl",
    "pcre2",

    // SELinux/security
    "libselinux",
    "libsemanage",
    "libcap",
    "libcap-ng",
    "libacl",
    "libattr",
    "audit-libs",
    "pam",

    // Shell and coreutils
    "bash",
    "coreutils",

    // System management
    "systemd",
    "util-linux",
    "shadow-utils",
    "kmod",

    // Text processing
    "grep",
    "sed",
    "gawk",
    "findutils",

    // Compression
    "tar",
    "gzip",

    // Network
    "iproute",
    "libbpf",
    "elfutils-libelf",
    "libmnl",

    // Filesystem
    "e2fsprogs",
    "dosfstools",
];

fn acquire() {
    // Meta-package - nothing to acquire
    print("base: Meta-package, dependencies will be installed");
}

fn build() {
    // Nothing to build
}

fn install() {
    // Meta-package - all work done by dependencies
    print("base: All dependencies installed successfully");
    print("");
    print("LevitateOS base system is now installed.");
    print("Next steps:");
    print("  1. Configure /etc/fstab");
    print("  2. Set hostname in /etc/hostname");
    print("  3. Create user with: useradd -m -G wheel USERNAME");
    print("  4. Set passwords with: passwd USERNAME");
    print("  5. Install bootloader");
}
