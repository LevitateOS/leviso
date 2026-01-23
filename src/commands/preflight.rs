//! Preflight command - runs preflight checks.

use anyhow::Result;
use std::path::Path;

use crate::preflight;

/// Execute the preflight command.
pub fn cmd_preflight(base_dir: &Path, strict: bool) -> Result<()> {
    if strict {
        preflight::run_preflight_or_fail(base_dir)?;
    } else {
        let report = preflight::run_preflight(base_dir)?;
        report.print();
        if !report.all_passed() {
            println!("Some checks failed. Use --strict to fail the build.");
        }
    }
    Ok(())
}
