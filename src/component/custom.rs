//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each function here is called by the executor when processing CustomOp variants.
//!
//! NOTE: This file contains the ACTUAL implementations, not delegations to build/*.
//! The old build/* files are kept only for utilities (libdeps.rs, context.rs, kernel.rs).

use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::CustomOp;
use crate::build::context::BuildContext;
use crate::build::libdeps::make_executable;
use crate::common::binary::copy_dir_recursive;
use crate::process::{shell_in, Cmd};

// =============================================================================
// Public API for external callers (like iso.rs)
// =============================================================================

/// Create live overlay directory with autologin, serial console, empty root password.
///
/// This is called by iso.rs during ISO creation. The overlay is applied ONLY
/// during live boot, not extracted to installed systems.
pub fn create_live_overlay_at(output_dir: &Path) -> Result<()> {
    println!("Creating live overlay directory...");

    let overlay_dir = output_dir.join("live-overlay");
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }

    let systemd_dir = overlay_dir.join("etc/systemd/system");
    let getty_wants = systemd_dir.join("getty.target.wants");
    let multi_user_wants = systemd_dir.join("multi-user.target.wants");

    fs::create_dir_all(&getty_wants)?;
    fs::create_dir_all(&multi_user_wants)?;
    fs::create_dir_all(overlay_dir.join("etc"))?;

    // Console autologin service
    fs::write(
        systemd_dir.join("console-autologin.service"),
        "[Unit]\n\
         Description=Console Autologin\n\
         After=systemd-user-sessions.service getty-pre.target\n\
         Before=getty.target\n\n\
         [Service]\n\
         Environment=HOME=/root\nEnvironment=TERM=linux\n\
         WorkingDirectory=/root\nExecStart=/bin/bash --login\n\
         StandardInput=tty\nStandardOutput=tty\nStandardError=tty\n\
         TTYPath=/dev/tty1\nTTYReset=yes\nTTYVHangup=yes\nTTYVTDisallocate=yes\n\
         Type=idle\nRestart=always\nRestartSec=0\n\n\
         [Install]\nWantedBy=getty.target\n",
    )?;

    std::os::unix::fs::symlink(
        "../console-autologin.service",
        getty_wants.join("console-autologin.service"),
    )?;

    // Disable getty@tty1 during live boot
    let getty_override = systemd_dir.join("getty@tty1.service.d");
    fs::create_dir_all(&getty_override)?;
    fs::write(
        getty_override.join("live-disable.conf"),
        "[Unit]\nConditionPathExists=!/live-boot-marker\n",
    )?;

    // Serial console service
    fs::write(
        systemd_dir.join("serial-console.service"),
        "[Unit]\n\
         Description=Serial Console Shell\n\
         After=basic.target\nConflicts=rescue.service emergency.service\n\n\
         [Service]\n\
         Environment=HOME=/root\nEnvironment=TERM=vt100\n\
         WorkingDirectory=/root\nExecStart=/bin/bash --login\n\
         StandardInput=tty\nStandardOutput=tty\nStandardError=tty\n\
         TTYPath=/dev/ttyS0\nTTYReset=yes\nTTYVHangup=yes\nTTYVTDisallocate=no\n\
         Type=idle\nRestart=always\nRestartSec=0\n\n\
         [Install]\nWantedBy=multi-user.target\n",
    )?;

    std::os::unix::fs::symlink(
        "../serial-console.service",
        multi_user_wants.join("serial-console.service"),
    )?;

    // Shadow file with empty root password
    fs::write(
        overlay_dir.join("etc/shadow"),
        "root::19000:0:99999:7:::\n\
         bin:*:19000:0:99999:7:::\ndaemon:*:19000:0:99999:7:::\nnobody:*:19000:0:99999:7:::\n\
         systemd-network:!*:19000::::::\nsystemd-resolve:!*:19000::::::\n\
         systemd-timesync:!*:19000::::::\nsystemd-coredump:!*:19000::::::\n\
         dbus:!*:19000::::::\nchrony:!*:19000::::::\n",
    )?;

    fs::set_permissions(
        overlay_dir.join("etc/shadow"),
        fs::Permissions::from_mode(0o600),
    )?;

    println!("  Created live overlay");
    Ok(())
}

/// Execute a custom operation.
pub fn execute(ctx: &BuildContext, op: CustomOp) -> Result<()> {
    match op {
        CustomOp::CreateFhsSymlinks => create_fhs_symlinks(ctx),
        CustomOp::CreateLiveOverlay => create_live_overlay(ctx),
        CustomOp::CopyWifiFirmware => copy_wifi_firmware(ctx),
        CustomOp::CopyAllFirmware => copy_all_firmware(ctx),
        CustomOp::RunDepmod => run_depmod(ctx),
        CustomOp::CopyModules => copy_modules(ctx),
        CustomOp::CreateEtcFiles => create_etc_files(ctx),
        CustomOp::CopyTimezoneData => copy_timezone_data(ctx),
        CustomOp::CopyLocales => copy_locales(ctx),
        CustomOp::CopyDracutModules => copy_dracut_modules(ctx),
        CustomOp::CopySystemdBootEfi => copy_systemd_boot_efi(ctx),
        CustomOp::CopyKeymaps => copy_keymaps(ctx),
        CustomOp::CreateWelcomeMessage => create_welcome_message(ctx),
        CustomOp::CopyRecstrap => copy_recstrap(ctx),
        CustomOp::DisableSelinux => disable_selinux(ctx),
        CustomOp::CreatePamFiles => create_pam_files(ctx),
        CustomOp::CreateSecurityConfig => create_security_config(ctx),
        CustomOp::CopyRecipe => copy_recipe(ctx),
        CustomOp::SetupRecipeConfig => setup_recipe_config(ctx),
        CustomOp::SetupLiveSystemdConfigs => setup_live_systemd_configs(ctx),
        CustomOp::CreateDracutConfig => create_dracut_config(ctx),
    }
}

// =============================================================================
// Filesystem operations
// =============================================================================

fn create_fhs_symlinks(ctx: &BuildContext) -> Result<()> {
    println!("Creating symlinks...");

    // /var/run -> /run
    let var_run = ctx.staging.join("var/run");
    if !var_run.exists() && !var_run.is_symlink() {
        std::os::unix::fs::symlink("/run", &var_run)
            .context("Failed to create /var/run symlink")?;
    }

    // /var/lock -> /run/lock
    let var_lock = ctx.staging.join("var/lock");
    if !var_lock.exists() && !var_lock.is_symlink() {
        std::os::unix::fs::symlink("/run/lock", &var_lock)
            .context("Failed to create /var/lock symlink")?;
    }

    // /bin -> /usr/bin (merged usr)
    let bin_link = ctx.staging.join("bin");
    if bin_link.exists() && !bin_link.is_symlink() {
        fs::remove_dir_all(&bin_link)?;
    }
    if !bin_link.exists() {
        std::os::unix::fs::symlink("usr/bin", &bin_link)
            .context("Failed to create /bin symlink")?;
    }

    // /sbin -> /usr/sbin (merged usr)
    let sbin_link = ctx.staging.join("sbin");
    if sbin_link.exists() && !sbin_link.is_symlink() {
        fs::remove_dir_all(&sbin_link)?;
    }
    if !sbin_link.exists() {
        std::os::unix::fs::symlink("usr/sbin", &sbin_link)
            .context("Failed to create /sbin symlink")?;
    }

    // /lib -> /usr/lib (merged usr)
    let lib_link = ctx.staging.join("lib");
    if lib_link.exists() && !lib_link.is_symlink() {
        fs::remove_dir_all(&lib_link)?;
    }
    if !lib_link.exists() {
        std::os::unix::fs::symlink("usr/lib", &lib_link)
            .context("Failed to create /lib symlink")?;
    }

    // /lib64 -> /usr/lib64 (merged usr)
    let lib64_link = ctx.staging.join("lib64");
    if lib64_link.exists() && !lib64_link.is_symlink() {
        fs::remove_dir_all(&lib64_link)?;
    }
    if !lib64_link.exists() {
        std::os::unix::fs::symlink("usr/lib64", &lib64_link)
            .context("Failed to create /lib64 symlink")?;
    }

    // /usr/bin/sh -> bash
    let sh_link = ctx.staging.join("usr/bin/sh");
    if !sh_link.exists() && !sh_link.is_symlink() {
        std::os::unix::fs::symlink("bash", &sh_link)
            .context("Failed to create /usr/bin/sh symlink")?;
    }

    println!("  Created essential symlinks");
    Ok(())
}

// =============================================================================
// Live overlay (complex multi-file generation)
// =============================================================================

fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    // Delegate to the public function that takes just the output path
    create_live_overlay_at(&ctx.output)
}

// =============================================================================
// Firmware operations (size tracking, multiple sources)
// =============================================================================

const WIFI_FIRMWARE_DIRS: &[&str] = &[
    "iwlwifi", "ath10k", "ath11k", "rtlwifi", "rtw88", "rtw89", "brcm", "cypress", "mediatek",
];

fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.source.join("lib/firmware");
    let alt_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("lib/firmware");

    let actual_src = if firmware_src.is_dir() {
        &firmware_src
    } else if alt_src.is_dir() {
        &alt_src
    } else {
        bail!("No firmware directory found - WiFi won't work");
    };

    fs::create_dir_all(&firmware_dst)?;

    let mut total: u64 = 0;
    for dir_name in WIFI_FIRMWARE_DIRS {
        let src_dir = actual_src.join(dir_name);
        if src_dir.is_dir() {
            let dst_dir = firmware_dst.join(dir_name);
            let size = copy_dir_recursive(&src_dir, &dst_dir)?;
            if size > 0 {
                total += size;
            }
        }
    }

    // Also copy iwlwifi-* files in root firmware dir
    if let Ok(entries) = fs::read_dir(actual_src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.starts_with("iwlwifi-") {
                    let dst = firmware_dst.join(&*name);
                    if !dst.exists() {
                        fs::copy(&path, &dst)?;
                        total += fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
    }

    println!("  WiFi firmware: {:.1} MB", total as f64 / 1_000_000.0);
    Ok(())
}

fn copy_all_firmware(ctx: &BuildContext) -> Result<()> {
    let firmware_src = ctx.source.join("usr/lib/firmware");
    let firmware_dst = ctx.staging.join("usr/lib/firmware");

    let alt_src = ctx.source.join("lib/firmware");
    let actual_src = if firmware_src.exists() {
        &firmware_src
    } else if alt_src.exists() {
        &alt_src
    } else {
        bail!(
            "No firmware directory found.\n\
             Firmware is REQUIRED - LevitateOS is a daily driver for real hardware."
        );
    };

    fs::create_dir_all(&firmware_dst)?;

    let size = copy_dir_recursive(actual_src, &firmware_dst)?;
    println!(
        "  Copied all firmware ({:.1} MB)",
        size as f64 / 1_000_000.0
    );

    // Copy Intel microcode from Rocky's non-standard location
    let intel_ucode_dst = firmware_dst.join("intel-ucode");
    let microcode_ctl_src = ctx
        .source
        .join("usr/share/microcode_ctl/ucode_with_caveats/intel/intel-ucode");
    if microcode_ctl_src.exists() && microcode_ctl_src.is_dir() {
        fs::create_dir_all(&intel_ucode_dst)?;
        let intel_size = copy_dir_recursive(&microcode_ctl_src, &intel_ucode_dst)?;
        println!(
            "  Copied Intel microcode from microcode_ctl ({:.1} KB)",
            intel_size as f64 / 1_000.0
        );
    }

    // Validate microcode directories exist (P0 critical for CPU security)
    let amd_ucode = firmware_dst.join("amd-ucode");
    let intel_ucode = firmware_dst.join("intel-ucode");

    if !amd_ucode.exists() {
        bail!(
            "AMD microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.",
            amd_ucode.display()
        );
    }
    let amd_count = fs::read_dir(&amd_ucode)?.filter(|e| e.is_ok()).count();
    if amd_count == 0 {
        bail!("AMD microcode directory is empty at {}", amd_ucode.display());
    }
    println!("  AMD microcode: {} files", amd_count);

    if !intel_ucode.exists() {
        bail!(
            "Intel microcode not found at {}.\n\
             LevitateOS ISO must work on ANY x86-64 hardware.",
            intel_ucode.display()
        );
    }
    let intel_count = fs::read_dir(&intel_ucode)?.filter(|e| e.is_ok()).count();
    if intel_count == 0 {
        bail!(
            "Intel microcode directory is empty at {}",
            intel_ucode.display()
        );
    }
    println!("  Intel microcode: {} files", intel_count);

    Ok(())
}

// =============================================================================
// Kernel modules
// =============================================================================

/// Module metadata files needed by modprobe.
const MODULE_METADATA_FILES: &[&str] = &[
    "modules.dep",
    "modules.dep.bin",
    "modules.alias",
    "modules.alias.bin",
    "modules.softdep",
    "modules.symbols",
    "modules.symbols.bin",
    "modules.builtin",
    "modules.builtin.bin",
    "modules.builtin.modinfo",
    "modules.order",
];

fn copy_modules(ctx: &BuildContext) -> Result<()> {
    println!("Setting up kernel modules...");

    let config = crate::config::Config::load();
    let modules = config.all_modules();

    let modules_base = ctx.source.join("usr/lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;
    println!("  Kernel version: {}", kernel_version);

    let src_modules = modules_base.join(&kernel_version);
    let dst_modules = ctx.staging.join("lib/modules").join(&kernel_version);
    fs::create_dir_all(&dst_modules)?;

    // Copy specified modules - ALL specified modules are REQUIRED
    let mut missing = Vec::new();
    for module in &modules {
        let src = src_modules.join(module);
        if src.exists() {
            let module_name = Path::new(module)
                .file_name()
                .context("Invalid module path")?;
            let dst = dst_modules.join(module_name);
            fs::copy(&src, &dst)?;
            println!("  Copied {}", module_name.to_string_lossy());
        } else {
            missing.push(module.to_string());
        }
    }

    // FAIL FAST if any specified module is missing
    if !missing.is_empty() {
        bail!(
            "Required kernel modules not found: {:?}\n\
             \n\
             These modules were specified in the configuration.\n\
             If a module is in the config, it's REQUIRED for hardware support.\n\
             \n\
             DO NOT change this to a warning. FAIL FAST.",
            missing
        );
    }

    // Copy module metadata files
    println!("  Copying module metadata for modprobe...");
    for metadata_file in MODULE_METADATA_FILES {
        let src = src_modules.join(metadata_file);
        if src.exists() {
            fs::copy(&src, dst_modules.join(metadata_file))?;
        }
    }

    // Run depmod
    println!("  Running depmod...");
    Cmd::new("depmod")
        .args(["-a", "-b"])
        .arg_path(&ctx.staging)
        .arg(&kernel_version)
        .error_msg("depmod failed. Install: sudo dnf install kmod")
        .run()?;
    println!("  depmod completed successfully");

    Ok(())
}

fn run_depmod(ctx: &BuildContext) -> Result<()> {
    let modules_base = ctx.staging.join("lib/modules");
    let kernel_version = find_kernel_version(&modules_base)?;

    Cmd::new("depmod")
        .args(["-a", "-b"])
        .arg_path(&ctx.staging)
        .arg(&kernel_version)
        .error_msg("depmod failed. Install: sudo dnf install kmod")
        .run()?;

    Ok(())
}

fn find_kernel_version(modules_base: &Path) -> Result<String> {
    for entry in fs::read_dir(modules_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains('.') && entry.path().is_dir() {
            return Ok(name_str.to_string());
        }
    }
    bail!("Could not find kernel modules directory")
}

// =============================================================================
// /etc configuration
// =============================================================================

fn create_etc_files(ctx: &BuildContext) -> Result<()> {
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

fn copy_timezone_data(ctx: &BuildContext) -> Result<()> {
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

fn copy_locales(ctx: &BuildContext) -> Result<()> {
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

fn create_dracut_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating dracut configuration...");

    let dracut_conf_dir = ctx.staging.join("etc/dracut.conf.d");
    fs::create_dir_all(&dracut_conf_dir)?;

    fs::write(
        dracut_conf_dir.join("levitate.conf"),
        r#"# LevitateOS dracut defaults
add_drivers+=" ext4 vfat "
hostonly="no"
add_dracutmodules+=" base rootfs-block "
compress="gzip"
"#,
    )?;

    println!("  Created /etc/dracut.conf.d/levitate.conf");
    Ok(())
}

// =============================================================================
// Dracut and bootloader
// =============================================================================

fn copy_dracut_modules(ctx: &BuildContext) -> Result<()> {
    let dracut_src = ctx.source.join("usr/lib/dracut");
    let dracut_dst = ctx.staging.join("usr/lib/dracut");

    if dracut_src.exists() {
        fs::create_dir_all(dracut_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&dracut_src, &dracut_dst)?;
        println!(
            "  Copied dracut modules ({:.1} MB)",
            size as f64 / 1_000_000.0
        );
    } else {
        bail!(
            "Dracut modules not found at {}.\n\
             Dracut is REQUIRED - it generates the initramfs during installation.",
            dracut_src.display()
        );
    }

    Ok(())
}

fn copy_systemd_boot_efi(ctx: &BuildContext) -> Result<()> {
    let efi_dst = ctx.staging.join("usr/lib/systemd/boot/efi");

    let rpm_dir = ctx
        .base_dir
        .join("downloads/iso-contents/AppStream/Packages/s");
    let rpm_pattern = "systemd-boot-unsigned";

    let rpm_path = std::fs::read_dir(&rpm_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains(rpm_pattern))
                .unwrap_or(false)
        });

    let Some(rpm_path) = rpm_path else {
        bail!(
            "systemd-boot-unsigned RPM not found in {}.\n\
             The EFI files from this package are REQUIRED for bootctl install.",
            rpm_dir.display()
        );
    };

    let temp_dir = ctx.base_dir.join("output/.systemd-boot-extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let cmd = format!("rpm2cpio '{}' | cpio -idm", rpm_path.display());
    shell_in(&cmd, &temp_dir)
        .with_context(|| format!("Failed to extract RPM: {}", rpm_path.display()))?;

    let efi_src = temp_dir.join("usr/lib/systemd/boot/efi");
    if efi_src.exists() {
        fs::create_dir_all(efi_dst.parent().unwrap())?;
        let size = copy_dir_recursive(&efi_src, &efi_dst)?;
        println!(
            "  Copied systemd-boot EFI files ({:.1} KB)",
            size as f64 / 1_000.0
        );
    } else {
        bail!("EFI files not found in extracted RPM at {}", temp_dir.display());
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

// =============================================================================
// Keymaps
// =============================================================================

fn copy_keymaps(ctx: &BuildContext) -> Result<()> {
    let keymaps_src = ctx.source.join("usr/lib/kbd/keymaps");
    let keymaps_dst = ctx.staging.join("usr/lib/kbd/keymaps");

    if keymaps_src.exists() {
        fs::create_dir_all(keymaps_dst.parent().unwrap())?;
        copy_dir_recursive(&keymaps_src, &keymaps_dst)?;
        println!("  Copied keymaps for keyboard layout support");
    } else {
        bail!(
            "Keymaps not found at {}.\n\
             Keymaps are REQUIRED for keyboard layout support (loadkeys).",
            keymaps_src.display()
        );
    }

    Ok(())
}

// =============================================================================
// Welcome message and recstrap
// =============================================================================

fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    let motd = ctx.staging.join("etc/motd");
    fs::write(
        &motd,
        r#"
  _                _ _        _        ___  ____
 | |    _____   __(_) |_ __ _| |_ ___ / _ \/ ___|
 | |   / _ \ \ / /| | __/ _` | __/ _ \ | | \___ \
 | |__|  __/\ V / | | || (_| | ||  __/ |_| |___) |
 |_____\___| \_/  |_|\__\__,_|\__\___|\___/|____/

 Welcome to LevitateOS Live!

 Installation (manual, like Arch):

   # 1. Partition disk
   fdisk /dev/vda                   # Create GPT, EFI + root partitions

   # 2. Format partitions
   mkfs.fat -F32 /dev/vda1          # EFI partition
   mkfs.ext4 /dev/vda2              # Root partition

   # 3. Mount
   mount /dev/vda2 /mnt
   mkdir -p /mnt/boot
   mount /dev/vda1 /mnt/boot

   # 4. Extract system
   recstrap /mnt

   # 5. Generate fstab
   recfstab -U /mnt >> /mnt/etc/fstab

   # 6. Chroot and configure
   recchroot /mnt
   passwd                           # Set root password
   bootctl install                  # Install bootloader
   exit

   # 7. Reboot
   reboot

 For networking:
   nmcli device wifi list           # List WiFi networks
   nmcli device wifi connect SSID password PASSWORD

"#,
    )?;

    let issue = ctx.staging.join("etc/issue");
    fs::write(&issue, "\nLevitateOS Live - \\l\n\n")?;

    Ok(())
}

fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    use crate::deps::DependencyResolver;

    let resolver = DependencyResolver::new(&ctx.base_dir)?;

    let (recstrap, recfstab, recchroot) = resolver
        .all_tools()
        .context("Installation tools are REQUIRED - the ISO cannot install itself without them")?;

    for tool in [&recstrap, &recfstab, &recchroot] {
        let dst = ctx.staging.join("usr/bin").join(tool.tool.name());
        fs::copy(&tool.path, &dst)?;
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        println!(
            "  Copied {} to /usr/bin/{} (from {:?})",
            tool.tool.name(),
            tool.tool.name(),
            tool.source
        );
    }

    Ok(())
}

// =============================================================================
// SELinux
// =============================================================================

fn disable_selinux(ctx: &BuildContext) -> Result<()> {
    let selinux_dir = ctx.staging.join("etc/selinux");
    fs::create_dir_all(&selinux_dir)?;

    fs::write(
        selinux_dir.join("config"),
        "# SELinux disabled - LevitateOS doesn't ship SELinux policies\n\
         SELINUX=disabled\n\
         SELINUXTYPE=targeted\n",
    )?;

    println!("  Disabled SELinux");
    Ok(())
}

// =============================================================================
// PAM
// =============================================================================

const PAM_SYSTEM_AUTH: &str = "\
auth required pam_env.so
auth sufficient pam_unix.so try_first_pass nullok
auth required pam_deny.so
account required pam_unix.so
password requisite pam_pwquality.so try_first_pass local_users_only retry=3
password sufficient pam_unix.so try_first_pass use_authtok nullok sha512 shadow
password required pam_deny.so
session optional pam_keyinit.so revoke
session required pam_limits.so
session required pam_unix.so
";

const PAM_LOGIN: &str = "\
auth requisite pam_nologin.so
auth include system-auth
account required pam_access.so
account include system-auth
password include system-auth
session required pam_loginuid.so
session optional pam_keyinit.so force revoke
session include system-auth
session required pam_namespace.so
session optional pam_lastlog.so showfailed
session optional pam_motd.so
";

fn create_pam_files(ctx: &BuildContext) -> Result<()> {
    println!("Setting up PAM configuration...");

    let pam_dir = ctx.staging.join("etc/pam.d");
    fs::create_dir_all(&pam_dir)?;

    fs::write(pam_dir.join("system-auth"), PAM_SYSTEM_AUTH)?;
    fs::write(pam_dir.join("password-auth"), PAM_SYSTEM_AUTH)?;
    fs::write(pam_dir.join("login"), PAM_LOGIN)?;

    fs::write(pam_dir.join("passwd"), "auth include system-auth\naccount include system-auth\npassword substack system-auth\n")?;
    fs::write(pam_dir.join("su"), "auth sufficient pam_rootok.so\nauth required pam_unix.so\naccount sufficient pam_rootok.so\naccount required pam_unix.so\nsession required pam_unix.so\n")?;
    fs::write(pam_dir.join("sudo"), "auth include system-auth\naccount include system-auth\npassword include system-auth\nsession optional pam_keyinit.so revoke\nsession required pam_limits.so\n")?;
    fs::write(pam_dir.join("chpasswd"), "auth sufficient pam_rootok.so\nauth required pam_unix.so\naccount required pam_unix.so\npassword include system-auth\n")?;
    fs::write(pam_dir.join("other"), "auth required pam_deny.so\naccount required pam_deny.so\npassword required pam_deny.so\nsession required pam_deny.so\n")?;
    fs::write(pam_dir.join("systemd-user"), "account include system-auth\nsession required pam_loginuid.so\nsession optional pam_keyinit.so force revoke\nsession include system-auth\n")?;

    println!("  Created PAM configuration files");
    Ok(())
}

fn create_security_config(ctx: &BuildContext) -> Result<()> {
    println!("Creating security configuration...");

    let security_dir = ctx.staging.join("etc/security");
    fs::create_dir_all(&security_dir)?;

    fs::write(
        security_dir.join("limits.conf"),
        "*               soft    core            0\n\
         *               hard    nofile          1048576\n\
         *               soft    nofile          1024\n\
         root            soft    nofile          1048576\n",
    )?;

    fs::write(security_dir.join("access.conf"), "+:root:LOCAL\n+:ALL:ALL\n")?;
    fs::write(security_dir.join("namespace.conf"), "# Polyinstantiation config\n")?;
    fs::write(security_dir.join("pam_env.conf"), "# Environment variables\n")?;
    fs::write(security_dir.join("pwquality.conf"), "minlen = 8\nminclass = 1\n")?;

    println!("  Created security configuration");
    Ok(())
}

// =============================================================================
// Recipe
// =============================================================================

fn copy_recipe(ctx: &BuildContext) -> Result<()> {
    println!("Copying recipe package manager...");

    let recipe_path = match &ctx.recipe_binary {
        Some(path) => {
            if !path.exists() {
                bail!(
                    "Recipe binary explicitly specified but not found at: {}\n\
                     Build it with: cd recipe && cargo build --release",
                    path.display()
                );
            }
            path.clone()
        }
        None => {
            if let Ok(env_path) = std::env::var("RECIPE_BINARY") {
                let path = std::path::PathBuf::from(&env_path);
                if path.exists() {
                    println!("  Using recipe from RECIPE_BINARY env var");
                    path
                } else {
                    bail!(
                        "RECIPE_BINARY points to non-existent path: {}\n\
                         Build it or update the env var.",
                        env_path
                    );
                }
            } else {
                let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                let search_paths = [
                    manifest_dir.parent().unwrap().join("recipe/target/release/recipe"),
                ];

                match search_paths.iter().find(|p| p.exists()) {
                    Some(path) => path.clone(),
                    None => {
                        bail!(
                            "recipe binary not found. LevitateOS REQUIRES the package manager.\n\
                             \n\
                             For monorepo users:\n\
                               cd ../recipe && cargo build --release\n\
                             \n\
                             For standalone users:\n\
                               1. Clone recipe: git clone https://github.com/LevitateOS/recipe\n\
                               2. Build it: cd recipe && cargo build --release\n\
                               3. Set env var: export RECIPE_BINARY=/path/to/recipe/target/release/recipe\n\
                             \n\
                             DO NOT remove this check. An ISO without recipe is BROKEN."
                        );
                    }
                }
            }
        }
    };

    let dest = ctx.staging.join("usr/bin/recipe");
    fs::copy(&recipe_path, &dest)
        .with_context(|| format!("Failed to copy recipe from {:?}", recipe_path))?;
    make_executable(&dest)?;

    println!("  Copied recipe to /usr/bin/recipe");
    Ok(())
}

fn setup_recipe_config(ctx: &BuildContext) -> Result<()> {
    println!("Setting up recipe configuration...");

    let recipe_dirs = [
        "etc/recipe",
        "etc/recipe/repos",
        "etc/recipe/repos/rocky10",
        "var/lib/recipe",
        "var/cache/recipe",
    ];

    for dir in recipe_dirs {
        fs::create_dir_all(ctx.staging.join(dir))?;
    }

    fs::write(
        ctx.staging.join("etc/recipe/recipe.conf"),
        r#"# Recipe package manager configuration
recipe_path = "/etc/recipe/repos/rocky10"
cache_dir = "/var/cache/recipe"
db_dir = "/var/lib/recipe"
"#,
    )?;

    fs::write(
        ctx.staging.join("etc/profile.d/recipe.sh"),
        "export RECIPE_PATH=/etc/recipe/repos/rocky10\n",
    )?;

    println!("  Created recipe configuration");
    Ok(())
}

// =============================================================================
// Live systemd configs
// =============================================================================

fn setup_live_systemd_configs(ctx: &BuildContext) -> Result<()> {
    println!("Setting up live systemd configs...");

    let journald_dir = ctx.staging.join("etc/systemd/journald.conf.d");
    fs::create_dir_all(&journald_dir)?;
    fs::write(
        journald_dir.join("volatile.conf"),
        "[Journal]\nStorage=volatile\nRuntimeMaxUse=64M\n",
    )?;

    let logind_dir = ctx.staging.join("etc/systemd/logind.conf.d");
    fs::create_dir_all(&logind_dir)?;
    fs::write(
        logind_dir.join("do-not-suspend.conf"),
        "[Login]\nHandleSuspendKey=ignore\nHandleHibernateKey=ignore\n\
         HandleLidSwitch=ignore\nHandleLidSwitchExternalPower=ignore\nIdleAction=ignore\n",
    )?;

    println!("  Created live systemd configs");
    Ok(())
}
