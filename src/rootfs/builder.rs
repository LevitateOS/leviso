//! Rootfs builder implementation.
//!
//! Builds a complete rootfs for LevitateOS.
//!
//! # WARNING: FALSE POSITIVES KILL PROJECTS
//!
//! The verification in this module MUST check what users actually need.
//! DO NOT:
//! - Mark missing binaries as "optional" just because they're missing
//! - Create tests that only check what exists
//! - Let builds succeed when critical components are absent
//! - Use warnings instead of errors for missing requirements
//!
//! A passing test means NOTHING if it doesn't test what matters.
//! See: .teams/KNOWLEDGE_false-positives-testing.md
//!
//! Remember: Developer sees "✓ 83/83 passed", user sees "bash: sudo: command not found"

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::BuildContext;
use super::parts::{binaries, etc, filesystem, pam, recipe, recipe_gen, systemd};
use super::rpm::{find_packages_dirs, RpmExtractor, REQUIRED_PACKAGES};

/// Builder for base system rootfs.
pub struct RootfsBuilder {
    /// ISO contents directory (for RPM extraction)
    iso_contents: Option<PathBuf>,
    /// Source directory containing Rocky rootfs (fallback if no RPMs)
    source_dir: PathBuf,
    /// Output directory
    output_dir: PathBuf,
    /// Optional path to recipe binary
    recipe_binary: Option<PathBuf>,
}

impl RootfsBuilder {
    pub fn new(source_dir: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> Self {
        Self {
            iso_contents: None,
            source_dir: source_dir.as_ref().to_path_buf(),
            output_dir: output_dir.as_ref().to_path_buf(),
            recipe_binary: None,
        }
    }

    /// Set the ISO contents directory for RPM extraction.
    ///
    /// When set, binaries will be extracted from RPM packages instead of
    /// relying on an incomplete minimal rootfs. This is the CORRECT approach.
    pub fn with_iso_contents(mut self, iso_contents: impl AsRef<Path>) -> Self {
        self.iso_contents = Some(iso_contents.as_ref().to_path_buf());
        self
    }

    /// Set the path to the recipe binary.
    pub fn with_recipe(mut self, recipe_binary: impl AsRef<Path>) -> Self {
        self.recipe_binary = Some(recipe_binary.as_ref().to_path_buf());
        self
    }

    /// Build the base system rootfs.
    pub fn build(&self) -> Result<PathBuf> {
        println!("Building base system rootfs...");
        println!("  Output: {}", self.output_dir.display());

        // Create output directory
        fs::create_dir_all(&self.output_dir)?;

        // Create staging directory for final rootfs
        let staging_dir = self.output_dir.join("staging");
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir)?;
        }
        fs::create_dir_all(&staging_dir)?;

        // Determine source directory - extract from RPMs if available
        let source_dir = if let Some(ref iso_contents) = self.iso_contents {
            println!("  ISO contents: {}", iso_contents.display());
            self.extract_rpms(iso_contents)?
        } else {
            println!("  Source: {}", self.source_dir.display());
            // Validate fallback source directory
            if !self.source_dir.exists() {
                anyhow::bail!(
                    "Source directory does not exist: {}\n\
                     Consider using .with_iso_contents() to extract from RPMs instead.",
                    self.source_dir.display()
                );
            }
            self.source_dir.clone()
        };

        // Create build context
        let mut ctx = BuildContext::new(
            source_dir.clone(),
            staging_dir.clone(),
            self.output_dir.clone(),
        );

        if let Some(ref recipe_path) = self.recipe_binary {
            ctx = ctx.with_recipe(recipe_path.clone());
        }

        // Build the rootfs
        self.build_rootfs(&ctx)?;

        // Generate recipes for installed packages (if building from RPMs)
        if let Some(ref iso_contents) = self.iso_contents {
            self.generate_recipes(iso_contents, &staging_dir)?;
        }

        // Create the tarball
        let tarball_path = self.create_tarball(&staging_dir)?;

        // Clean up staging directory
        println!("Cleaning up staging directory...");
        fs::remove_dir_all(&staging_dir)?;

        // Clean up extracted RPMs if we created them
        if self.iso_contents.is_some() {
            let rpm_extracted = self.output_dir.join("rpm-extracted");
            if rpm_extracted.exists() {
                println!("Cleaning up extracted RPMs...");
                fs::remove_dir_all(&rpm_extracted)?;
            }
        }

        println!("Rootfs tarball created: {}", tarball_path.display());
        Ok(tarball_path)
    }

    /// Extract required RPM packages to a staging directory.
    fn extract_rpms(&self, iso_contents: &Path) -> Result<PathBuf> {
        println!("\n=== Extracting RPM packages ===\n");

        let (baseos, appstream) = find_packages_dirs(iso_contents)?;
        let rpm_staging = self.output_dir.join("rpm-extracted");

        // Clean existing extraction
        if rpm_staging.exists() {
            fs::remove_dir_all(&rpm_staging)?;
        }

        // Create extractor with BaseOS packages
        let mut extractor = RpmExtractor::new(&baseos, &rpm_staging);

        // Add AppStream if available (for packages like wget)
        if let Some(appstream_dir) = appstream {
            println!("  Including AppStream packages");
            extractor = extractor.with_packages_dir(appstream_dir);
        }

        extractor.extract_all()?;

        println!("\n=== RPM extraction complete ===\n");
        Ok(rpm_staging)
    }

    /// Generate .rhai recipes for all installed packages.
    ///
    /// Creates recipe files in /etc/recipe/repos/rocky10/ that track
    /// which packages are installed and enable updates from Rocky mirrors.
    fn generate_recipes(&self, iso_contents: &Path, staging: &Path) -> Result<()> {
        println!("\n=== Generating package recipes ===\n");

        let (baseos, appstream) = find_packages_dirs(iso_contents)?;

        // Create output directory for recipes
        let recipes_dir = staging.join("etc/recipe/repos/rocky10");
        fs::create_dir_all(&recipes_dir)?;

        // Create recipe generator
        let mut generator = recipe_gen::RecipeGenerator::new(&recipes_dir)
            .with_packages_dir(&baseos);

        if let Some(appstream_dir) = appstream {
            generator = generator.with_packages_dir(appstream_dir);
        }

        // Generate recipes for all required packages
        generator.generate_packages(REQUIRED_PACKAGES)?;

        println!("\n=== Recipe generation complete ===\n");
        Ok(())
    }

    /// Build the complete rootfs in staging directory.
    fn build_rootfs(&self, ctx: &BuildContext) -> Result<()> {
        println!("\n=== Building rootfs ===\n");

        // 1. Create FHS directory structure
        filesystem::create_fhs_structure(&ctx.staging)?;

        // 2. Create symlinks (must be after dirs but before binaries)
        filesystem::create_symlinks(&ctx.staging)?;

        // 3. Copy shell (bash) first
        binaries::copy_shell(ctx)?;

        // 4. Copy coreutils binaries
        binaries::copy_coreutils(ctx)?;

        // 5. Copy sbin utilities
        binaries::copy_sbin_utils(ctx)?;

        // 6. Copy systemd binaries and setup
        binaries::copy_systemd_binaries(ctx)?;
        binaries::copy_login_binaries(ctx)?;

        // 7. Copy systemd units
        systemd::copy_systemd_units(ctx)?;
        systemd::copy_dbus_symlinks(ctx)?;

        // 8. Set up systemd services
        systemd::setup_getty(ctx)?;
        systemd::setup_serial_console(ctx)?;
        systemd::setup_networkd(ctx)?;
        systemd::set_default_target(ctx)?;
        systemd::setup_dbus(ctx)?;

        // 9. Copy udev rules and tmpfiles
        systemd::copy_udev_rules(ctx)?;
        systemd::copy_tmpfiles(ctx)?;
        systemd::copy_sysctl(ctx)?;

        // 10. Create /etc configuration files
        etc::create_etc_files(ctx)?;
        etc::copy_timezone_data(ctx)?;
        etc::copy_locales(ctx)?;

        // 11. Set up PAM
        pam::setup_pam(ctx)?;
        pam::copy_pam_modules(ctx)?;
        pam::create_security_config(ctx)?;

        // 12. Copy recipe package manager
        recipe::copy_recipe(ctx)?;
        recipe::setup_recipe_config(ctx)?;

        println!("\n=== Rootfs build complete ===\n");
        Ok(())
    }

    /// Create the tarball from the staging directory.
    fn create_tarball(&self, staging: &Path) -> Result<PathBuf> {
        println!("Creating tarball...");

        let tarball_path = self.output_dir.join("levitateos-base.tar.xz");

        // Use tar command for better compatibility and performance
        let status = Command::new("tar")
            .args([
                "-cJf",
                tarball_path.to_str().unwrap(),
                "-C",
                staging.to_str().unwrap(),
                ".",
            ])
            .status()
            .context("Failed to run tar command")?;

        if !status.success() {
            anyhow::bail!("tar command failed with status: {}", status);
        }

        // Print tarball size
        let metadata = fs::metadata(&tarball_path)?;
        let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
        println!("  Tarball size: {:.2} MB", size_mb);

        Ok(tarball_path)
    }
}

/// List contents of an existing tarball.
pub fn list_tarball(path: &Path) -> Result<()> {
    println!("Contents of {}:", path.display());

    let status = Command::new("tar")
        .args(["-tJf", path.to_str().unwrap()])
        .status()
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar command failed with status: {}", status);
    }

    Ok(())
}

/// Extract tarball to a directory for inspection.
pub fn extract_tarball(tarball: &Path, output_dir: &Path) -> Result<()> {
    if !tarball.exists() {
        anyhow::bail!(
            "Tarball not found: {}\nRun 'leviso rootfs' first to build it.",
            tarball.display()
        );
    }

    // Clean and create output directory
    if output_dir.exists() {
        println!("Removing existing {}...", output_dir.display());
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    println!("Extracting {} to {}...", tarball.display(), output_dir.display());

    let status = Command::new("tar")
        .args([
            "-xJf",
            tarball.to_str().unwrap(),
            "-C",
            output_dir.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar extraction failed with status: {}", status);
    }

    // Print summary
    let bin_count = fs::read_dir(output_dir.join("usr/bin"))
        .map(|d| d.count())
        .unwrap_or(0);
    let sbin_count = fs::read_dir(output_dir.join("usr/sbin"))
        .map(|d| d.count())
        .unwrap_or(0);

    println!("\nExtracted rootfs:");
    println!("  {} binaries in /usr/bin", bin_count);
    println!("  {} binaries in /usr/sbin", sbin_count);
    println!("\nInspect at: {}", output_dir.display());

    Ok(())
}

/// Verify tarball contents - checks ALL critical components.
///
/// # ⚠️ WARNING: DO NOT WEAKEN THIS VERIFICATION ⚠️
///
/// This function exists to catch broken builds BEFORE they ship.
///
/// If this verification fails, the CORRECT response is:
/// 1. Fix the build to include the missing files
/// 2. NOT: Remove the check for the missing file
/// 3. NOT: Move the file to an "optional" category
/// 4. NOT: Add an exception "just for now"
///
/// A verification that passes on broken builds is WORSE than no verification.
/// It gives false confidence and lets broken products ship.
///
/// Remember: "✓ 83/83 passed" means NOTHING if those 83 don't include
/// what users actually need.
///
/// Read: .teams/KNOWLEDGE_false-positives-testing.md
pub fn verify_tarball(path: &Path) -> Result<()> {
    println!("Verifying {}...\n", path.display());

    let output = Command::new("tar")
        .args(["-tJf", path.to_str().unwrap()])
        .output()
        .context("Failed to run tar command")?;

    if !output.status.success() {
        anyhow::bail!("tar command failed");
    }

    let contents = String::from_utf8_lossy(&output.stdout);
    let mut missing = Vec::new();
    let mut checked = 0;

    // Critical binaries - SAME list as in binaries.rs
    // ⚠️ DO NOT REMOVE ITEMS FROM THIS LIST JUST BECAUSE THEY'RE MISSING ⚠️
    // If something is missing, FIX THE BUILD, don't weaken the test
    // Note: In Rocky 10, mount/umount/lsblk are in /usr/bin, not /usr/sbin
    let critical_coreutils = [
        "ls", "cat", "cp", "mv", "rm", "mkdir", "rmdir", "touch",
        "chmod", "chown", "ln", "readlink",
        "echo", "head", "tail", "wc", "sort", "cut", "tr", "tee",
        "grep", "find", "xargs",
        "pwd", "uname", "date", "env", "id", "hostname",
        "sleep", "kill", "ps",
        "gzip", "gunzip", "xz", "unxz", "tar",
        "true", "false", "expr",
        "sed",
        "df", "du", "sync",
        "mount", "umount", "lsblk", "findmnt",  // disk utils in /usr/bin
        "systemctl", "journalctl",
    ];

    let critical_sbin = [
        "fsck", "blkid", "losetup",
        "reboot", "shutdown", "poweroff",
        "insmod", "rmmod", "modprobe", "lsmod",
        "chroot", "ldconfig",
        "useradd", "groupadd", "chpasswd",
        "ip", "sysctl",
    ];

    // Check critical coreutils
    println!("Checking critical coreutils...");
    for bin in critical_coreutils {
        let path = format!("./usr/bin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check critical sbin
    println!("Checking critical sbin utilities...");
    for bin in critical_sbin {
        let path = format!("./usr/sbin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    // Check shell
    println!("Checking shell...");
    for path in ["./usr/bin/bash", "./usr/bin/sh"] {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check systemd
    println!("Checking systemd...");
    let systemd_critical = [
        "./usr/lib/systemd/systemd",
        "./usr/sbin/init",
        "./etc/systemd/system/default.target",
    ];
    for path in systemd_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check /etc essentials
    println!("Checking /etc configuration...");
    let etc_critical = [
        "./etc/passwd",
        "./etc/shadow",
        "./etc/group",
        "./etc/os-release",
        "./etc/fstab",
        "./etc/hosts",
    ];
    for path in etc_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check PAM
    println!("Checking PAM...");
    let pam_critical = [
        "./etc/pam.d/system-auth",
        "./etc/pam.d/login",
        "./usr/lib64/security/pam_unix.so",
    ];
    for path in pam_critical {
        checked += 1;
        if !contents.contains(path) {
            missing.push(path.to_string());
        }
    }

    // Check login binaries
    println!("Checking login binaries...");
    for bin in ["agetty", "login", "nologin"] {
        let path = format!("./usr/sbin/{}", bin);
        checked += 1;
        if !contents.contains(&path) {
            missing.push(path);
        }
    }

    println!();
    if missing.is_empty() {
        println!("✓ Verified {}/{} critical files present", checked, checked);
        Ok(())
    } else {
        println!("✗ VERIFICATION FAILED");
        println!("  Missing {}/{} critical files:", missing.len(), checked);
        for file in &missing {
            println!("    - {}", file);
        }
        anyhow::bail!("Tarball is INCOMPLETE - {} critical files missing", missing.len());
    }
}
