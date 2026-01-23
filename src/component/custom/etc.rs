//! /etc configuration file creation.

use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use leviso_elf::copy_dir_recursive;

use crate::build::context::BuildContext;

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

fn create_passwd_files(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

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
sshd:x:74:74:Privilege-separated SSH:/var/empty/sshd:/usr/sbin/nologin
"#,
    )?;

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
sshd:!*:19000::::::
"#,
    )?;

    let mut perms = fs::metadata(etc.join("shadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("shadow"), perms)?;

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
sshd:x:74:
"#,
    )?;

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
sshd:!::
"#,
    )?;

    let mut perms = fs::metadata(etc.join("gshadow"))?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(etc.join("gshadow"), perms)?;

    Ok(())
}

fn create_system_identity(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    let name = std::env::var("OS_NAME").unwrap_or_else(|_| "LevitateOS".to_string());
    let id = std::env::var("OS_ID").unwrap_or_else(|_| "levitateos".to_string());
    let id_like = std::env::var("OS_ID_LIKE").unwrap_or_else(|_| "fedora".to_string());
    let version = std::env::var("OS_VERSION").unwrap_or_else(|_| "1.0".to_string());
    let version_id = std::env::var("OS_VERSION_ID").unwrap_or_else(|_| "1".to_string());
    let home_url = std::env::var("OS_HOME_URL").unwrap_or_else(|_| "https://levitateos.org".to_string());
    let bug_url = std::env::var("OS_BUG_REPORT_URL")
        .unwrap_or_else(|_| "https://github.com/levitateos/levitateos/issues".to_string());

    let hostname = std::env::var("OS_HOSTNAME").unwrap_or_else(|_| id.clone());
    fs::write(etc.join("hostname"), format!("{}\n", hostname))?;
    fs::write(etc.join("machine-id"), "")?;

    fs::write(
        etc.join("os-release"),
        format!(
            r#"NAME="{name}"
ID={id}
ID_LIKE={id_like}
VERSION="{version}"
VERSION_ID={version_id}
PRETTY_NAME="{name} {version}"
HOME_URL="{home_url}"
BUG_REPORT_URL="{bug_url}"
"#
        ),
    )?;

    Ok(())
}

fn create_filesystem_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(
        etc.join("fstab"),
        r#"# /etc/fstab - Static file system information
# <device>  <mount>  <type>  <options>  <dump>  <fsck>

proc  /proc  proc  defaults  0  0
sysfs  /sys  sysfs  defaults  0  0
devtmpfs  /dev  devtmpfs  mode=0755,nosuid  0  0
tmpfs  /tmp  tmpfs  defaults,nosuid,nodev  0  0
tmpfs  /run  tmpfs  mode=0755,nosuid,nodev  0  0
"#,
    )?;

    let mtab = etc.join("mtab");
    if !mtab.exists() && !mtab.is_symlink() {
        std::os::unix::fs::symlink("/proc/self/mounts", &mtab)?;
    }

    Ok(())
}

fn create_auth_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(
        etc.join("securetty"),
        "console\ntty1\ntty2\ntty3\ntty4\ntty5\ntty6\nttyS0\nttyS1\n",
    )?;

    fs::write(
        etc.join("shells"),
        "/usr/bin/bash\n/bin/bash\n/usr/bin/sh\n/bin/sh\n",
    )?;

    fs::write(
        etc.join("login.defs"),
        r#"MAIL_DIR /var/spool/mail
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

    fs::write(
        etc.join("sudoers"),
        r#"Defaults   !visiblepw
Defaults   always_set_home
Defaults   match_group_by_gid
Defaults   env_reset
Defaults   secure_path = /usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

root    ALL=(ALL:ALL) ALL
%wheel  ALL=(ALL:ALL) ALL

@includedir /etc/sudoers.d
"#,
    )?;

    let mut perms = fs::metadata(etc.join("sudoers"))?.permissions();
    perms.set_mode(0o440);
    fs::set_permissions(etc.join("sudoers"), perms)?;

    fs::create_dir_all(etc.join("sudoers.d"))?;

    fs::write(
        etc.join("sudo.conf"),
        "Plugin sudoers_policy sudoers.so\nPlugin sudoers_io sudoers.so\n\
         Set disable_coredump true\nSet group_source static\n",
    )?;

    Ok(())
}

fn create_locale_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    let localtime = etc.join("localtime");
    if !localtime.exists() && !localtime.is_symlink() {
        std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", &localtime)?;
    }

    fs::write(etc.join("adjtime"), "0.0 0 0.0\n0\nUTC\n")?;
    fs::write(etc.join("locale.conf"), "LANG=C.UTF-8\n")?;
    fs::write(etc.join("vconsole.conf"), "KEYMAP=us\n")?;

    Ok(())
}

fn create_network_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(
        etc.join("hosts"),
        "127.0.0.1   localhost localhost.localdomain\n\
         ::1         localhost localhost.localdomain ip6-localhost ip6-loopback\n",
    )?;

    let resolv = etc.join("resolv.conf");
    if !resolv.exists() && !resolv.is_symlink() {
        std::os::unix::fs::symlink("/run/systemd/resolve/stub-resolv.conf", &resolv)?;
    }

    Ok(())
}

fn create_shell_config(ctx: &BuildContext) -> Result<()> {
    let etc = ctx.staging.join("etc");

    fs::write(
        etc.join("profile"),
        r#"export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
export EDITOR="vi"
export PAGER="less"

for script in /etc/profile.d/*.sh; do
    [ -r "$script" ] && . "$script"
done
unset script

if [ -n "$PS1" ]; then
    PS1='[\u@\h \W]\$ '
    alias poweroff='systemctl poweroff --force'
    alias reboot='systemctl reboot --force'
    alias halt='systemctl halt --force'
    [ -f /etc/motd ] && cat /etc/motd
fi
"#,
    )?;

    fs::create_dir_all(etc.join("profile.d"))?;
    fs::write(
        etc.join("profile.d/xdg.sh"),
        r#"export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export XDG_DATA_DIRS="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export XDG_CONFIG_DIRS="${XDG_CONFIG_DIRS:-/etc/xdg}"
if [ -z "$XDG_RUNTIME_DIR" ]; then
    export XDG_RUNTIME_DIR="/run/user/$(id -u)"
fi
"#,
    )?;

    fs::write(
        etc.join("bashrc"),
        r#"if [ -n "$PS1" ]; then
    HISTSIZE=1000
    HISTFILESIZE=2000
    HISTCONTROL=ignoredups:erasedups
    alias ls='ls --color=auto'
    alias ll='ls -la'
    alias l='ls -l'
fi
"#,
    )?;

    let root_home = ctx.staging.join("root");
    fs::write(
        root_home.join(".bashrc"),
        "[ -f /etc/bashrc ] && . /etc/bashrc\nexport PS1='[\\u@\\h \\W]# '\n",
    )?;
    fs::write(
        root_home.join(".bash_profile"),
        "[ -f ~/.bashrc ] && . ~/.bashrc\n",
    )?;

    fs::write(
        etc.join("skel/.bashrc"),
        "[ -f /etc/bashrc ] && . /etc/bashrc\n",
    )?;
    fs::write(
        etc.join("skel/.bash_profile"),
        "[ -f ~/.bashrc ] && . ~/.bashrc\n",
    )?;

    for xdg_dir in [".config", ".local/share", ".local/state", ".cache"] {
        let dir = etc.join("skel").join(xdg_dir);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(".keep"), "")?;
    }

    Ok(())
}

fn create_nsswitch(ctx: &BuildContext) -> Result<()> {
    fs::write(
        ctx.staging.join("etc/nsswitch.conf"),
        r#"passwd:     files systemd
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

/// Copy timezone data from source to staging.
pub fn copy_timezone_data(ctx: &BuildContext) -> Result<()> {
    println!("Copying timezone data...");

    let src = ctx.source.join("usr/share/zoneinfo");
    let dst = ctx.staging.join("usr/share/zoneinfo");
    fs::create_dir_all(&dst)?;

    if src.exists() {
        copy_dir_recursive(&src, &dst)?;
        println!("  Copied all timezone data");
    }

    Ok(())
}

/// Copy locale archive from source to staging.
pub fn copy_locales(ctx: &BuildContext) -> Result<()> {
    println!("Copying locales...");

    let archive_src = ctx.source.join("usr/lib/locale/locale-archive");
    let archive_dst = ctx.staging.join("usr/lib/locale/locale-archive");

    if archive_src.exists() {
        fs::create_dir_all(archive_dst.parent().unwrap())?;
        fs::copy(&archive_src, &archive_dst)?;
        println!("  Copied locale-archive");
    }

    Ok(())
}
