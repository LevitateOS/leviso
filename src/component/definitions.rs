//! Component definitions - declarative data for all system components.
//!
//! This file contains all the static component definitions. Each component
//! describes what needs to be installed, not how to install it.
//!
//! # Organization
//!
//! Components are organized by phase:
//! 1. Filesystem - FHS structure and symlinks
//! 2. Binaries - bash, coreutils, sbin utilities
//! 3. Systemd - units, getty, udev
//! 4. D-Bus - message bus
//! 5. Services - network, chrony, ssh, pam
//! 6. Config - /etc files
//! 7. Packages - recipe, dracut
//! 8. Firmware - WiFi, keymaps
//! 9. Final - welcome message, recstrap

use super::{
    bin_optional, bins_required, copy_file_optional, copy_file_required, copy_tree, custom, dir,
    dirs, enable_getty, enable_multi_user, enable_sockets, enable_sysinit, group, sbins_required,
    symlink, units, user, Component, CustomOp, Op, Phase,
};

// =============================================================================
// Phase 1: Filesystem
// =============================================================================

/// FHS directory structure.
const FHS_DIRS: &[&str] = &[
    // /usr hierarchy (merged)
    "usr/bin",
    "usr/sbin",
    "usr/lib",
    "usr/lib64",
    "usr/share",
    "usr/share/man",
    "usr/share/doc",
    "usr/share/licenses",
    "usr/share/zoneinfo",
    "usr/local/bin",
    "usr/local/sbin",
    "usr/local/lib",
    "usr/local/share",
    // /etc configuration
    "etc",
    "etc/systemd/system",
    "etc/pam.d",
    "etc/security",
    "etc/profile.d",
    // XDG Base Directory spec
    "etc/xdg",
    "etc/xdg/autostart",
    // User skeleton with XDG structure
    "etc/skel",
    "etc/skel/.config",
    "etc/skel/.local",
    "etc/skel/.local/share",
    "etc/skel/.local/state",
    "etc/skel/.cache",
    // Volatile directories
    "proc",
    "sys",
    "dev",
    "dev/pts",
    "dev/shm",
    "run",
    "run/lock",
    "tmp",
    // Persistent data
    "var",
    "var/log",
    "var/log/journal",
    "var/tmp",
    "var/cache",
    "var/lib",
    "var/spool",
    // Mount points
    "mnt",
    "media",
    // User directories
    "root",
    "home",
    // Optional
    "opt",
    "srv",
    // Systemd
    "usr/lib/systemd/system",
    "usr/lib/systemd/system-generators",
    "usr/lib64/systemd",
    // Modules
    "usr/lib/modules",
    // PAM
    "usr/lib64/security",
    // D-Bus
    "usr/share/dbus-1/system.d",
    "usr/share/dbus-1/system-services",
    // Locale
    "usr/lib/locale",
];

pub static FILESYSTEM: Component = Component {
    name: "filesystem",
    phase: Phase::Filesystem,
    ops: &[
        dirs(FHS_DIRS),
        custom(CustomOp::CreateFhsSymlinks),
    ],
};

// =============================================================================
// Phase 2: Binaries
// =============================================================================

/// Binaries for /usr/bin.
const BIN_UTILS: &[&str] = &[
    // === COREUTILS ===
    "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
    "chmod", "chown", "chgrp", "ln", "readlink", "realpath",
    "stat", "file", "mknod", "mkfifo",
    "timeout", "sleep", "true", "false", "test", "[",
    // Text processing
    "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
    "sed", "awk", "gawk", "printf", "uniq", "seq",
    // Search
    "grep", "find", "xargs",
    // System info
    "pwd", "uname", "date", "env", "id", "hostname",
    "printenv", "whoami", "groups", "dmesg",
    // Process control
    "kill", "nice", "nohup", "setsid",
    // Compression
    "gzip", "gunzip", "xz", "unxz", "tar", "bzip2", "bunzip2", "cpio",
    // Shell utilities
    "expr", "yes", "mktemp",
    // Disk info
    "df", "du", "sync", "mount", "umount", "lsblk", "findmnt", "flock",
    // Path utilities
    "dirname", "basename",
    // Other
    "which",
    // === DIFFUTILS ===
    "diff", "cmp",
    // === PROCPS-NG ===
    "ps", "pgrep", "pkill", "top", "free", "uptime", "w", "vmstat", "watch",
    // === SYSTEMD ===
    "systemctl", "journalctl", "timedatectl", "hostnamectl", "localectl", "loginctl", "bootctl",
    // === EDITORS ===
    "vi", "nano",
    // === NETWORK ===
    "ping", "curl", "wget",
    // === TERMINAL ===
    "clear", "stty", "tty",
    // === KEYBOARD ===
    "loadkeys",
    // === LOCALE ===
    "localedef",
    // === UDEV ===
    "udevadm",
    // === MISC ===
    "less", "more",
    // === UTIL-LINUX ===
    "getopt",
    // === DRACUT ===
    "dracut",
    // === GLIBC UTILITIES ===
    "getent", "ldd",
];

/// Authentication binaries for /usr/bin.
const AUTH_BIN: &[&str] = &["su", "sudo", "sudoedit", "sudoreplay"];

/// Binaries for /usr/sbin.
const SBIN_UTILS: &[&str] = &[
    // === UTIL-LINUX ===
    "fsck", "blkid", "losetup", "mkswap", "swapon", "swapoff",
    "fdisk", "sfdisk", "wipefs", "blockdev", "pivot_root", "chroot",
    "switch_root", "parted",
    // === E2FSPROGS ===
    "fsck.ext4", "fsck.ext2", "fsck.ext3", "e2fsck", "mke2fs",
    "mkfs.ext4", "mkfs.ext2", "mkfs.ext3", "tune2fs", "resize2fs",
    // === DOSFSTOOLS ===
    "mkfs.fat", "mkfs.vfat", "fsck.fat", "fsck.vfat",
    // === KMOD ===
    "insmod", "rmmod", "modprobe", "lsmod", "depmod", "modinfo",
    // === SHADOW-UTILS ===
    "useradd", "userdel", "usermod", "groupadd", "groupdel", "groupmod",
    "chpasswd", "passwd",
    // === IPROUTE ===
    "ip", "ss", "bridge",
    // === PROCPS-NG ===
    "sysctl",
    // === SYSTEM CONTROL ===
    "reboot", "shutdown", "poweroff", "halt",
    // === OTHER ===
    "ldconfig", "hwclock", "lspci", "ifconfig", "route",
    "agetty", "login", "sulogin", "nologin", "chronyd",
    // === SQUASHFS-TOOLS ===
    "unsquashfs",
];

/// Authentication binaries for /usr/sbin.
const AUTH_SBIN: &[&str] = &["visudo"];

/// Systemd helper binaries.
const SYSTEMD_BINARIES: &[&str] = &[
    "systemd-executor",
    "systemd-shutdown",
    "systemd-sulogin-shell",
    "systemd-cgroups-agent",
    "systemd-journald",
    "systemd-modules-load",
    "systemd-sysctl",
    "systemd-tmpfiles",
    "systemd-timedated",
    "systemd-hostnamed",
    "systemd-localed",
    "systemd-logind",
    "systemd-networkd",
    "systemd-resolved",
    "systemd-udevd",
    "systemd-fsck",
    "systemd-remount-fs",
    "systemd-vconsole-setup",
    "systemd-random-seed",
];

/// Sudo support libraries.
const SUDO_LIBS: &[&str] = &[
    "libsudo_util.so.0.0.0",
    "libsudo_util.so.0",
    "libsudo_util.so",
    "sudoers.so",
    "group_file.so",
    "system_group.so",
];

pub static SHELL: Component = Component {
    name: "shell",
    phase: Phase::Binaries,
    ops: &[Op::Bash],
};

pub static COREUTILS: Component = Component {
    name: "coreutils",
    phase: Phase::Binaries,
    ops: &[
        bins_required(BIN_UTILS),
        bins_required(AUTH_BIN),
    ],
};

pub static SBIN_BINARIES: Component = Component {
    name: "sbin",
    phase: Phase::Binaries,
    ops: &[
        sbins_required(SBIN_UTILS),
        sbins_required(AUTH_SBIN),
    ],
};

pub static SYSTEMD_BINS: Component = Component {
    name: "systemd-binaries",
    phase: Phase::Binaries,
    ops: &[
        Op::SystemdBinaries(SYSTEMD_BINARIES),
        symlink("usr/sbin/init", "/usr/lib/systemd/systemd"),
        Op::SudoLibs(SUDO_LIBS),
    ],
};

// =============================================================================
// Phase 3: Systemd
// =============================================================================

/// Essential systemd unit files.
const ESSENTIAL_UNITS: &[&str] = &[
    // Targets
    "basic.target", "sysinit.target", "multi-user.target", "default.target",
    "getty.target", "local-fs.target", "local-fs-pre.target",
    "remote-fs.target", "remote-fs-pre.target",
    "network.target", "network-pre.target", "network-online.target",
    "paths.target", "slices.target", "sockets.target", "timers.target",
    "swap.target", "shutdown.target", "rescue.target", "emergency.target",
    "reboot.target", "poweroff.target", "halt.target",
    "suspend.target", "sleep.target", "umount.target", "final.target",
    "graphical.target",
    // Services - core
    "systemd-journald.service", "systemd-journald@.service",
    "systemd-udevd.service", "systemd-udev-trigger.service",
    "systemd-modules-load.service", "systemd-sysctl.service",
    "systemd-tmpfiles-setup.service", "systemd-tmpfiles-setup-dev.service",
    "systemd-tmpfiles-clean.service",
    "systemd-random-seed.service", "systemd-vconsole-setup.service",
    // Services - disk
    "systemd-fsck-root.service", "systemd-fsck@.service",
    "systemd-remount-fs.service", "systemd-fstab-generator",
    // Services - auth
    "systemd-logind.service",
    // Services - getty
    "getty@.service", "serial-getty@.service",
    "console-getty.service", "container-getty@.service",
    // Services - time/network
    "systemd-timedated.service", "systemd-hostnamed.service",
    "systemd-localed.service", "systemd-networkd.service",
    "systemd-resolved.service", "systemd-networkd-wait-online.service",
    // Services - misc
    "dbus.service", "dbus-broker.service", "chronyd.service",
    // Services - SSH
    "sshd.service", "sshd@.service", "sshd.socket",
    "sshd-keygen.target", "sshd-keygen@.service",
    // Sockets
    "systemd-journald.socket", "systemd-journald-dev-log.socket",
    "systemd-journald-audit.socket",
    "systemd-udevd-control.socket", "systemd-udevd-kernel.socket",
    "dbus.socket",
    // Paths
    "systemd-ask-password-console.path", "systemd-ask-password-wall.path",
    // Slices
    "-.slice", "system.slice", "user.slice", "machine.slice",
];

/// D-Bus activation symlinks.
const DBUS_ACTIVATION_SYMLINKS: &[&str] = &[
    "dbus-org.freedesktop.timedate1.service",
    "dbus-org.freedesktop.hostname1.service",
    "dbus-org.freedesktop.locale1.service",
    "dbus-org.freedesktop.login1.service",
    "dbus-org.freedesktop.network1.service",
    "dbus-org.freedesktop.resolve1.service",
];

/// Udev helper binaries.
const UDEV_HELPERS: &[&str] = &[
    "ata_id", "scsi_id", "cdrom_id", "v4l_id", "dmi_memory_id", "mtd_probe",
];

pub static SYSTEMD_UNITS: Component = Component {
    name: "systemd-units",
    phase: Phase::Systemd,
    ops: &[
        units(ESSENTIAL_UNITS),
        Op::DbusSymlinks(DBUS_ACTIVATION_SYMLINKS),
    ],
};

pub static GETTY: Component = Component {
    name: "getty",
    phase: Phase::Systemd,
    ops: &[
        enable_getty("getty@tty1.service"),
        enable_multi_user("getty.target"),
        symlink("etc/systemd/system/default.target", "/usr/lib/systemd/system/multi-user.target"),
    ],
};

pub static UDEV: Component = Component {
    name: "udev",
    phase: Phase::Systemd,
    ops: &[
        copy_tree("usr/lib/udev/rules.d"),
        copy_tree("usr/lib/udev/hwdb.d"),
        Op::UdevHelpers(UDEV_HELPERS),
        enable_sysinit("systemd-udevd-control.socket"),
        enable_sysinit("systemd-udevd-kernel.socket"),
        enable_sysinit("systemd-udev-trigger.service"),
    ],
};

pub static TMPFILES: Component = Component {
    name: "tmpfiles",
    phase: Phase::Systemd,
    ops: &[
        copy_tree("usr/lib/tmpfiles.d"),
        copy_tree("usr/lib/sysctl.d"),
    ],
};

pub static LIVE_SYSTEMD: Component = Component {
    name: "live-systemd",
    phase: Phase::Systemd,
    ops: &[
        custom(CustomOp::SetupLiveSystemdConfigs),
    ],
};

// =============================================================================
// Phase 4: D-Bus
// =============================================================================

/// D-Bus binaries.
const DBUS_BINARIES: &[&str] = &[
    "dbus-broker",
    "dbus-broker-launch",
    "dbus-send",
    "dbus-daemon",
    "busctl",
];

/// D-Bus systemd units.
const DBUS_UNITS: &[&str] = &["dbus.socket", "dbus-daemon.service"];

pub static DBUS: Component = Component {
    name: "dbus",
    phase: Phase::Dbus,
    ops: &[
        dir("run/dbus"),
        bins_required(DBUS_BINARIES),
        copy_tree("usr/share/dbus-1"),
        copy_tree("etc/dbus-1"),
        units(DBUS_UNITS),
        symlink("usr/lib/systemd/system/dbus.service", "dbus-daemon.service"),
        enable_sockets("dbus.socket"),
        enable_sockets("systemd-journald.socket"),
        enable_sockets("systemd-journald-dev-log.socket"),
        user("dbus", 81, 81, "/", "/sbin/nologin"),
        group("dbus", 81),
    ],
};

// =============================================================================
// Phase 5: Services
// =============================================================================

// --- Network ---

/// NetworkManager required binaries.
const NM_REQUIRED: &[&str] = &["nmcli", "nm-online"];
/// NetworkManager sbin binaries.
const NM_SBIN: &[&str] = &["NetworkManager"];
/// wpa_supplicant binaries.
const WPA_SBIN: &[&str] = &["wpa_supplicant", "wpa_cli", "wpa_passphrase"];
/// NetworkManager units.
const NM_UNITS: &[&str] = &["NetworkManager.service", "NetworkManager-dispatcher.service"];
/// wpa_supplicant units.
const WPA_UNITS: &[&str] = &["wpa_supplicant.service"];

pub static NETWORK: Component = Component {
    name: "network",
    phase: Phase::Services,
    ops: &[
        // NetworkManager
        sbins_required(NM_SBIN),
        bins_required(NM_REQUIRED),
        bin_optional("nmtui"),
        // wpa_supplicant
        sbins_required(WPA_SBIN),
        // Helpers and plugins
        copy_tree("usr/libexec"),
        copy_tree("usr/lib64/NetworkManager"),
        // Configs
        copy_tree("etc/NetworkManager"),
        copy_tree("etc/wpa_supplicant"),
        // D-Bus policies (REQUIRED)
        copy_file_required("usr/share/dbus-1/system.d/org.freedesktop.NetworkManager.conf"),
        copy_file_required("etc/dbus-1/system.d/wpa_supplicant.conf"),
        // Units
        units(NM_UNITS),
        units(WPA_UNITS),
        enable_multi_user("NetworkManager.service"),
        // WiFi firmware (custom - complex logic)
        custom(CustomOp::CopyWifiFirmware),
        // Optional VPN user
        user("nm-openconnect", 993, 988, "/", "/sbin/nologin"),
        group("nm-openconnect", 988),
    ],
};

// --- Chrony ---

pub static CHRONY: Component = Component {
    name: "chrony",
    phase: Phase::Services,
    ops: &[
        dir("var/lib/chrony"),
        dir("var/run/chrony"),
        copy_file_optional("etc/chrony.conf"),
        copy_file_optional("etc/sysconfig/chronyd"),
        copy_file_optional("usr/lib/systemd/ntp-units.d/50-chronyd.list"),
        enable_multi_user("chronyd.service"),
        user("chrony", 992, 987, "/var/lib/chrony", "/sbin/nologin"),
        group("chrony", 987),
    ],
};

// --- OpenSSH ---

/// SSH server binaries.
const SSH_SERVER: &[&str] = &["sshd"];
/// SSH client binaries.
const SSH_CLIENT: &[&str] = &["ssh", "scp", "sftp", "ssh-keygen", "ssh-add", "ssh-agent"];
/// SSH units.
const SSH_UNITS: &[&str] = &[
    "sshd.service",
    "sshd.socket",
    "sshd@.service",
    "sshd-keygen.target",
    "sshd-keygen@.service",
];

pub static OPENSSH: Component = Component {
    name: "openssh",
    phase: Phase::Services,
    ops: &[
        sbins_required(SSH_SERVER),
        bins_required(SSH_CLIENT),
        copy_tree("usr/libexec/openssh"),
        copy_tree("etc/ssh"),
        copy_file_required("etc/pam.d/sshd"),
        units(SSH_UNITS),
        copy_file_optional("etc/sysconfig/sshd"),
        copy_tree("etc/crypto-policies"),
        copy_tree("usr/share/crypto-policies"),
        dir("var/empty/sshd"),
        dir("run/sshd"),
        user("sshd", 74, 74, "/var/empty/sshd", "/usr/sbin/nologin"),
        group("sshd", 74),
    ],
};

// --- PAM ---

pub static PAM: Component = Component {
    name: "pam",
    phase: Phase::Services,
    ops: &[
        copy_tree("usr/lib64/security"),
        custom(CustomOp::CreatePamFiles),
        custom(CustomOp::CreateSecurityConfig),
    ],
};

// --- Kernel Modules ---

pub static MODULES: Component = Component {
    name: "modules",
    phase: Phase::Services,
    ops: &[
        custom(CustomOp::CopyModules),
    ],
};

// =============================================================================
// Phase 6: Config
// =============================================================================

pub static ETC_CONFIG: Component = Component {
    name: "etc",
    phase: Phase::Config,
    ops: &[
        custom(CustomOp::CreateEtcFiles),
        custom(CustomOp::CopyTimezoneData),
        custom(CustomOp::CopyLocales),
        custom(CustomOp::DisableSelinux),
    ],
};

// =============================================================================
// Phase 7: Packages
// =============================================================================

pub static RECIPE: Component = Component {
    name: "recipe",
    phase: Phase::Packages,
    ops: &[
        custom(CustomOp::CopyRecipe),
        custom(CustomOp::SetupRecipeConfig),
    ],
};

pub static DRACUT: Component = Component {
    name: "dracut",
    phase: Phase::Packages,
    ops: &[
        custom(CustomOp::CopyDracutModules),
        custom(CustomOp::CreateDracutConfig),
    ],
};

pub static BOOTLOADER: Component = Component {
    name: "bootloader",
    phase: Phase::Packages,
    ops: &[
        custom(CustomOp::CopySystemdBootEfi),
    ],
};

// =============================================================================
// Phase 8: Firmware
// =============================================================================

pub static FIRMWARE: Component = Component {
    name: "firmware",
    phase: Phase::Firmware,
    ops: &[
        custom(CustomOp::CopyAllFirmware),
        custom(CustomOp::CopyKeymaps),
    ],
};

// =============================================================================
// Phase 9: Final
// =============================================================================

pub static FINAL: Component = Component {
    name: "final",
    phase: Phase::Final,
    ops: &[
        custom(CustomOp::CreateWelcomeMessage),
        custom(CustomOp::CopyRecstrap),
    ],
};
