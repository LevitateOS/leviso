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

mod tarball;
mod verify;

pub use tarball::{extract_tarball, list_tarball};
pub use verify::verify_tarball;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::context::BuildContext;
use super::parts::{binaries, etc, filesystem, kernel, pam, recipe, recipe_gen, systemd};
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
        let tarball_path = tarball::create_tarball(&staging_dir, &self.output_dir)?;

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

        // Fix permissions: some RPM files (like sudo) are setuid without owner read
        // We need to read them to copy, so ensure all files are readable
        let chmod_output = Command::new("chmod")
            .args(["-R", "+r"])
            .arg(&rpm_staging)
            .output()
            .context("Failed to fix permissions on extracted RPMs")?;

        if !chmod_output.status.success() {
            eprintln!(
                "Warning: chmod failed: {}",
                String::from_utf8_lossy(&chmod_output.stderr)
            );
        }

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

        // 7. Copy sudo support libraries (from auth.rs single source of truth)
        binaries::copy_sudo_libs(ctx)?;

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

        // 13. Copy Linux firmware
        self.copy_firmware(ctx)?;

        // 14. Copy kernel if pre-built (optional - can be built separately)
        kernel::copy_kernel(ctx)?;

        println!("\n=== Rootfs build complete ===\n");
        Ok(())
    }

    /// Copy Linux firmware from extracted RPMs.
    ///
    /// Firmware is required for hardware drivers (network cards, graphics, etc.)
    fn copy_firmware(&self, ctx: &BuildContext) -> Result<()> {
        let src_firmware = ctx.source.join("usr/lib/firmware");
        let dst_firmware = ctx.staging.join("usr/lib/firmware");

        if !src_firmware.exists() {
            println!("Firmware not found in extracted RPMs - skipping");
            println!("  (Install linux-firmware RPM to enable hardware support)");
            return Ok(());
        }

        println!("Copying Linux firmware...");
        fs::create_dir_all(&dst_firmware)?;

        // Use cp -a to preserve symlinks and attributes
        let status = Command::new("cp")
            .args(["-a"])
            .arg(&src_firmware)
            .arg(ctx.staging.join("usr/lib/"))
            .status()
            .context("Failed to copy firmware")?;

        if !status.success() {
            anyhow::bail!("Failed to copy firmware directory");
        }

        // Count firmware files
        let count = fs::read_dir(&dst_firmware)
            .map(|d| d.count())
            .unwrap_or(0);
        println!("  Copied {} firmware directories", count);

        Ok(())
    }
}
