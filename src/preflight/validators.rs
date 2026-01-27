//! Anti-cheat validation functions.
//!
//! These functions verify OUTCOMES, not PROXIES.
//! See: https://www.anthropic.com/research/emergent-misalignment-reward-hacking
//!
//! A check that only tests .exists() is like writing "A+" on your own essay.
//! These functions verify the file will actually work for its intended purpose.

use std::path::Path;

use distro_builder::process::Cmd;

/// Validate Rocky ISO is complete (not a partial download).
///
/// ANTI-CHEAT: A partial curl download creates a file that passes .exists()
/// but fails during extraction. We check file size >= 7GB (actual is ~8.6GB).
pub fn validate_rocky_iso_size(base_dir: &Path) -> Result<f64, String> {
    let downloads_dir = base_dir.join("downloads");

    // Find the Rocky ISO file
    let iso_path = if let Ok(path) = std::env::var("ROCKY_ISO_PATH") {
        std::path::PathBuf::from(path)
    } else {
        // Check for default filename
        let default = downloads_dir.join("Rocky-10.1-x86_64-dvd1.iso");
        if default.exists() {
            default
        } else {
            // Look for any Rocky ISO
            match std::fs::read_dir(&downloads_dir) {
                Ok(entries) => {
                    entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .find(|p| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().starts_with("Rocky-") && n.to_string_lossy().ends_with(".iso"))
                                .unwrap_or(false)
                        })
                        .ok_or_else(|| "No Rocky ISO found in downloads/".to_string())?
                }
                Err(_) => return Err("Cannot read downloads directory".to_string()),
            }
        }
    };

    let metadata = std::fs::metadata(&iso_path)
        .map_err(|e| format!("Cannot stat ISO: {}", e))?;

    let size_bytes = metadata.len();
    let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    // Rocky 10 DVD ISO is ~8.6GB. Anything under 7GB is definitely partial.
    const MIN_SIZE_GB: f64 = 7.0;

    if size_gb < MIN_SIZE_GB {
        return Err(format!(
            "ISO is only {:.2}GB (expected ~8.6GB) - likely partial download",
            size_gb
        ));
    }

    Ok(size_gb)
}

/// Validate kconfig is a real kernel configuration.
///
/// ANTI-CHEAT: An empty file or random text passes .exists() but produces
/// a broken kernel. We verify it contains actual CONFIG_ options.
pub fn validate_kconfig(path: &Path) -> Result<usize, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read: {}", e))?;

    // Count CONFIG_ lines (both =y and =m and =n and strings)
    let config_count = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("CONFIG_") && trimmed.contains('=')
        })
        .count();

    // A real kernel config has thousands of options. 100 is a very low bar.
    const MIN_CONFIG_OPTIONS: usize = 100;

    if config_count < MIN_CONFIG_OPTIONS {
        return Err(format!(
            "Only {} CONFIG_ options found (expected 1000+) - file may be corrupted or incomplete",
            config_count
        ));
    }

    // Check for critical options that MUST be present for LevitateOS
    let critical_options = [
        "CONFIG_SQUASHFS",      // Required to mount Rocky Linux's install.img during extraction
        "CONFIG_OVERLAY_FS",    // Required for live overlay
        "CONFIG_BLK_DEV_LOOP",  // Required to mount EROFS/squashfs via loop device
    ];

    for opt in critical_options {
        if !content.contains(opt) {
            return Err(format!(
                "Missing critical option: {} - this kernel won't boot LevitateOS",
                opt
            ));
        }
    }

    Ok(config_count)
}

/// Validate init script is a real shell script.
///
/// ANTI-CHEAT: An empty file passes .exists() but the system won't boot.
/// We verify it has a shebang and substantial content.
pub fn validate_init_script(path: &Path) -> Result<usize, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Err("File is empty".to_string());
    }

    // Check for shebang
    let first_line = lines[0].trim();
    if !first_line.starts_with("#!") {
        return Err(format!(
            "No shebang found (first line: '{}') - not a valid script",
            first_line.chars().take(40).collect::<String>()
        ));
    }

    // Check it's a shell script (busybox sh, bash, etc.)
    if !first_line.contains("sh") && !first_line.contains("bash") {
        return Err(format!(
            "Unexpected interpreter: {} - expected shell",
            first_line
        ));
    }

    // Count non-empty, non-comment lines
    let code_lines = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .count();

    // A real init script has substantial logic. 20 lines is very minimal.
    const MIN_CODE_LINES: usize = 20;

    if code_lines < MIN_CODE_LINES {
        return Err(format!(
            "Only {} lines of code (expected 50+) - script may be incomplete",
            code_lines
        ));
    }

    // Check for critical commands that MUST be in an init script
    let critical_patterns = [
        "mount",        // Must mount filesystems
        "switch_root",  // Must pivot to real root (or exec init)
    ];

    for pattern in critical_patterns {
        if !content.contains(pattern) {
            return Err(format!(
                "Missing critical command: '{}' - init script won't work",
                pattern
            ));
        }
    }

    Ok(lines.len())
}

/// Validate a binary is executable and responds to --version or --help.
///
/// ANTI-CHEAT: A file can exist but not be executable, or be the wrong binary.
/// We verify it actually runs.
pub fn validate_executable(path: &Path, _name: &str) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;

    // Check executable bit
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Cannot stat: {}", e))?;

    let mode = metadata.permissions().mode();
    if mode & 0o111 == 0 {
        return Err("Not executable (missing +x permission)".to_string());
    }

    // Try to run --version
    let result = Cmd::new(path.to_string_lossy().as_ref())
        .arg("--version")
        .allow_fail()
        .run();

    match result {
        Ok(r) if r.success() => {
            let first_line = r.stdout.lines().next().unwrap_or("unknown");
            // Truncate to reasonable length
            let version = if first_line.len() > 50 {
                format!("{}...", &first_line[..47])
            } else {
                first_line.to_string()
            };
            Ok(version)
        }
        Ok(r) => {
            // --version failed but binary ran - try --help
            let help_result = Cmd::new(path.to_string_lossy().as_ref())
                .arg("--help")
                .allow_fail()
                .run();

            match help_result {
                Ok(h) if h.success() || !h.stdout.is_empty() => {
                    Ok("executable (no version)".to_string())
                }
                _ => {
                    Err(format!(
                        "Runs but --version failed: {}",
                        r.stderr.lines().next().unwrap_or("unknown error")
                    ))
                }
            }
        }
        Err(e) => Err(format!("Cannot execute: {}", e)),
    }
}
