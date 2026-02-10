//! Systemd operation handlers: Op::Units, Op::UserUnits, Op::Enable, Op::DbusSymlinks, Op::UdevHelpers

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::build::context::BuildContext;
use crate::build::libdeps::copy_systemd_units;
use crate::component::Target;
use anyhow::Result;
use leviso_elf::create_symlink_if_missing;

/// Handle Op::Units: Copy systemd unit files
pub fn handle_units(ctx: &BuildContext, names: &[&str]) -> Result<()> {
    copy_systemd_units(ctx, names)?;
    Ok(())
}

/// Handle Op::UserUnits: Copy user-level systemd units
pub fn handle_user_units(ctx: &BuildContext, names: &[&str]) -> Result<()> {
    let src_dir = ctx.source.join("usr/lib/systemd/user");
    let dst_dir = ctx.staging.join("usr/lib/systemd/user");
    fs::create_dir_all(&dst_dir)?;

    for name in names {
        let src = src_dir.join(name);
        let dst = dst_dir.join(name);
        if src.exists() {
            fs::copy(&src, &dst)?;
        } else if src.is_symlink() {
            let target = fs::read_link(&src)?;
            create_symlink_if_missing(&target, &dst)?;
        }
    }

    Ok(())
}

/// Handle Op::Enable: Enable a systemd unit
pub fn handle_enable(ctx: &BuildContext, unit: &str, target: &Target) -> Result<()> {
    let wants_dir = ctx.staging.join(target.wants_dir());
    fs::create_dir_all(&wants_dir)?;
    let link = wants_dir.join(unit);
    create_symlink_if_missing(
        Path::new(&format!("/usr/lib/systemd/system/{}", unit)),
        &link,
    )?;
    Ok(())
}

/// Handle Op::DbusSymlinks: Copy D-Bus symlinks
pub fn handle_dbus_symlinks(ctx: &BuildContext, symlinks: &[&str]) -> Result<()> {
    let unit_src = ctx.source.join("usr/lib/systemd/system");
    let unit_dst = ctx.staging.join("usr/lib/systemd/system");

    for symlink in symlinks {
        let src = unit_src.join(symlink);
        let dst = unit_dst.join(symlink);
        if src.is_symlink() {
            let target = fs::read_link(&src)?;
            create_symlink_if_missing(&target, &dst)?;
        }
    }

    Ok(())
}

/// Handle Op::UdevHelpers: Copy udev helper executables
pub fn handle_udev_helpers(ctx: &BuildContext, helpers: &[&str]) -> Result<()> {
    let udev_src = ctx.source.join("usr/lib/udev");
    let udev_dst = ctx.staging.join("usr/lib/udev");
    fs::create_dir_all(&udev_dst)?;

    for helper in helpers {
        let src = udev_src.join(helper);
        let dst = udev_dst.join(helper);
        if src.exists() && !dst.exists() {
            fs::copy(&src, &dst)?;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Op, Phase, Target};
    use distro_builder::{LicenseTracker, PackageManager};
    use leviso_cheat_test::cheat_aware;
    use std::fs;

    // Import test helpers from parent module
    use super::super::helpers::*;

    #[cheat_aware(
        protects = "Op::Enable creates systemd wants symlink",
        severity = "HIGH",
        ease = "MEDIUM",
        cheats = [
            "Create symlink in wrong directory",
            "Point to wrong unit path",
            "Skip wants directory creation"
        ],
        consequence = "Services not started at boot - network, SSH, etc. don't come up"
    )]
    #[test]
    fn test_component_enable_creates_wants_symlink() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);
        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let enable_component = Component {
            name: "TestEnable",
            phase: Phase::Services,
            ops: &[Op::Enable("test-service.service", Target::MultiUser)],
        };

        let result = super::super::execute(&ctx, &enable_component, &tracker);
        assert!(result.is_ok(), "Op::Enable should succeed: {:?}", result);

        let wants_link = env
            .initramfs
            .join("etc/systemd/system/multi-user.target.wants/test-service.service");
        assert!(
            wants_link.is_symlink(),
            "Wants symlink should exist at {}",
            wants_link.display()
        );

        let target = fs::read_link(&wants_link).expect("Should read symlink");
        assert!(
            target.to_string_lossy().contains("test-service.service"),
            "Should point to the service unit"
        );
    }

    #[cheat_aware(
        protects = "Op::Units copies systemd unit files from rootfs",
        severity = "HIGH",
        ease = "EASY",
        cheats = [
            "Skip units that don't exist",
            "Create empty unit files",
            "Only copy first unit"
        ],
        consequence = "Services don't start - unit files missing"
    )]
    #[test]
    fn test_component_units_copies_unit_files() {
        let env = TestEnv::new();
        create_mock_rootfs(&env.rootfs);

        // Create mock systemd units in rootfs
        let unit_dir = env.rootfs.join("usr/lib/systemd/system");
        fs::write(
            unit_dir.join("test-service.service"),
            "[Unit]\nDescription=Test Service\n[Service]\nExecStart=/bin/true\n",
        )
        .unwrap();
        fs::write(
            unit_dir.join("test-socket.socket"),
            "[Unit]\nDescription=Test Socket\n[Socket]\nListenStream=/run/test.sock\n",
        )
        .unwrap();

        let ctx = env.build_context();
        let tracker = LicenseTracker::new(
            std::path::PathBuf::from("/nonexistent"),
            PackageManager::Rpm,
        );

        let units_component = Component {
            name: "TestUnits",
            phase: Phase::Systemd,
            ops: &[Op::Units(&["test-service.service", "test-socket.socket"])],
        };

        let result = super::super::execute(&ctx, &units_component, &tracker);
        assert!(result.is_ok(), "Op::Units should succeed: {:?}", result);

        // Verify units were copied
        let dst_unit_dir = env.initramfs.join("usr/lib/systemd/system");
        assert_file_exists(&dst_unit_dir.join("test-service.service"));
        assert_file_exists(&dst_unit_dir.join("test-socket.socket"));
        assert_file_contains(&dst_unit_dir.join("test-service.service"), "Test Service");
    }
}
