//! Binary operation handlers: Op::Bin, Op::Bins, Op::Bash, Op::SystemdBinaries, Op::SudoLibs

use anyhow::{bail, Result};
use std::fs;

use crate::build::context::BuildContext;
use crate::build::libdeps::{
    copy_bash, copy_binary_with_libs, copy_sbin_binary_with_libs, make_executable,
};
use crate::build::licenses::LicenseTracker;
use crate::component::Dest;

/// Handle Op::Bin: Copy a required binary with libraries
pub fn handle_bin(
    ctx: &BuildContext,
    name: &str,
    dest: &Dest,
    tracker: &LicenseTracker,
) -> Result<()> {
    let found = match dest {
        Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin", Some(tracker))?,
        Dest::Sbin => copy_sbin_binary_with_libs(ctx, name, Some(tracker))?,
    };
    if !found {
        bail!("{} not found", name);
    }
    Ok(())
}

/// Handle Op::Bins: Copy multiple required binaries, report all missing
pub fn handle_bins(
    ctx: &BuildContext,
    names: &[&str],
    dest: &Dest,
    tracker: &LicenseTracker,
) -> Result<()> {
    let mut missing = Vec::new();
    for name in names {
        let found = match dest {
            Dest::Bin => copy_binary_with_libs(ctx, name, "usr/bin", Some(tracker))?,
            Dest::Sbin => copy_sbin_binary_with_libs(ctx, name, Some(tracker))?,
        };
        if !found {
            missing.push(*name);
        }
    }
    if !missing.is_empty() {
        bail!("Missing binaries: {}", missing.join(", "));
    }
    Ok(())
}

/// Handle Op::Bash: Copy bash shell
pub fn handle_bash(ctx: &BuildContext, tracker: &LicenseTracker) -> Result<()> {
    copy_bash(ctx, Some(tracker))?;
    Ok(())
}

/// Handle Op::SystemdBinaries: Copy systemd binaries and related files
pub fn handle_systemd_binaries(
    ctx: &BuildContext,
    binaries: &[&str],
    tracker: &LicenseTracker,
) -> Result<()> {
    // Register systemd for license tracking
    tracker.register_binary("systemd");

    // Copy main systemd binary
    let systemd_src = ctx.source.join("usr/lib/systemd/systemd");
    let systemd_dst = ctx.staging.join("usr/lib/systemd/systemd");
    if systemd_src.exists() {
        fs::create_dir_all(systemd_dst.parent().unwrap())?;
        fs::copy(&systemd_src, &systemd_dst)?;
        make_executable(&systemd_dst)?;
    }

    // Copy helper binaries
    for binary in binaries {
        let src = ctx.source.join("usr/lib/systemd").join(binary);
        let dst = ctx.staging.join("usr/lib/systemd").join(binary);
        if src.exists() {
            fs::copy(&src, &dst)?;
            make_executable(&dst)?;
        }
    }

    // Copy systemd private libraries
    let systemd_lib_src = ctx.source.join("usr/lib64/systemd");
    if systemd_lib_src.exists() {
        fs::create_dir_all(ctx.staging.join("usr/lib64/systemd"))?;
        for entry in fs::read_dir(&systemd_lib_src)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("libsystemd-") && name_str.ends_with(".so") {
                let dst = ctx.staging.join("usr/lib64/systemd").join(&name);
                fs::copy(entry.path(), &dst)?;
            }
        }
    }

    // Copy system-generators (e.g., systemd-fstab-generator)
    let generators_src = ctx.source.join("usr/lib/systemd/system-generators");
    if generators_src.exists() {
        let generators_dst = ctx.staging.join("usr/lib/systemd/system-generators");
        fs::create_dir_all(&generators_dst)?;
        for entry in fs::read_dir(&generators_src)? {
            let entry = entry?;
            let dst = generators_dst.join(entry.file_name());
            if entry.path().is_file() && !dst.exists() {
                fs::copy(entry.path(), &dst)?;
                make_executable(&dst)?;
            }
        }
    }

    Ok(())
}

/// Handle Op::SudoLibs: Copy sudo plugin libraries
pub fn handle_sudo_libs(ctx: &BuildContext, libs: &[&str], tracker: &LicenseTracker) -> Result<()> {
    // Register sudo for license tracking
    tracker.register_binary("sudo");

    let src_dir = ctx.source.join("usr/libexec/sudo");
    let dst_dir = ctx.staging.join("usr/libexec/sudo");

    if !src_dir.exists() {
        bail!("sudo libexec not found at {}", src_dir.display());
    }

    fs::create_dir_all(&dst_dir)?;

    for lib in libs {
        let src = src_dir.join(lib);
        let dst = dst_dir.join(lib);

        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            if dst.exists() || dst.is_symlink() {
                fs::remove_file(&dst)?;
            }
            std::os::unix::fs::symlink(&target, &dst)?;
        } else if src.exists() {
            fs::copy(&src, &dst)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Dest, Op, Phase};
    use leviso_cheat_test::cheat_aware;

    // Import test helpers from parent module
    use super::super::helpers::*;

    #[cheat_aware(
        protects = "Op::Bin fails loudly when binary not found",
        severity = "CRITICAL",
        ease = "EASY",
        cheats = [
            "Return Ok(()) when binary missing",
            "Log warning instead of error",
            "Skip missing binaries silently"
        ],
        consequence = "Initramfs boots but critical commands don't exist - system unusable"
    )]
    #[test]
    fn test_component_missing_required_binary_fails() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        // Create a component that requires a binary that doesn't exist
        let missing_binary_component = Component {
            name: "TestMissingBinary",
            phase: Phase::Binaries,
            ops: &[Op::Bin("nonexistent-binary-xyz", Dest::Bin)],
        };

        // Execute should fail because the binary doesn't exist
        let result = super::super::execute(&ctx, &missing_binary_component, &tracker);

        assert!(
            result.is_err(),
            "Op::Bin should fail when binary is not found, got Ok"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nonexistent-binary-xyz") || err_msg.contains("not found"),
            "Error should mention the missing binary name, got: {}",
            err_msg
        );
    }

    #[cheat_aware(
        protects = "Op::Bins reports ALL missing binaries, not just the first",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Stop at first missing binary",
            "Only report last missing binary",
            "Truncate list of missing binaries"
        ],
        consequence = "Developer fixes one missing binary, rebuild fails with another - wastes iteration time"
    )]
    #[test]
    fn test_component_bins_reports_all_missing() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new();

        // Create a component that requires multiple missing binaries
        let missing_bins_component = Component {
            name: "TestMissingBins",
            phase: Phase::Binaries,
            ops: &[Op::Bins(
                &["missing-alpha", "missing-beta", "missing-gamma"],
                Dest::Bin,
            )],
        };

        let result = super::super::execute(&ctx, &missing_bins_component, &tracker);

        assert!(
            result.is_err(),
            "Op::Bins should fail when binaries missing"
        );

        let err_msg = result.unwrap_err().to_string();

        // All three should be mentioned
        assert!(
            err_msg.contains("missing-alpha"),
            "Error should list missing-alpha, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("missing-beta"),
            "Error should list missing-beta, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("missing-gamma"),
            "Error should list missing-gamma, got: {}",
            err_msg
        );
    }
}
