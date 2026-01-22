//! /etc configuration files for installed system.
//!
//! These are the complete configuration files needed for a real
//! installed system (not the minimal live environment).

use anyhow::Result;
use std::fs;

use super::context::BuildContext;
use super::filesystem::copy_dir_recursive;

/// Create all /etc configuration files.
pub fn create_etc_files(ctx: &BuildContext) -> Result<()> {
    println!("Creating /etc configuration files...");

    create_passwd_files(ctx)?;
    create_system_identity(ctx)?;
    create_filesystem_config(ctx)?;
    create_auth_config(ctx)?;
    create_locale_config(ctx)?;
    create_network_config(ctx)?;
    create_shell_config(ctx)?;
    create_nsswitch(ctx)?;

    println!("  Created /etc configuration files");
    Ok(())
}

/// Create passwd, shadow, group, gshadow files.
fn create_passwd_files(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/passwd - basic system users
    fs::write(
        etc.join("passwd"),
        r#"root:x:0:0:root:/root:/usr/bin/bash
bin:x:1:1:bin:/bin:/usr/sbin/nologin
daemon:x:2:2:daemon:/sbin:/usr/sbin/nologin
nobody:x:65534:65534:Kernel Overflow User:/:/usr/sbin/nologin
systemd-network:x:192:192:systemd Network Management:/:/usr/sbin/nologin
systemd-resolve:x:193:193:systemd Resolver:/:/usr/sbin/nologin
systemd-timesync:x:194:194:systemd Time Synchronization:/:/usr/sbin/nologin
systemd-coredump:x:195:195:systemd Core Dumper:/:/usr/sbin/nologin
dbus:x:81:81:System message bus:/:/usr/sbin/nologin
chrony:x:996:993::/var/lib/chrony:/usr/sbin/nologin
"#,
    )?;

    // /etc/shadow - password hashes (root has no password initially)
    fs::write(
        etc.join("shadow"),
        r#"root:!:19000:0:99999:7:::
bin:*:19000:0:99999:7:::
daemon:*:19000:0:99999:7:::
nobody:*:19000:0:99999:7:::
systemd-network:!*:19000::::::
systemd-resolve:!*:19000::::::
systemd-timesync:!*:19000::::::
systemd-coredump:!*:19000::::::
dbus:!*:19000::::::
chrony:!*:19000::::::
"#,
    )?;

    // Set proper permissions on shadow
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(etc.join("shadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("shadow"), perms)?;

    // /etc/group
    fs::write(
        etc.join("group"),
        r#"root:x:0:
bin:x:1:
daemon:x:2:
sys:x:3:
adm:x:4:
tty:x:5:
disk:x:6:
wheel:x:10:
kmem:x:9:
audio:x:11:
video:x:12:
users:x:100:
nobody:x:65534:
systemd-network:x:192:
systemd-resolve:x:193:
systemd-timesync:x:194:
systemd-coredump:x:195:
dbus:x:81:
chrony:x:993:
"#,
    )?;

    // /etc/gshadow
    fs::write(
        etc.join("gshadow"),
        r#"root:::
bin:::
daemon:::
sys:::
adm:::
tty:::
disk:::
wheel:::
kmem:::
audio:::
video:::
users:::
nobody:::
systemd-network:!::
systemd-resolve:!::
systemd-timesync:!::
systemd-coredump:!::
dbus:!::
chrony:!::
"#,
    )?;

    // Set proper permissions on gshadow
    let mut perms = fs::metadata(etc.join("gshadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("gshadow"), perms)?;

    Ok(())
}

/// Create system identity files.
fn create_system_identity(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/hostname (empty - will be set during installation)
    fs::write(etc.join("hostname"), "levitateos\n")?;

    // /etc/machine-id (empty - systemd generates on first boot)
    fs::write(etc.join("machine-id"), "")?;

    // /etc/os-release
    fs::write(
        etc.join("os-release"),
        r#"NAME="LevitateOS"
ID=levitateos
ID_LIKE=fedora
VERSION="1.0"
VERSION_ID=1
PRETTY_NAME="LevitateOS 1.0"
HOME_URL="https://levitateos.org"
BUG_REPORT_URL="https://github.com/levitateos/levitateos/issues"
"#,
    )?;

    Ok(())
}

/// Create filesystem configuration.
fn create_filesystem_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/fstab - template, will be updated during installation
    fs::write(
        etc.join("fstab"),
        r#"# /etc/fstab - Static file system information
# <device>  <mount>  <type>  <options>  <dump>  <fsck>

# Root filesystem (set by installer)
# /dev/xxx  /  ext4  defaults  0  1

# EFI System Partition (set by installer)
# /dev/xxx  /boot/efi  vfat  umask=0077  0  2

# Proc and sys (always needed)
proc  /proc  proc  defaults  0  0
sysfs  /sys  sysfs  defaults  0  0
devtmpfs  /dev  devtmpfs  mode=0755,nosuid  0  0
tmpfs  /tmp  tmpfs  defaults,nosuid,nodev  0  0
tmpfs  /run  tmpfs  mode=0755,nosuid,nodev  0  0
"#,
    )?;

    // /etc/mtab -> /proc/self/mounts
    let mtab = etc.join("mtab");
    if !mtab.exists() && !mtab.is_symlink() {
        std::os::unix::fs::symlink("/proc/self/mounts", &mtab)?;
    }

    Ok(())
}

/// Create authentication configuration.
fn create_auth_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/securetty - allowed tty for root login
    fs::write(
        etc.join("securetty"),
        r#"console
tty1
tty2
tty3
tty4
tty5
tty6
ttyS0
ttyS1
"#,
    )?;

    // /etc/shells - valid login shells
    fs::write(
        etc.join("shells"),
        r#"/usr/bin/bash
/bin/bash
/usr/bin/sh
/bin/sh
"#,
    )?;

    // /etc/login.defs
    fs::write(
        etc.join("login.defs"),
        r#"# Login configuration
MAIL_DIR /var/spool/mail
PASS_MAX_DAYS 99999
PASS_MIN_DAYS 0
PASS_WARN_AGE 7
UID_MIN 1000
UID_MAX 60000
SYS_UID_MIN 201
SYS_UID_MAX 999
GID_MIN 1000
GID_MAX 60000
SYS_GID_MIN 201
SYS_GID_MAX 999
CREATE_HOME yes
UMASK 022
USERGROUPS_ENAB yes
ENCRYPT_METHOD SHA512
"#,
    )?;

    // /etc/sudoers - sudo configuration
    // wheel group members can use sudo with password
    fs::write(
        etc.join("sudoers"),
        r#"## Sudoers file - LevitateOS
##
## This file MUST be edited with visudo!
## Run: visudo
##

# Host alias specification
# (none needed for single host)

# User alias specification
# (none needed)

# Cmnd alias specification
# (none needed)

# Defaults specification
Defaults   !visiblepw
Defaults   always_set_home
Defaults   match_group_by_gid
Defaults   env_reset
Defaults   env_keep = "COLORS DISPLAY HOSTNAME HISTSIZE KDEDIR LS_COLORS"
Defaults   env_keep += "MAIL QTDIR USERNAME LANG LC_ADDRESS LC_CTYPE"
Defaults   env_keep += "LC_COLLATE LC_IDENTIFICATION LC_MEASUREMENT LC_MESSAGES"
Defaults   env_keep += "LC_MONETARY LC_NAME LC_NUMERIC LC_PAPER LC_TELEPHONE"
Defaults   env_keep += "LC_TIME LC_ALL LANGUAGE LINGUAS _XKB_CHARSET XAUTHORITY"
Defaults   secure_path = /usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# User privilege specification
root    ALL=(ALL:ALL) ALL

# Allow members of group wheel to execute any command
%wheel  ALL=(ALL:ALL) ALL

# Include drop-in files from /etc/sudoers.d
@includedir /etc/sudoers.d
"#,
    )?;

    // Set proper permissions on sudoers (must be 0440)
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(etc.join("sudoers"))?.permissions();
    perms.set_mode(0o440);
    fs::set_permissions(etc.join("sudoers"), perms)?;

    // /etc/sudoers.d/ directory for drop-in configs
    fs::create_dir_all(etc.join("sudoers.d"))?;

    // /etc/sudo.conf - sudo configuration
    fs::write(
        etc.join("sudo.conf"),
        r#"# sudo.conf - sudo configuration file
#
# Plugin configuration
Plugin sudoers_policy sudoers.so
Plugin sudoers_io sudoers.so
Plugin sudoers_audit sudoers.so

# Path to sudo log file
Set disable_coredump true
Set group_source static
"#,
    )?;

    Ok(())
}

/// Create locale and time configuration.
fn create_locale_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/localtime -> UTC (default, installer can change)
    let localtime = etc.join("localtime");
    if !localtime.exists() && !localtime.is_symlink() {
        std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime)?;
    }

    // /etc/adjtime
    fs::write(
        etc.join("adjtime"),
        r#"0.0 0 0.0
0
UTC
"#,
    )?;

    // /etc/locale.conf
    fs::write(etc.join("locale.conf"), "LANG=C.UTF-8\n")?;

    // /etc/vconsole.conf
    fs::write(etc.join("vconsole.conf"), "KEYMAP=us\n")?;

    Ok(())
}

/// Create network configuration.
fn create_network_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/hosts
    fs::write(
        etc.join("hosts"),
        r#"127.0.0.1   localhost localhost.localdomain
::1         localhost localhost.localdomain ip6-localhost ip6-loopback
"#,
    )?;

    // /etc/resolv.conf (stub - systemd-resolved manages this)
    let resolv = etc.join("resolv.conf");
    if !resolv.exists() && !resolv.is_symlink() {
        // Point to systemd-resolved stub
        std::os::unix::fs::symlink("/run/systemd/resolve/stub-resolv.conf", &resolv)?;
    }

    Ok(())
}

/// Create shell configuration.
fn create_shell_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    // /etc/profile
    fs::write(
        etc.join("profile"),
        r#"# System-wide profile
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
export EDITOR="vi"
export PAGER="less"

# Source profile.d scripts
for script in /etc/profile.d/*.sh; do
    [ -r "$script" ] && . "$script"
done
unset script

# Interactive shell settings
if [ -n "$PS1" ]; then
    PS1='[\u@\h \W]\$ '

    # Power management aliases (live environment doesn't have full polkit)
    alias poweroff='systemctl poweroff --force'
    alias reboot='systemctl reboot --force'
    alias halt='systemctl halt --force'

    # Show welcome message on login
    [ -f /etc/motd ] && cat /etc/motd
fi
"#,
    )?;

    // /etc/profile.d/xdg.sh - XDG Base Directory Specification
    fs::create_dir_all(etc.join("profile.d"))?;
    fs::write(
        etc.join("profile.d/xdg.sh"),
        r#"# XDG Base Directory Specification
# https://specifications.freedesktop.org/basedir-spec/latest/

# User directories (defaults per spec)
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"

# System directories
export XDG_DATA_DIRS="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_CONFIG_DIRS="${XDG_CONFIG_DIRS:-/etc/xdg}"

# Runtime directory (set by pam_systemd, but provide fallback)
if [ -z "$XDG_RUNTIME_DIR" ]; then
    export XDG_RUNTIME_DIR="/run/user/$(id -u)"
fi
"#,
    )?;

    // /etc/bashrc - for non-login interactive shells
    // NOTE: Do NOT source /etc/profile here! Login shells already do that.
    // Sourcing it here causes double-execution (motd shown twice, etc.)
    fs::write(
        etc.join("bashrc"),
        r#"# System-wide bashrc (non-login interactive shells)
# Interactive shell settings
if [ -n "$PS1" ]; then
    # History settings
    HISTSIZE=1000
    HISTFILESIZE=2000
    HISTCONTROL=ignoredups:erasedups

    # Enable color ls
    alias ls='ls --color=auto'
    alias ll='ls -la'
    alias l='ls -l'
fi
"#,
    )?;

    // /root/.bashrc
    let root_home = ctx.staging.join("root");
    fs::write(
        root_home.join(".bashrc"),
        r#"# Root bashrc
[ -f /etc/bashrc ] && . /etc/bashrc
export PS1='[\u@\h \W]# '
"#,
    )?;

    // /root/.bash_profile
    fs::write(
        root_home.join(".bash_profile"),
        r#"# Root bash_profile
[ -f ~/.bashrc ] && . ~/.bashrc
"#,
    )?;

    // /etc/skel/.bashrc (for new users)
    fs::write(
        etc.join("skel/.bashrc"),
        r#"# User bashrc
[ -f /etc/bashrc ] && . /etc/bashrc
"#,
    )?;

    // /etc/skel/.bash_profile
    fs::write(
        etc.join("skel/.bash_profile"),
        r#"# User bash_profile
[ -f ~/.bashrc ] && . ~/.bashrc
"#,
    )?;

    // XDG directories with .keep files to ensure they're copied by useradd -m
    for xdg_dir in [".config", ".local/share", ".local/state", ".cache"] {
        let dir = etc.join("skel").join(xdg_dir);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(".keep"), "")?;
    }

    Ok(())
}

/// Create nsswitch.conf for name service lookup.
fn create_nsswitch(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(
        etc.join("nsswitch.conf"),
        r#"# Name Service Switch configuration
passwd:     files systemd
shadow:     files
group:      files systemd
hosts:      files resolve [!UNAVAIL=return] dns myhostname
networks:   files
protocols:  files
services:   files
ethers:     files
rpc:        files
"#,
    )?;

    Ok(())
}

/// Copy timezone data from source rootfs.
pub fn copy_timezone_data(ctx: &BuildContext) -> Result<()> {
    println!("Copying timezone data...");

    let src = ctx.source.join("usr/share/zoneinfo");
    let dst = ctx.staging.join("usr/share/zoneinfo");
    fs::create_dir_all(&dst)?;

    if src.exists() {
        // Copy ALL timezone data - users need their local timezone
        // This is a daily driver OS, not an embedded system
        copy_dir_recursive(&src, &dst)?;
        println!("  Copied all timezone data");
    }

    Ok(())
}

/// Copy locales from source rootfs.
pub fn copy_locales(ctx: &BuildContext) -> Result<()> {
    println!("Copying locales...");

    // Copy locale-archive if it exists (compiled locales)
    let archive_src = ctx.source.join("usr/lib/locale/locale-archive");
    let archive_dst = ctx.staging.join("usr/lib/locale/locale-archive");

    if archive_src.exists() {
        fs::create_dir_all(archive_dst.parent().unwrap())?;
        fs::copy(&archive_src, &archive_dst)?;
        println!("  Copied locale-archive");
    }

    Ok(())
}
