use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_initramfs(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("rocky-extracted");
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
        "bin", "sbin", "lib64", "lib", "etc", "proc", "sys", "dev", "tmp", "root",
    ] {
        fs::create_dir_all(initramfs_root.join(dir))?;
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
    ];

    // Copy essential system binaries (from sbin)
    let sbin_utils = ["mount", "umount", "hostname"];

    for util in coreutils {
        copy_binary_with_libs(&actual_rootfs, util, &initramfs_root)?;
    }

    for util in sbin_utils {
        copy_binary_with_libs(&actual_rootfs, util, &initramfs_root)?;
    }

    // Create symlinks
    std::os::unix::fs::symlink("bash", initramfs_root.join("bin/sh"))?;

    // Copy init script
    let init_src = base_dir.join("profile/init");
    let init_dst = initramfs_root.join("init");
    fs::copy(&init_src, &init_dst)?;
    let mut perms = fs::metadata(&init_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_dst, perms)?;

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
