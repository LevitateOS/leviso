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
//!
//! # Single Source of Truth
//!
//! The lists of binaries, units, etc. are defined in `distro-spec/src/shared/components.rs`.
//! This file imports those lists and uses them to define Components.

use super::{
    bins, copy_file, copy_tree, custom, dirs, enable_getty, enable_multi_user,
    enable_sysinit, group, sbins, symlink, units, user, write_file, Component, CustomOp, Op,
    Phase,
};

// Import component definitions from distro-spec (SINGLE SOURCE OF TRUTH)
use distro_spec::shared::{
    FHS_DIRS, BIN_UTILS, AUTH_BIN, SBIN_UTILS, AUTH_SBIN,
    SYSTEMD_BINARIES, ESSENTIAL_UNITS, DBUS_ACTIVATION_SYMLINKS,
    UDEV_HELPERS, SUDO_LIBS, NM_BIN, NM_SBIN, WPA_SBIN, NM_UNITS, WPA_UNITS,
    // Desktop services
    BLUETOOTH_SBIN, BLUETOOTH_UNITS,
    PIPEWIRE_SBIN, PIPEWIRE_UNITS,
    POLKIT_SBIN, POLKIT_UNITS,
    UDISKS_SBIN, UDISKS_UNITS,
    UPOWER_SBIN, UPOWER_UNITS,
};

// =============================================================================
// Phase 1: Filesystem
// =============================================================================

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

pub static SYSTEMD_UNITS: Component = Component {
    name: "systemd-units",
    phase: Phase::Systemd,
    ops: &[
        units(ESSENTIAL_UNITS),
        Op::DbusSymlinks(DBUS_ACTIVATION_SYMLINKS),
    ],
};

/// Serial getty configuration for virtual serial consoles.
/// Fixes issues that prevent login prompt from appearing in QEMU:
/// 1. Missing -L - without CLOCAL, agetty waits for modem carrier detect (DCD)
///    which QEMU's virtual serial port doesn't properly emulate
/// 2. TERM=linux sends cursor position queries ([6n) that require terminal response.
///    When using piped I/O (test harness), nothing responds, causing agetty to hang.
///    Using TERM=vt100 avoids these queries while preserving basic functionality.
/// KNOWLEDGE: See .teams/KNOWLEDGE_login-prompt-debugging.md for root cause.
const SERIAL_GETTY_CONF: &str = "\
[Service]
# Override ExecStart for virtual serial consoles.
# -L: local line (no carrier detect wait) - required for QEMU/virtual serial
# --keep-baud: try multiple baud rates for compatibility
# vt100: simpler terminal type that doesn't send cursor position queries
#        (linux terminal sends [6n which hangs when no terminal responds)
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

pub static NETWORK: Component = Component {
    name: "network",
    phase: Phase::Services,
    ops: &[
        // NetworkManager
        sbins(NM_SBIN),
        bins(NM_BIN),
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

// DRACUT component removed - initramfs is now built using custom builder.
// See TEAM_125 and .teams/KNOWLEDGE_no-dracut.md for details.

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
        custom(CustomOp::InstallTools),
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
    user_units: &[],
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
    user_units: &[],
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
    user_units: &[],
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

// =============================================================================
// Desktop Services (Phase 5)
// =============================================================================

/// Bluetooth support (bluez).
pub static BLUETOOTH_SVC: Service = Service {
    name: "bluetooth",
    phase: Phase::Services,
    bins: &["bluetoothctl"],
    sbins: BLUETOOTH_SBIN,
    units: BLUETOOTH_UNITS,
    user_units: &[],
    enable: &[],  // Not enabled by default - user enables when needed
    config_trees: &[
        "etc/bluetooth",
        "usr/libexec/bluetooth",  // Contains bluetoothd (not in /usr/sbin)
    ],
    config_files: &[
        "usr/share/dbus-1/system.d/bluetooth.conf",
    ],
    dirs: &["var/lib/bluetooth"],
    symlinks: &[],
    users: &[],
    groups: &[Group { name: "bluetooth", gid: 170 }],
    custom: &[],
};

/// PipeWire audio server (modern replacement for PulseAudio).
/// Runs as a user service, not a system service.
pub static PIPEWIRE_SVC: Service = Service {
    name: "pipewire",
    phase: Phase::Services,
    // Client tools
    bins: &[
        "pw-cli", "pw-dump", "pw-cat", "pw-play", "pw-record",
        "pw-top", "pw-metadata", "pw-mon", "pw-link",
        "wpctl",  // WirePlumber control
        // PulseAudio compatibility (pacmd not provided by pipewire-pulseaudio)
        "pactl", "paplay", "parecord",
    ],
    sbins: PIPEWIRE_SBIN,
    units: &[],  // No system units
    user_units: PIPEWIRE_UNITS,  // User-level services
    enable: &[],  // User units are socket-activated
    config_trees: &[
        "usr/share/pipewire",
        "etc/pipewire",
        "usr/share/wireplumber",
        "etc/wireplumber",
    ],
    config_files: &[],  // ReserveDevice1.service not present in Rocky
    dirs: &[],
    symlinks: &[],
    users: &[],
    groups: &[
        Group { name: "pipewire", gid: 171 },
        Group { name: "audio", gid: 63 },
    ],
    custom: &[],
};

/// Polkit authorization framework.
/// Required for desktop privilege escalation (e.g., mounting disks, changing settings).
pub static POLKIT_SVC: Service = Service {
    name: "polkit",
    phase: Phase::Services,
    bins: &["pkexec", "pkaction", "pkcheck"],
    sbins: POLKIT_SBIN,
    units: POLKIT_UNITS,
    user_units: &[],
    enable: &[],  // Started on-demand by D-Bus
    config_trees: &[
        "usr/share/polkit-1",
        "etc/polkit-1",
        "usr/lib/polkit-1",  // Contains polkitd (not in /usr/sbin)
    ],
    config_files: &[
        "usr/share/dbus-1/system.d/org.freedesktop.PolicyKit1.conf",
        "usr/share/dbus-1/system-services/org.freedesktop.PolicyKit1.service",
    ],
    dirs: &["var/lib/polkit-1"],
    symlinks: &[],
    users: &[User {
        name: "polkitd",
        uid: 27,
        gid: 27,
        home: "/",
        shell: "/sbin/nologin",
    }],
    groups: &[Group { name: "polkitd", gid: 27 }],
    custom: &[],
};

/// UDisks2 disk management daemon.
/// Enables automatic mounting of USB drives and other removable media.
pub static UDISKS_SVC: Service = Service {
    name: "udisks2",
    phase: Phase::Services,
    bins: &["udisksctl"],
    sbins: UDISKS_SBIN,
    units: UDISKS_UNITS,
    user_units: &[],
    enable: &[],  // Started on-demand by D-Bus
    config_trees: &[
        "usr/lib/udisks2",
        "etc/udisks2",
        "usr/libexec/udisks2",  // Contains udisksd (not in /usr/sbin)
    ],
    config_files: &[
        "usr/share/dbus-1/system.d/org.freedesktop.UDisks2.conf",
        "usr/share/dbus-1/system-services/org.freedesktop.UDisks2.service",
    ],
    dirs: &["var/lib/udisks2"],
    symlinks: &[],
    users: &[],
    groups: &[],
    custom: &[],
};

/// UPower power management daemon.
/// Provides battery status, suspend/hibernate support.
pub static UPOWER_SVC: Service = Service {
    name: "upower",
    phase: Phase::Services,
    bins: &["upower"],
    sbins: UPOWER_SBIN,
    units: UPOWER_UNITS,
    user_units: &[],
    enable: &[],  // Started on-demand by D-Bus
    config_trees: &[
        "etc/UPower",
        // Note: upowerd is a standalone binary in /usr/libexec, handled via CopyFile
    ],
    config_files: &[
        "usr/share/dbus-1/system.d/org.freedesktop.UPower.conf",
        "usr/share/dbus-1/system-services/org.freedesktop.UPower.service",
        "usr/libexec/upowerd",  // Contains upowerd (not in /usr/sbin)
    ],
    dirs: &["var/lib/upower"],
    symlinks: &[],
    users: &[],
    groups: &[],
    custom: &[],
};
