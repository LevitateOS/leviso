//! Recipe generation from RPM packages.
//!
//! Generates .rhai recipe files from RPM metadata at build time.
//! These recipes track what's installed and enable updates from Rocky mirrors.
//!
//! ## Philosophy
//!
//! For BASE system packages (from Rocky), we generate RPM shim recipes:
//! - Download from Rocky mirrors (not upstream)
//! - Track installed version and files
//! - Enable `recipe update` to check Rocky repodata
//!
//! For USER packages, users write real recipes with upstream sources.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Metadata extracted from an RPM package.
#[derive(Debug, Clone)]
pub struct RpmInfo {
    pub name: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub summary: String,
    pub files: Vec<String>,
    pub requires: Vec<String>,
}


/// Extract metadata from an RPM file.
///
/// Uses rpm command-line tool to query package info.
pub fn extract_rpm_info(rpm_path: &Path) -> Result<RpmInfo> {
    // Query basic info: NAME\tVERSION\tRELEASE\tARCH\tSUMMARY
    let output = Command::new("rpm")
        .args([
            "-qp",
            "--queryformat",
            "%{NAME}\t%{VERSION}\t%{RELEASE}\t%{ARCH}\t%{SUMMARY}",
            rpm_path.to_str().unwrap(),
        ])
        .output()
        .context("Failed to run rpm -qp")?;

    if !output.status.success() {
        anyhow::bail!(
            "rpm query failed for {}: {}",
            rpm_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let info_str = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = info_str.split('\t').collect();
    if parts.len() < 5 {
        anyhow::bail!("Unexpected rpm output format: {}", info_str);
    }

    // Query file list
    let files_output = Command::new("rpm")
        .args(["-qpl", rpm_path.to_str().unwrap()])
        .output()
        .context("Failed to run rpm -qpl")?;

    let files: Vec<String> = if files_output.status.success() {
        String::from_utf8_lossy(&files_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };

    // Query dependencies
    let deps_output = Command::new("rpm")
        .args(["-qpR", rpm_path.to_str().unwrap()])
        .output()
        .context("Failed to run rpm -qpR")?;

    let requires: Vec<String> = if deps_output.status.success() {
        parse_rpm_requires(&String::from_utf8_lossy(&deps_output.stdout))
    } else {
        Vec::new()
    };

    Ok(RpmInfo {
        name: parts[0].to_string(),
        version: parts[1].to_string(),
        release: parts[2].to_string(),
        arch: parts[3].to_string(),
        summary: parts[4].to_string(),
        files,
        requires,
    })
}

/// Parse rpm -qpR output into package names.
///
/// Filters out:
/// - Version constraints (e.g., "glibc >= 2.34")
/// - Library requirements (e.g., "libc.so.6(GLIBC_2.17)")
/// - rpmlib requirements
/// - Config requirements (e.g., "config(bash)")
/// - rtld requirements (e.g., "rtld(GNU_HASH)")
fn parse_rpm_requires(output: &str) -> Vec<String> {
    let mut packages: HashSet<String> = HashSet::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip special requirements
        if line.starts_with("rpmlib(")
            || line.starts_with("config(")
            || line.starts_with("rtld(")
            || line.contains(".so")
            || line.starts_with("/")
            || line.starts_with("(")
            || line.contains("(")  // Skip anything with parentheses (capability markers)
        {
            continue;
        }

        // Take just the package name (before any version constraint)
        let name = line.split_whitespace().next().unwrap_or(line);
        if !name.is_empty() {
            packages.insert(name.to_string());
        }
    }

    let mut result: Vec<String> = packages.into_iter().collect();
    result.sort();
    result
}

/// Generate .rhai recipe content from RPM info.
pub fn generate_recipe(info: &RpmInfo) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Escape strings for Rhai
    let summary_escaped = info.summary.replace('\\', "\\\\").replace('"', "\\\"");

    let mut recipe = String::new();

    // Header
    recipe.push_str(&format!(
        r#"// {} - {}
// Base system package - RPM shim for Rocky Linux 10
// Generated at build time by leviso

let name = "{}";
let version = "{}";
let release = "{}";
let arch = "{}";
let description = "{}";

// Dependencies (other recipes required)
let deps = ["#,
        info.name,
        summary_escaped,
        info.name,
        info.version,
        info.release,
        info.arch,
        summary_escaped
    ));

    // Add dependencies
    for (i, dep) in info.requires.iter().enumerate() {
        if i > 0 {
            recipe.push_str(", ");
        }
        recipe.push_str(&format!("\"{}\"", dep));
    }
    recipe.push_str("];\n\n");

    // Rocky mirror info
    recipe.push_str(
        r#"// Rocky Linux mirror
let mirror = "https://dl.rockylinux.org/pub/rocky/10";
let repo = "BaseOS";

// === STATE ===
"#,
    );

    // State
    recipe.push_str("let installed = true;\n");
    recipe.push_str(&format!(
        "let installed_version = \"{}-{}\";\n",
        info.version, info.release
    ));
    recipe.push_str(&format!("let installed_at = {};  // Build timestamp\n", timestamp));

    // Installed files array
    recipe.push_str("let installed_files = [\n");
    for file in &info.files {
        recipe.push_str(&format!("    \"{}\",\n", file));
    }
    recipe.push_str("];\n\n");

    // Lifecycle functions
    recipe.push_str(&format!(
        r#"// === LIFECYCLE ===

fn acquire() {{
    // Download from Rocky mirror
    let url = `${{mirror}}/${{repo}}/${{arch}}/os/Packages/{}/{}`;
    let filename = `${{name}}-${{version}}-${{release}}.${{arch}}.rpm`;
    download(url + filename);
}}

fn build() {{
    // Extract RPM contents
    extract_rpm(`${{name}}-${{version}}-${{release}}.${{arch}}.rpm`);
}}

fn install() {{
    // Copy to system
    install_tree("usr", "/usr");
    install_tree("etc", "/etc");
}}

fn remove() {{
    for file in installed_files {{
        rm(file);
    }}
}}

fn check_update() {{
    // Phase 2: Check Rocky repodata
    rocky_check_update(name, mirror, repo)
}}
"#,
        info.name.chars().next().unwrap_or('_'),
        "${name}-${version}-${release}.${arch}.rpm"
    ));

    recipe
}

/// Generator for recipe files from extracted RPMs.
pub struct RecipeGenerator {
    /// Directories containing RPM packages
    packages_dirs: Vec<PathBuf>,
    /// Output directory for generated recipes
    output_dir: PathBuf,
}

impl RecipeGenerator {
    /// Create a new recipe generator.
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        Self {
            packages_dirs: Vec::new(),
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    /// Add a packages directory to search for RPMs.
    pub fn with_packages_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.packages_dirs.push(dir.as_ref().to_path_buf());
        self
    }

    /// Generate recipes for the specified packages.
    pub fn generate_packages(&self, packages: &[&str]) -> Result<()> {
        println!("Generating recipes for {} packages...", packages.len());

        // Create output directory
        std::fs::create_dir_all(&self.output_dir)?;

        let mut generated = 0;
        let mut failed = Vec::new();

        for package in packages {
            match self.generate_package(package) {
                Ok(true) => {
                    generated += 1;
                }
                Ok(false) => {
                    println!("  Warning: {} not found", package);
                    failed.push(*package);
                }
                Err(e) => {
                    println!("  ERROR generating {}: {}", package, e);
                    failed.push(*package);
                }
            }
        }

        if !failed.is_empty() {
            println!(
                "  Warning: Failed to generate recipes for {} packages: {}",
                failed.len(),
                failed.join(", ")
            );
        }

        println!("  Generated {}/{} recipes", generated, packages.len());
        Ok(())
    }

    /// Generate recipe for a single package.
    fn generate_package(&self, package_name: &str) -> Result<bool> {
        // Find the RPM
        let rpm_path = match self.find_rpm(package_name)? {
            Some(path) => path,
            None => return Ok(false),
        };

        // Extract metadata
        let info = extract_rpm_info(&rpm_path)?;

        // Generate recipe content
        let content = generate_recipe(&info);

        // Write recipe file
        let recipe_path = self.output_dir.join(format!("{}.rhai", package_name));
        std::fs::write(&recipe_path, content)?;

        println!("  Generated: {}.rhai", package_name);
        Ok(true)
    }

    /// Find an RPM file for the given package name.
    fn find_rpm(&self, package_name: &str) -> Result<Option<PathBuf>> {
        let first_char = package_name
            .chars()
            .next()
            .context("Empty package name")?
            .to_lowercase()
            .next()
            .unwrap();

        for packages_dir in &self.packages_dirs {
            let subdir = packages_dir.join(first_char.to_string());
            if !subdir.exists() {
                continue;
            }

            // Look for matching RPM
            let entries = std::fs::read_dir(&subdir)?;
            for entry in entries {
                let entry = entry?;
                let filename = entry.file_name();
                let filename_str = filename.to_string_lossy();

                // Match package-version.arch.rpm pattern
                let expected_prefix = format!("{}-", package_name);
                if filename_str.starts_with(&expected_prefix) && filename_str.ends_with(".rpm") {
                    // Verify next char is a digit (version number)
                    let rest = &filename_str[expected_prefix.len()..];
                    if rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        return Ok(Some(entry.path()));
                    }
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rpm_requires() {
        let input = r#"
bash
glibc >= 2.34
libc.so.6(GLIBC_2.17)(64bit)
rpmlib(CompressedFileNames) <= 3.0.4-1
config(bash) = 5.2.32-1.el10
rtld(GNU_HASH)
/bin/sh
ncurses-libs
        "#;

        let result = parse_rpm_requires(input);

        assert!(result.contains(&"bash".to_string()));
        assert!(result.contains(&"ncurses-libs".to_string()));

        // Should NOT contain:
        assert!(!result.iter().any(|s| s.contains(".so")));
        assert!(!result.iter().any(|s| s.starts_with("rpmlib")));
        assert!(!result.iter().any(|s| s.starts_with("config(")));
        assert!(!result.iter().any(|s| s.starts_with("rtld(")));
        assert!(!result.iter().any(|s| s.starts_with("/")));
        assert!(!result.iter().any(|s| s.contains("(")));
        // glibc has version constraint so it's included (just package name)
        assert!(result.contains(&"glibc".to_string()));
    }

}
