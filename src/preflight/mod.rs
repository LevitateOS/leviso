//! Preflight checks for LevitateOS build.
//!
//! Validates all dependencies and host tools before starting a build.
//! Run with `leviso preflight` to check everything is ready.

mod dependencies;
mod environment;
mod host_tools;
mod types;
mod validators;

use std::path::Path;

use anyhow::{bail, Result};

pub use types::PreflightReport;

/// Run all preflight checks.
pub fn run_preflight(base_dir: &Path) -> Result<PreflightReport> {
    let mut checks = Vec::new();

    println!("Running preflight checks...\n");

    // =======================================================================
    // Host Tools
    // =======================================================================
    println!("Checking host tools...");
    checks.extend(host_tools::check_host_tools());

    // =======================================================================
    // Dependencies
    // =======================================================================
    println!("Checking dependencies...");
    checks.extend(dependencies::check_dependencies(base_dir)?);

    // =======================================================================
    // Build Environment
    // =======================================================================
    println!("Checking build environment...");
    checks.extend(environment::check_build_environment(base_dir));

    println!();

    Ok(PreflightReport { checks })
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
