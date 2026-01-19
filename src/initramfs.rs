use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_initramfs(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("downloads");
    let rootfs_dir = extract_dir.join("rootfs");
    let output_dir = base_dir.join("output");
    let initramfs_root = output_dir.join("initramfs-root");

    // Check if rootfs exists - try both direct and nested paths
    let actual_rootfs = if rootfs_dir.join("bin").exists() {
        rootfs_dir.clone()
    } else if rootfs_dir.join("squashfs-root").exists() {
        rootfs_dir.join("squashfs-root")
    } else if rootfs_dir.join("LiveOS").exists() {
        // Rocky uses LiveOS/rootfs.img inside install.img
        let liveos = rootfs_dir.join("LiveOS");
        if liveos.join("rootfs.img").exists() {
            println!("Found nested rootfs.img, extracting...");
            let inner_rootfs = extract_dir.join("inner-rootfs");
            if !inner_rootfs.exists() {
                fs::create_dir_all(&inner_rootfs)?;
                // Mount or extract the inner rootfs.img
                let status = Command::new("unsquashfs")
                    .args([
                        "-d",
                        inner_rootfs.to_str().unwrap(),
                        "-f",
                        liveos.join("rootfs.img").to_str().unwrap(),
                    ])
                    .status();

                if status.is_err() || !status.unwrap().success() {
                    // It might be ext4, try mounting
                    println!("Not a squashfs, trying to extract as ext4...");
                    let status = Command::new("7z")
                        .args([
                            "x",
                            "-y",
                            liveos.join("rootfs.img").to_str().unwrap(),
                            &format!("-o{}", inner_rootfs.display()),
                        ])
                        .status()?;
                    if !status.success() {
                        bail!("Could not extract inner rootfs.img");
                    }
                }
            }
            inner_rootfs
        } else {
            bail!("Rootfs not found. Run 'leviso extract' first.");
        }
    } else {
        bail!(
            "Rootfs not found at {}. Run 'leviso extract' first.",
            rootfs_dir.display()
        );
    };

    println!("Using rootfs from: {}", actual_rootfs.display());

    // Clean and create initramfs root
    if initramfs_root.exists() {
        fs::remove_dir_all(&initramfs_root)?;
    }
    fs::create_dir_all(&initramfs_root)?;

    // Create directory structure
    for dir in [
        "bin", "sbin", "lib64", "lib", "etc", "proc", "sys", "dev", "dev/pts", "tmp", "root",
        "run", "run/lock", "var/log", "var/tmp",
        "usr/lib/systemd/system", "usr/lib64/systemd", "etc/systemd/system",
        "mnt",
    ] {
        fs::create_dir_all(initramfs_root.join(dir))?;
    }

    // Create /var/run as symlink to /run
    let var_run = initramfs_root.join("var/run");
    if !var_run.exists() {
        std::os::unix::fs::symlink("/run", &var_run)?;
    }

    // Find bash
    let bash_candidates = [
        actual_rootfs.join("usr/bin/bash"),
        actual_rootfs.join("bin/bash"),
    ];
    let bash_path = bash_candidates
        .iter()
        .find(|p| p.exists())
        .context("Could not find bash in rootfs")?;

    println!("Found bash at: {}", bash_path.display());

    // Copy bash
    fs::copy(bash_path, initramfs_root.join("bin/bash"))?;

    // Make bash executable
    let mut perms = fs::metadata(initramfs_root.join("bin/bash"))?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(initramfs_root.join("bin/bash"), perms)?;

    // Get library dependencies using ldd
    println!("Finding library dependencies...");
    let ldd_output = Command::new("ldd")
        .arg(bash_path)
        .output()
        .context("Failed to run ldd")?;

    let libs = parse_ldd_output(&String::from_utf8_lossy(&ldd_output.stdout))?;

    // Copy libraries
    for lib in &libs {
        copy_library(&actual_rootfs, lib, &initramfs_root)?;
    }

    // Copy essential coreutils
    let coreutils = [
        "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch", "chmod", "chown", "echo", "pwd",
        "head", "tail", "grep", "find", "wc", "sort", "uniq", "uname", "env", "printenv", "clear",
        "sleep", "ln", "readlink", "dirname", "basename",
        // Phase 2: disk info (lsblk is in /usr/bin)
        "lsblk",
        // Phase 3: system config
        "date", "loadkeys",
        // Compression utilities
        "gzip", "gunzip",
        // Systemd utilities
        "timedatectl", "systemctl", "journalctl",
        // Console
        "agetty", "login",
    ];

    // Copy essential system binaries (from sbin)
    let sbin_utils = [
        "mount", "umount", "hostname",
        // Phase 2: disk utilities
        "blkid", "fdisk", "parted", "wipefs",
        "mkfs.ext4", "mkfs.fat",
        // Phase 3: system config
        "chroot", "hwclock",
    ];

    for util in coreutils {
        copy_binary_with_libs(&actual_rootfs, util, &initramfs_root)?;
    }

    for util in sbin_utils {
        copy_binary_with_libs(&actual_rootfs, util, &initramfs_root)?;
    }

    // Create symlinks
    std::os::unix::fs::symlink("bash", initramfs_root.join("bin/sh"))?;

    // Copy keymaps for loadkeys
    let keymaps_src = actual_rootfs.join("usr/lib/kbd/keymaps");
    let keymaps_dst = initramfs_root.join("usr/lib/kbd/keymaps");
    if keymaps_src.exists() {
        println!("Copying keymaps...");
        copy_dir_recursive(&keymaps_src, &keymaps_dst)?;
    }

    // Setup systemd as init
    setup_systemd(&actual_rootfs, &initramfs_root)?;

    // Copy init script (mounts cgroups, then execs systemd)
    let init_src = base_dir.join("profile/init");
    let init_dst = initramfs_root.join("init");
    fs::copy(&init_src, &init_dst)?;
    let mut perms = fs::metadata(&init_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_dst, perms)?;
    println!("Copied init script");

    // Create /etc/passwd and /etc/group for root
    fs::write(
        initramfs_root.join("etc/passwd"),
        "root:x:0:0:root:/root:/bin/bash\n",
    )?;
    fs::write(initramfs_root.join("etc/group"), "root:x:0:\n")?;

    // Create simple profile
    fs::write(
        initramfs_root.join("etc/profile"),
        r#"
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export PS1='root@leviso:\w# '
cd /root
"#,
    )?;

    fs::write(
        initramfs_root.join("root/.bashrc"),
        r#"
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export PS1='root@leviso:\w# '
"#,
    )?;

    // Setup PAM (required for login/agetty)
    setup_pam(&actual_rootfs, &initramfs_root)?;

    // Build cpio archive
    println!("Building initramfs cpio archive...");
    let initramfs_cpio = output_dir.join("initramfs.cpio.gz");

    let find_output = Command::new("sh")
        .current_dir(&initramfs_root)
        .args([
            "-c",
            "find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9",
        ])
        .output()
        .context("Failed to create cpio archive")?;

    if !find_output.status.success() {
        bail!(
            "cpio failed: {}",
            String::from_utf8_lossy(&find_output.stderr)
        );
    }

    fs::write(&initramfs_cpio, &find_output.stdout)?;
    println!("Created initramfs at: {}", initramfs_cpio.display());

    Ok(())
}

fn parse_ldd_output(output: &str) -> Result<Vec<String>> {
    let mut libs = Vec::new();

    for line in output.lines() {
        // Parse lines like: "libc.so.6 => /lib64/libc.so.6 (0x...)" or "/lib64/ld-linux-x86-64.so.2 (0x...)"
        let line = line.trim();
        if line.contains("=>") {
            if let Some(path_part) = line.split("=>").nth(1) {
                if let Some(path) = path_part.split_whitespace().next() {
                    if path.starts_with('/') {
                        libs.push(path.to_string());
                    }
                }
            }
        } else if line.starts_with('/') {
            if let Some(path) = line.split_whitespace().next() {
                libs.push(path.to_string());
            }
        }
    }

    Ok(libs)
}

fn copy_library(rootfs: &Path, lib_path: &str, initramfs: &Path) -> Result<()> {
    // Try to find the library in rootfs first, then fall back to host
    let src_candidates = [
        rootfs.join(lib_path.trim_start_matches('/')),
        rootfs.join("usr").join(lib_path.trim_start_matches('/')),
        PathBuf::from(lib_path), // Host system fallback
    ];

    let src = src_candidates
        .iter()
        .find(|p| p.exists())
        .with_context(|| format!("Could not find library: {}", lib_path))?;

    // Determine destination path
    let dest_path = if lib_path.contains("lib64") {
        initramfs
            .join("lib64")
            .join(Path::new(lib_path).file_name().unwrap())
    } else {
        initramfs
            .join("lib")
            .join(Path::new(lib_path).file_name().unwrap())
    };

    if !dest_path.exists() {
        // Handle symlinks
        if src.is_symlink() {
            let link_target = fs::read_link(src)?;
            // If it's a relative symlink, resolve it
            let actual_src = if link_target.is_relative() {
                src.parent().unwrap().join(&link_target)
            } else {
                link_target.clone()
            };

            // Copy the actual file
            if actual_src.exists() {
                fs::copy(&actual_src, &dest_path)?;
            } else {
                // Try in rootfs
                let rootfs_target =
                    rootfs.join(link_target.to_str().unwrap().trim_start_matches('/'));
                if rootfs_target.exists() {
                    fs::copy(&rootfs_target, &dest_path)?;
                } else {
                    fs::copy(src, &dest_path)?;
                }
            }
        } else {
            fs::copy(src, &dest_path)?;
        }
        println!("  Copied: {} -> {}", src.display(), dest_path.display());
    }

    Ok(())
}

fn copy_binary_with_libs(rootfs: &Path, binary: &str, initramfs: &Path) -> Result<()> {
    // Find the binary
    let bin_candidates = [
        rootfs.join("usr/bin").join(binary),
        rootfs.join("bin").join(binary),
        rootfs.join("usr/sbin").join(binary),
        rootfs.join("sbin").join(binary),
    ];

    let bin_path = match bin_candidates.iter().find(|p| p.exists()) {
        Some(p) => p,
        None => {
            println!("  Warning: {} not found, skipping", binary);
            return Ok(());
        }
    };

    // Copy binary
    let dest = initramfs.join("bin").join(binary);
    if !dest.exists() {
        fs::copy(bin_path, &dest)?;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
        println!("  Copied binary: {}", binary);
    }

    // Get and copy its libraries
    let ldd_output = Command::new("ldd").arg(bin_path).output();

    if let Ok(output) = ldd_output {
        if output.status.success() {
            let libs = parse_ldd_output(&String::from_utf8_lossy(&output.stdout))?;
            for lib in &libs {
                let _ = copy_library(rootfs, lib, initramfs);
            }
        }
    }

    Ok(())
}

fn setup_systemd(rootfs: &Path, initramfs: &Path) -> Result<()> {
    println!("Setting up systemd...");

    // Copy systemd binary
    let systemd_src = rootfs.join("usr/lib/systemd/systemd");
    let systemd_dst = initramfs.join("usr/lib/systemd/systemd");
    fs::create_dir_all(systemd_dst.parent().unwrap())?;
    fs::copy(&systemd_src, &systemd_dst)?;
    let mut perms = fs::metadata(&systemd_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&systemd_dst, perms)?;
    println!("  Copied systemd");

    // Copy essential systemd binaries (required by systemd 255+)
    // Reference: Arch mkinitcpio systemd hook
    let systemd_binaries = [
        "systemd-executor",      // CRITICAL: required since systemd 255
        "systemd-shutdown",
        "systemd-sulogin-shell",
        "systemd-cgroups-agent",
        "systemd-journald",
        "systemd-modules-load",
        "systemd-sysctl",
        "systemd-tmpfiles-setup",
    ];

    let systemd_lib_dir = initramfs.join("usr/lib/systemd");
    for binary in systemd_binaries {
        let src = rootfs.join("usr/lib/systemd").join(binary);
        if src.exists() {
            let dst = systemd_lib_dir.join(binary);
            fs::copy(&src, &dst)?;
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;
            println!("  Copied {}", binary);
        } else {
            println!("  Warning: {} not found", binary);
        }
    }

    // Copy systemd private libraries
    let systemd_lib_src = rootfs.join("usr/lib64/systemd");
    if systemd_lib_src.exists() {
        for entry in fs::read_dir(&systemd_lib_src)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                let dst = initramfs.join("usr/lib64/systemd").join(&name);
                fs::copy(entry.path(), &dst)?;
                println!("  Copied {}", name_str);
            }
        }
    }

    // Copy all libraries needed by systemd (found via recursive readelf analysis)
    let systemd_libs = [
        "libacl.so.1",
        "libattr.so.1",
        "libaudit.so.1",
        "libblkid.so.1",
        "libcap-ng.so.0",
        "libcap.so.2",
        "libcrypto.so.3",
        "libcrypt.so.2",
        "libc.so.6",
        "libeconf.so.0",
        "libgcc_s.so.1",
        "libmount.so.1",
        "libm.so.6",
        "libpam.so.0",
        "libpcre2-8.so.0",
        "libseccomp.so.2",
        "libselinux.so.1",
        "libz.so.1",
        "ld-linux-x86-64.so.2",
    ];
    for lib in systemd_libs {
        let src_candidates = [
            rootfs.join("usr/lib64").join(lib),
            rootfs.join("lib64").join(lib),
        ];
        let dst = initramfs.join("lib64").join(lib);
        if !dst.exists() {
            for src in &src_candidates {
                if src.exists() {
                    fs::copy(src, &dst)?;
                    println!("  Copied {}", lib);
                    break;
                }
            }
        }
    }

    // Create /sbin/init symlink to systemd (for chroot environments)
    let init_link = initramfs.join("sbin/init");
    if !init_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/systemd", &init_link)?;
    }

    // Note: /init is the bash script that mounts cgroups then execs systemd
    // It's copied from profile/init, not a symlink

    // Copy essential systemd unit files
    let unit_src = rootfs.join("usr/lib/systemd/system");
    let unit_dst = initramfs.join("usr/lib/systemd/system");

    let essential_units = [
        // Targets
        "basic.target",
        "sysinit.target",
        "multi-user.target",
        "default.target",
        "getty.target",
        "local-fs.target",
        "local-fs-pre.target",
        "network.target",
        "network-pre.target",
        "paths.target",
        "slices.target",
        "sockets.target",
        "timers.target",
        "swap.target",
        "shutdown.target",
        "rescue.target",
        "emergency.target",
        // Services
        "getty@.service",
        "serial-getty@.service",
        "systemd-tmpfiles-setup.service",
        "systemd-journald.service",
        "systemd-udevd.service",
        // Sockets
        "systemd-journald.socket",
        "systemd-journald-dev-log.socket",
    ];

    for unit in essential_units {
        let src = unit_src.join(unit);
        let dst = unit_dst.join(unit);
        if src.exists() {
            fs::copy(&src, &dst)?;
        }
    }

    // Don't copy .wants directories from rootfs - they contain files instead of symlinks
    // and systemd ignores non-symlinks. Create only the symlinks we need.
    println!("  Copied essential unit files");

    // Create autologin getty override for tty1
    let getty_override_dir = initramfs.join("etc/systemd/system/getty@tty1.service.d");
    fs::create_dir_all(&getty_override_dir)?;
    fs::write(
        getty_override_dir.join("autologin.conf"),
        r#"[Service]
ExecStart=
ExecStart=-/bin/agetty --autologin root --noclear --keep-baud %I 115200,38400,9600 $TERM
Type=idle
"#,
    )?;

    // Create a simple serial console service - just run bash directly
    // This is simpler for a live environment than agetty+login
    let serial_console = initramfs.join("etc/systemd/system/serial-console.service");
    fs::write(
        &serial_console,
        r#"[Unit]
Description=Serial Console Shell
After=basic.target
Conflicts=rescue.service emergency.service

[Service]
Environment=HOME=/root
Environment=TERM=vt100
WorkingDirectory=/root
ExecStart=/bin/bash --login
StandardInput=tty
StandardOutput=tty
StandardError=tty
TTYPath=/dev/ttyS0
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=no
Type=idle
Restart=always
RestartSec=0

[Install]
WantedBy=multi-user.target
"#,
    )?;

    // Enable both getty on tty1 and serial-console
    let getty_wants = initramfs.join("etc/systemd/system/getty.target.wants");
    fs::create_dir_all(&getty_wants)?;
    let getty_link = getty_wants.join("getty@tty1.service");
    if !getty_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &getty_link)?;
    }

    // Enable serial-console directly (doesn't use udev)
    let multi_user_wants = initramfs.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;
    let serial_link = multi_user_wants.join("serial-console.service");
    if !serial_link.exists() {
        std::os::unix::fs::symlink("/etc/systemd/system/serial-console.service", &serial_link)?;
    }

    // Enable getty.target from multi-user.target
    let multi_user_wants = initramfs.join("etc/systemd/system/multi-user.target.wants");
    fs::create_dir_all(&multi_user_wants)?;
    let getty_target_link = multi_user_wants.join("getty.target");
    if !getty_target_link.exists() {
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty.target", &getty_target_link)?;
    }

    // Create machine-id (empty, systemd will populate on first boot)
    fs::write(initramfs.join("etc/machine-id"), "")?;

    // Create os-release
    fs::write(
        initramfs.join("etc/os-release"),
        r#"NAME="LevitateOS"
ID=levitateos
VERSION="1.0"
PRETTY_NAME="LevitateOS Live"
"#,
    )?;

    println!("  Configured autologin on tty1");

    Ok(())
}

fn setup_pam(rootfs: &Path, initramfs: &Path) -> Result<()> {
    println!("Setting up PAM...");

    // Create PAM directories
    let pam_d = initramfs.join("etc/pam.d");
    let security_dir = initramfs.join("lib64/security");
    fs::create_dir_all(&pam_d)?;
    fs::create_dir_all(&security_dir)?;

    // Copy essential PAM modules
    let pam_modules = [
        "pam_permit.so",
        "pam_deny.so",
        "pam_unix.so",
        "pam_rootok.so",
        "pam_env.so",
        "pam_limits.so",
        "pam_nologin.so",
        "pam_securetty.so",
        "pam_shells.so",
        "pam_succeed_if.so",
    ];

    let pam_src = rootfs.join("usr/lib64/security");
    for module in pam_modules {
        let src = pam_src.join(module);
        if src.exists() {
            let dst = security_dir.join(module);
            fs::copy(&src, &dst)?;
            println!("  Copied {}", module);
        }
    }

    // Create minimal PAM config for login (permissive for live environment)
    fs::write(
        pam_d.join("login"),
        r#"#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_permit.so
account    required     pam_permit.so
password   required     pam_permit.so
session    required     pam_permit.so
"#,
    )?;

    // System-auth (referenced by other PAM configs)
    fs::write(
        pam_d.join("system-auth"),
        r#"#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_permit.so
account    required     pam_permit.so
password   required     pam_permit.so
session    required     pam_permit.so
"#,
    )?;

    // Create /etc/securetty (terminals where root can login)
    fs::write(
        initramfs.join("etc/securetty"),
        "tty1\ntty2\ntty3\ntty4\ntty5\ntty6\nttyS0\n",
    )?;

    // Create empty /etc/shadow for root (no password = allow login)
    fs::write(initramfs.join("etc/shadow"), "root::0::::::\n")?;

    // Create /etc/shells (required for login)
    fs::write(initramfs.join("etc/shells"), "/bin/bash\n/bin/sh\n")?;

    println!("  Created PAM configuration");

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}
