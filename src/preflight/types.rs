//! Preflight check types and report.

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
    pub fn pass(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            details: None,
        }
    }

    pub fn pass_with(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            details: Some(details.to_string()),
        }
    }

    pub fn fail(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Fail,
            details: Some(details.to_string()),
        }
    }

    pub fn warn(name: &str, details: &str) -> Self {
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
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count()
    }

    /// Count of warnings.
    pub fn warn_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warn)
            .count()
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
        let passed = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count();
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
