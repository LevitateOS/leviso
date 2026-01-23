//! Preflight checks for LevitateOS build.
//!
//! Validates all dependencies and host tools before starting a build.
//! Run with `leviso preflight` to check everything is ready.

use anyhow::{bail, Result};
use std::path::Path;

use crate::deps::DependencyResolver;
use crate::process::{self, Cmd};

/// Result of a single preflight check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Check passed.
    Pass,
    /// Check failed - build will fail.
    Fail,
    /// Check passed but with a warning.
    Warn,
    /// Check skipped (not applicable).
    #[allow(dead_code)]
    Skip,
}

impl CheckResult {
    fn pass(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            details: None,
        }
    }

    fn pass_with(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            details: Some(details.to_string()),
        }
    }

    fn fail(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Fail,
            details: Some(details.to_string()),
        }
    }

    fn warn(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Warn,
            details: Some(details.to_string()),
        }
    }
}

/// Results of all preflight checks.
pub struct PreflightReport {
    pub checks: Vec<CheckResult>,
}

impl PreflightReport {
    /// Returns true if all checks passed (no failures).
    pub fn all_passed(&self) -> bool {
        !self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    /// Count of failed checks.
    pub fn fail_count(&self) -> usize {
        self.checks.iter().filter(|c| c.status == CheckStatus::Fail).count()
    }

    /// Count of warnings.
    pub fn warn_count(&self) -> usize {
        self.checks.iter().filter(|c| c.status == CheckStatus::Warn).count()
    }

    /// Print the report to stdout.
    pub fn print(&self) {
        println!("=== Preflight Check Results ===\n");

        for check in &self.checks {
            let icon = match check.status {
                CheckStatus::Pass => "✓",
                CheckStatus::Fail => "✗",
                CheckStatus::Warn => "⚠",
                CheckStatus::Skip => "○",
            };

            let status_str = match check.status {
                CheckStatus::Pass => "PASS",
                CheckStatus::Fail => "FAIL",
                CheckStatus::Warn => "WARN",
                CheckStatus::Skip => "SKIP",
            };

            print!("  {} [{}] {}", icon, status_str, check.name);
            if let Some(details) = &check.details {
                println!(": {}", details);
            } else {
                println!();
            }
        }

        println!();
        let total = self.checks.len();
        let passed = self.checks.iter().filter(|c| c.status == CheckStatus::Pass).count();
        let failed = self.fail_count();
        let warned = self.warn_count();

        println!("Summary: {}/{} passed", passed, total);
        if failed > 0 {
            println!("         {} FAILED - build will not succeed", failed);
        }
        if warned > 0 {
            println!("         {} warnings", warned);
        }
    }
}

/// Run all preflight checks.
pub fn run_preflight(base_dir: &Path) -> Result<PreflightReport> {
    let mut checks = Vec::new();

    println!("Running preflight checks...\n");

    // =======================================================================
    // Host Tools
    // =======================================================================
    println!("Checking host tools...");
    checks.extend(check_host_tools());

    // =======================================================================
    // Dependencies
    // =======================================================================
    println!("Checking dependencies...");
    checks.extend(check_dependencies(base_dir)?);

    // =======================================================================
    // Build Environment
    // =======================================================================
    println!("Checking build environment...");
    checks.extend(check_build_environment(base_dir));

    println!();

    Ok(PreflightReport { checks })
}

/// Check host tools are installed.
fn check_host_tools() -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Required tools with package hints
    let required_tools = [
        ("mksquashfs", "squashfs-tools", "Required to create squashfs filesystem"),
        ("unsquashfs", "squashfs-tools", "Required to extract squashfs"),
        ("xorriso", "xorriso", "Required to create ISO image"),
        ("mkfs.fat", "dosfstools", "Required for EFI partition"),
        ("readelf", "binutils", "Required for library dependency detection"),
    ];

    for (tool, package, purpose) in required_tools {
        let result = check_tool_exists(tool, package, purpose, true);
        results.push(result);
    }

    // Optional tools (for testing/development)
    let optional_tools = [
        ("qemu-system-x86_64", "qemu-system-x86", "Required for `leviso run`"),
        ("qemu-img", "qemu-utils", "Required for virtual disk creation"),
    ];

    for (tool, package, purpose) in optional_tools {
        let result = check_tool_exists(tool, package, purpose, false);
        results.push(result);
    }

    // Check for OVMF (UEFI firmware)
    let ovmf_paths = [
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/qemu/OVMF.fd",
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
    ];

    let ovmf_found = ovmf_paths.iter().any(|p| Path::new(p).exists());
    if ovmf_found {
        results.push(CheckResult::pass("OVMF firmware"));
    } else {
        results.push(CheckResult::warn(
            "OVMF firmware",
            "Not found - `leviso run` requires UEFI. Install edk2-ovmf or ovmf package.",
        ));
    }

    results
}

/// Check if a tool exists in PATH.
fn check_tool_exists(tool: &str, package: &str, purpose: &str, required: bool) -> CheckResult {
    match process::which(tool) {
        Some(path) => CheckResult::pass_with(tool, &path),
        None => {
            let msg = format!(
                "Not found. Install '{}' package. {}",
                package, purpose
            );
            if required {
                CheckResult::fail(tool, &msg)
            } else {
                CheckResult::warn(tool, &msg)
            }
        }
    }
}

/// Check all build dependencies.
fn check_dependencies(base_dir: &Path) -> Result<Vec<CheckResult>> {
    let mut results = Vec::new();
    let resolver = DependencyResolver::new(base_dir)?;

    // Linux source
    if resolver.has_linux() {
        results.push(CheckResult::pass_with(
            "Linux source",
            "Found (submodule or downloaded)",
        ));
    } else {
        results.push(CheckResult::warn(
            "Linux source",
            "Not found - will be downloaded on first build",
        ));
    }

    // Rocky ISO - ANTI-CHEAT: verify size, not just existence
    // A partial curl download passes .exists() but fails the build later.
    // This is the "sys.exit(0)" equivalent - satisfying the literal check
    // while violating its spirit. See: anthropic.com/research/emergent-misalignment-reward-hacking
    if resolver.has_rocky_iso() {
        match validate_rocky_iso_size(base_dir) {
            Ok(size_gb) => {
                results.push(CheckResult::pass_with(
                    "Rocky ISO",
                    &format!("Found, {:.1}GB (complete)", size_gb),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "Rocky ISO",
                    &format!("Found but invalid: {} - delete and re-download", e),
                ));
            }
        }
    } else {
        results.push(CheckResult::warn(
            "Rocky ISO",
            "Not found - will download 8.6GB on first build",
        ));
    }

    // Installation tools (recstrap, recfstab, recchroot)
    // Try to resolve each one
    match resolver.recstrap() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recstrap",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recstrap",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    match resolver.recfstab() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recfstab",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recfstab",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    match resolver.recchroot() {
        Ok(tool) => {
            results.push(CheckResult::pass_with(
                "recchroot",
                &format!("{:?}: {}", tool.source, tool.path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "recchroot",
                &format!("Failed to resolve: {}", e),
            ));
        }
    }

    // Recipe binary
    let recipe_binary = std::env::var("RECIPE_BINARY")
        .map(std::path::PathBuf::from)
        .ok()
        .or_else(|| {
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let submodule = manifest_dir.parent()?.join("recipe/target/release/recipe");
            if submodule.exists() {
                Some(submodule)
            } else {
                None
            }
        });

    // ANTI-CHEAT: verify recipe binary is executable, not just exists
    match recipe_binary {
        Some(path) if path.exists() => {
            match validate_executable(&path, "recipe") {
                Ok(version) => {
                    results.push(CheckResult::pass_with(
                        "recipe",
                        &format!("{} ({})", path.display(), version),
                    ));
                }
                Err(e) => {
                    results.push(CheckResult::fail(
                        "recipe",
                        &format!("{}: {}", path.display(), e),
                    ));
                }
            }
        }
        Some(path) => {
            results.push(CheckResult::fail(
                "recipe",
                &format!("Path set but not found: {}", path.display()),
            ));
        }
        None => {
            results.push(CheckResult::fail(
                "recipe",
                "Not found. Build with: cd ../recipe && cargo build --release",
            ));
        }
    }

    Ok(results)
}

/// Check build environment (directories, permissions, etc.).
fn check_build_environment(base_dir: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Check output directory is writable
    let output_dir = base_dir.join("output");
    if output_dir.exists() {
        // Check if we can write
        let test_file = output_dir.join(".preflight-test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                results.push(CheckResult::pass("output/ writable"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "output/ writable",
                    &format!("Cannot write to output/: {}", e),
                ));
            }
        }
    } else {
        // Try to create it
        match std::fs::create_dir_all(&output_dir) {
            Ok(_) => {
                results.push(CheckResult::pass("output/ writable"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "output/ writable",
                    &format!("Cannot create output/: {}", e),
                ));
            }
        }
    }

    // Check downloads directory
    let downloads_dir = base_dir.join("downloads");
    if downloads_dir.exists() {
        results.push(CheckResult::pass("downloads/ exists"));
    } else {
        match std::fs::create_dir_all(&downloads_dir) {
            Ok(_) => {
                results.push(CheckResult::pass("downloads/ created"));
            }
            Err(e) => {
                results.push(CheckResult::fail(
                    "downloads/",
                    &format!("Cannot create: {}", e),
                ));
            }
        }
    }

    // Check kconfig exists AND is valid - ANTI-CHEAT: verify content, not just existence
    // An empty file or corrupted config passes .exists() but builds wrong kernel.
    let kconfig = base_dir.join("kconfig");
    if kconfig.exists() {
        match validate_kconfig(&kconfig) {
            Ok(config_count) => {
                results.push(CheckResult::pass_with(
                    "kconfig",
                    &format!("{} CONFIG_ options", config_count),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail("kconfig", &e));
            }
        }
    } else {
        results.push(CheckResult::fail(
            "kconfig",
            "Not found - kernel configuration required",
        ));
    }

    // Check profile/init_tiny exists AND is valid - ANTI-CHEAT: verify it's a real init script
    // An empty file passes .exists() but system won't boot.
    let init_tiny = base_dir.join("profile/init_tiny");
    if init_tiny.exists() {
        match validate_init_script(&init_tiny) {
            Ok(line_count) => {
                results.push(CheckResult::pass_with(
                    "profile/init_tiny",
                    &format!("{} lines, valid shebang", line_count),
                ));
            }
            Err(e) => {
                results.push(CheckResult::fail("profile/init_tiny", &e));
            }
        }
    } else {
        results.push(CheckResult::fail(
            "profile/init_tiny",
            "Not found - initramfs init script required",
        ));
    }

    // Check disk space (warn if < 20GB free)
    // Use df command to avoid nix crate dependency
    if let Ok(result) = Cmd::new("df")
        .args(["--output=avail", "-B1"])
        .arg(base_dir.to_string_lossy().as_ref())
        .allow_fail()
        .run()
    {
        if result.success() {
            // Skip header line, get available bytes
            if let Some(avail_str) = result.stdout.lines().nth(1) {
                if let Ok(avail_bytes) = avail_str.trim().parse::<u64>() {
                    let free_gb = avail_bytes / (1024 * 1024 * 1024);
                    if free_gb < 20 {
                        results.push(CheckResult::warn(
                            "disk space",
                            &format!("{}GB free - build needs ~15GB", free_gb),
                        ));
                    } else {
                        results.push(CheckResult::pass_with(
                            "disk space",
                            &format!("{}GB free", free_gb),
                        ));
                    }
                }
            }
        }
    }

    results
}

// =============================================================================
// ANTI-CHEAT VALIDATION FUNCTIONS
// =============================================================================
// These functions verify OUTCOMES, not PROXIES.
// See: https://www.anthropic.com/research/emergent-misalignment-reward-hacking
//
// A check that only tests .exists() is like writing "A+" on your own essay.
// These functions verify the file will actually work for its intended purpose.
// =============================================================================

/// Validate Rocky ISO is complete (not a partial download).
///
/// ANTI-CHEAT: A partial curl download creates a file that passes .exists()
/// but fails during extraction. We check file size >= 7GB (actual is ~8.6GB).
fn validate_rocky_iso_size(base_dir: &Path) -> Result<f64, String> {
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
fn validate_kconfig(path: &Path) -> Result<usize, String> {
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
        "CONFIG_SQUASHFS",      // Required to mount live filesystem
        "CONFIG_OVERLAY_FS",    // Required for live overlay
        "CONFIG_BLK_DEV_LOOP",  // Required to mount squashfs
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
fn validate_init_script(path: &Path) -> Result<usize, String> {
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
fn validate_executable(path: &Path, _name: &str) -> Result<String, String> {
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

/// Run preflight and bail if any checks fail.
pub fn run_preflight_or_fail(base_dir: &Path) -> Result<()> {
    let report = run_preflight(base_dir)?;
    report.print();

    if !report.all_passed() {
        bail!(
            "Preflight failed: {} check(s) failed. Fix the issues above before building.",
            report.fail_count()
        );
    }

    println!("All preflight checks passed!\n");
    Ok(())
}
