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
    bins, copy_file, copy_tree, custom, dirs, enable_getty, enable_multi_user,
    enable_sysinit, group, sbins, symlink, units, user, write_file, Component, CustomOp, Op,
    Phase,
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
    "printenv", "whoami", "groups", "dmesg", "lsusb",
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
    // === CHECKSUMS ===
    "base64", "md5sum", "sha256sum", "sha512sum",
    // === TERMINAL MULTIPLEXER ===
    "tmux",
    // === NETWORK DIAGNOSTICS ===
    "dig", "nslookup", "tracepath",
    // === WIRELESS ===
    "iwctl",          // iwd WiFi client
    // === BINARY INSPECTION ===
    "strings", "hexdump",
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
    "reboot", "shutdown", "poweroff", "halt", "efibootmgr",
    // === OTHER ===
    "ldconfig", "hwclock", "lspci", "ifconfig", "route",
    "agetty", "login", "sulogin", "nologin", "chronyd",
    // === SQUASHFS-TOOLS ===
    "unsquashfs",
    // === CRYPTSETUP (LUKS) ===
    "cryptsetup",
    // === LVM ===
    "lvm",
    // === HARDWARE DETECTION ===
    "dmidecode", "ethtool",
    // === XFS ===
    "mkfs.xfs", "xfs_repair",
    // === BTRFS ===
    "mkfs.btrfs", "btrfs", "btrfsck",
    // === DISK HEALTH ===
    "smartctl", "hdparm", "nvme",
];

/// Authentication binaries for /usr/sbin.
/// unix_chkpwd is CRITICAL - pam_unix.so has hardcoded path to /usr/sbin/unix_chkpwd
/// Without it, chpasswd/passwd silently fail (PAM returns success but password unchanged)
const AUTH_SBIN: &[&str] = &["visudo", "unix_chkpwd"];

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
        bins(BIN_UTILS),
        bins(AUTH_BIN),
    ],
};

pub static SBIN_BINARIES: Component = Component {
    name: "sbin",
    phase: Phase::Binaries,
    ops: &[
        sbins(SBIN_UTILS),
        sbins(AUTH_SBIN),
        // CRITICAL: agetty defaults to /bin/login (via /usr/bin/login), but login is in /usr/sbin.
        // Without this symlink, agetty can't find login and user authentication fails silently.
        // See TEAM_108_review-of-107-and-login-architecture.md for root cause analysis.
        symlink("usr/bin/login", "../sbin/login"),
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

/// Serial getty configuration for virtual serial consoles.
/// Fixes two issues that prevent login prompt from appearing:
/// 1. TERM unset - causes agetty to send terminal queries that may hang
/// 2. Missing -L - without CLOCAL, agetty waits for modem carrier detect (DCD)
///    which QEMU's virtual serial port doesn't properly emulate
/// KNOWLEDGE: See .teams/KNOWLEDGE_login-prompt-debugging.md for root cause.
const SERIAL_GETTY_CONF: &str = "\
[Service]
# Override ExecStart for virtual serial consoles.
# -L: local line (no carrier detect wait) - required for QEMU/virtual serial
# --keep-baud: try multiple baud rates for compatibility
# linux: terminal type that works well with most serial consoles
ExecStart=
ExecStart=-/sbin/agetty -L --keep-baud 115200,57600,38400,9600 %I linux
";

pub static GETTY: Component = Component {
    name: "getty",
    phase: Phase::Systemd,
    ops: &[
        enable_getty("getty@tty1.service"),
        enable_multi_user("getty.target"),
        symlink("etc/systemd/system/default.target", "/usr/lib/systemd/system/multi-user.target"),
        // Serial console fix: add -L flag for virtual serial ports (CLOCAL - ignore modem control)
        // Without this, agetty waits for carrier detect that QEMU doesn't provide
        dirs(&["etc/systemd/system/serial-getty@.service.d"]),
        write_file(
            "etc/systemd/system/serial-getty@.service.d/local.conf",
            SERIAL_GETTY_CONF,
        ),
    ],
};

// =============================================================================
// EFIVARS - EFI Variable Filesystem Support
// =============================================================================
//
// This subsystem handles mounting the efivarfs filesystem which is required
// for efibootmgr to write UEFI boot entries. The mounting is attempted in
// two places for redundancy:
//
// 1. Initramfs (leviso/profile/init_tiny.template):
//    - Mounts efivarfs after /sys is moved to /newroot/sys
//    - This is the primary mount mechanism for live boot
//
// 2. systemd mount unit (below):
//    - Backup mechanism if initramfs mount fails
//    - Also handles installed systems where initramfs doesn't run
//
// STATUS: Under investigation - see .teams/TEAM_114_efivarfs-mount-investigation.md
// The mount is not working as expected on QEMU live boot.
//
// Safe on all systems:
// - BIOS: ConditionPathExists fails, unit skipped
// - UEFI + kernel auto-mount: systemd sees existing mount, no action
// - UEFI + no mount: unit mounts it, efibootmgr works

/// EFI Variable Filesystem mount unit.
/// Required for efibootmgr to write boot entries on UEFI systems.
/// Options=rw is REQUIRED - systemd may mount read-only by default.
const EFIVARFS_MOUNT: &str = "\
[Unit]
Description=EFI Variable Filesystem
ConditionPathExists=/sys/firmware/efi
DefaultDependencies=no
Before=sysinit.target
After=local-fs-pre.target

[Mount]
What=efivarfs
Where=/sys/firmware/efi/efivars
Type=efivarfs
Options=rw

[Install]
WantedBy=sysinit.target
";

pub static EFIVARS: Component = Component {
    name: "efivars",
    phase: Phase::Systemd,
    ops: &[
        write_file(
            "usr/lib/systemd/system/sys-firmware-efi-efivars.mount",
            EFIVARFS_MOUNT,
        ),
        enable_sysinit("sys-firmware-efi-efivars.mount"),
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
        // Service must be enabled - socket-activation alone can race on boot
        enable_sysinit("systemd-udevd.service"),
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
// See DBUS_SVC in the Service-based definitions section below.

// =============================================================================
// Phase 5: Services
// =============================================================================

// --- Network ---

/// NetworkManager required binaries.
const NM_REQUIRED: &[&str] = &["nmcli", "nm-online", "nmtui"];
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
        sbins(NM_SBIN),
        bins(NM_REQUIRED),
        // wpa_supplicant
        sbins(WPA_SBIN),
        // Helpers and plugins
        copy_tree("usr/libexec"),
        copy_tree("usr/lib64/NetworkManager"),
        // Configs
        copy_tree("etc/NetworkManager"),
        copy_tree("etc/wpa_supplicant"),
        // D-Bus policies (REQUIRED)
        copy_file("usr/share/dbus-1/system.d/org.freedesktop.NetworkManager.conf"),
        copy_file("etc/dbus-1/system.d/wpa_supplicant.conf"),
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
// See CHRONY_SVC in the Service-based definitions section below.

// --- OpenSSH ---
// See OPENSSH_SVC in the Service-based definitions section below.

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
        // Pre-generate SSH host keys so sshd starts immediately
        custom(CustomOp::CreateSshHostKeys),
        // Terminal support (required for tmux, levitate-docs, ncurses apps)
        copy_tree("usr/share/terminfo"),
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
        custom(CustomOp::CopyDocsTui),
    ],
};

// =============================================================================
// Service-based definitions (higher-level abstraction)
// =============================================================================
//
// These use the Service struct which provides a more ergonomic API for
// defining services with binaries, units, configs, and users/groups.

use super::{Group, Service, Symlink, Target, User};

/// OpenSSH server and client.
pub static OPENSSH_SVC: Service = Service {
    name: "openssh",
    phase: Phase::Services,
    bins: &["ssh", "scp", "sftp", "ssh-keygen", "ssh-add", "ssh-agent"],
    sbins: &["sshd"],
    units: &[
        "sshd.service",
        "sshd.socket",
        "sshd@.service",
        "sshd-keygen.target",
        "sshd-keygen@.service",
        "ssh-host-keys-migration.service",  // Required by sshd.service Wants=
    ],
    enable: &[],  // Not enabled by default
    config_trees: &[
        "usr/libexec/openssh",
        "etc/ssh",
        "etc/crypto-policies",
        "usr/share/crypto-policies",
    ],
    config_files: &["etc/pam.d/sshd", "etc/sysconfig/sshd"],
    dirs: &["var/empty/sshd"],  // Note: /run/sshd is created by tmpfiles at boot
    symlinks: &[],
    users: &[User {
        name: "sshd",
        uid: 74,
        gid: 74,
        home: "/var/empty/sshd",
        shell: "/usr/sbin/nologin",
    }],
    groups: &[Group { name: "sshd", gid: 74 }],
    custom: &[],
};

/// Chrony NTP daemon.
pub static CHRONY_SVC: Service = Service {
    name: "chrony",
    phase: Phase::Services,
    bins: &[],
    sbins: &[],  // chronyd is already in SBIN_UTILS
    units: &[],  // Already in ESSENTIAL_UNITS
    enable: &[(Target::MultiUser, "chronyd.service")],
    config_trees: &[],
    config_files: &[
        "etc/chrony.conf",
        "etc/sysconfig/chronyd",
        "usr/lib/systemd/ntp-units.d/50-chronyd.list",
    ],
    dirs: &["var/lib/chrony", "var/run/chrony"],
    symlinks: &[],
    users: &[User {
        name: "chrony",
        uid: 992,
        gid: 987,
        home: "/var/lib/chrony",
        shell: "/sbin/nologin",
    }],
    groups: &[Group { name: "chrony", gid: 987 }],
    custom: &[],
};

/// D-Bus message bus.
pub static DBUS_SVC: Service = Service {
    name: "dbus",
    phase: Phase::Dbus,
    bins: &["dbus-broker", "dbus-broker-launch", "dbus-send", "dbus-daemon", "busctl"],
    sbins: &[],
    units: &["dbus.socket", "dbus-daemon.service"],
    enable: &[
        (Target::Sockets, "dbus.socket"),
        (Target::Sockets, "systemd-journald.socket"),
        (Target::Sockets, "systemd-journald-dev-log.socket"),
    ],
    config_trees: &["usr/share/dbus-1", "etc/dbus-1"],
    config_files: &[],
    dirs: &["run/dbus"],
    symlinks: &[Symlink {
        link: "usr/lib/systemd/system/dbus.service",
        target: "dbus-daemon.service",
    }],
    users: &[User {
        name: "dbus",
        uid: 81,
        gid: 81,
        home: "/",
        shell: "/sbin/nologin",
    }],
    groups: &[Group { name: "dbus", gid: 81 }],
    custom: &[],
};
