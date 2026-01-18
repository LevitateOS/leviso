// Shared Rocky Linux helpers for recipes
// Usage: import "lib/rocky" as rocky;

/// Find and copy an RPM from the mounted Rocky ISO to BUILD_DIR
/// Requires ROCKY_ISO_MOUNT env var to be set
fn find_rpm(package_name) {
    let mount = env("ROCKY_ISO_MOUNT");
    if mount == "" {
        throw "ROCKY_ISO_MOUNT environment variable not set";
    }

    let search_dirs = [
        `${mount}/Minimal/Packages`,
        `${mount}/BaseOS/Packages`,
        `${mount}/Packages`
    ];

    for dir in search_dirs {
        if !dir_exists(dir) { continue; }

        // Search subdirectories (a-z folders)
        let pattern = `${dir}/*/${package_name}-[0-9]*.rpm`;
        let matches = glob_list(pattern);

        for rpm_path in matches {
            let filename = rpm_path.split("/").pop();
            // Skip -devel, -doc, -debuginfo variants
            if !filename.contains("-devel-") &&
               !filename.contains("-doc-") &&
               !filename.contains("-debuginfo-") {
                print(`     found ${filename}`);
                copy(rpm_path);
                return;
            }
        }
    }

    throw `RPM not found: ${package_name}`;
}

/// Install all RPMs in BUILD_DIR to PREFIX using rpm2cpio
fn install_rpms() {
    let rpms = glob_list(`${BUILD_DIR}/*.rpm`);
    for rpm in rpms {
        print(`     installing ${rpm}`);
        run(`rpm2cpio '${rpm}' | cpio -idmv -D '${PREFIX}'`);
    }
}
