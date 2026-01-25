// Rocky Linux ISO dependency with extraction
//
// Phase-separated recipe following recipe design philosophy:
// - acquire() → Download ISO, verify checksum (skip if already done)
// - build()   → Extract ISO, squashfs, merge RPMs (skip if already done)
// - install() → Mark complete (dependency recipes don't install to PREFIX)
//
// External tools required: 7z, unsquashfs, rpm2cpio, cpio
//
// Output: BUILD_DIR/rootfs (extracted Rocky rootfs)

let name = "rocky";
let description = "Rocky Linux rootfs (extracted from DVD ISO)";
let version = "10.1";

// === Rocky 10.1 Configuration ===
let iso_name = "Rocky-10.1-x86_64-dvd1.iso";
let torrent_url = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.torrent";
let http_url = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso";
let sha256 = "bd29df7f8a99b6fc4686f52cbe9b46cf90e07f90be2c0c5f1f18c2ecdd432d34";
let size_bytes = 9278128128;  // ~8.6 GB

/// PHASE 1: Acquire - Download ISO if not already present
///
/// Idempotent: Skips if ISO exists in any of the search locations.
fn acquire() {
    let build_dir = BUILD_DIR;
    let cwd = env("PWD");

    // Check all possible locations for existing ISO
    let iso_path = find_existing_iso(build_dir, cwd);
    if iso_path != "" {
        print("  [acquire] ISO already exists at " + iso_path);
        return;
    }

    // No ISO found - download it
    print("  [acquire] Downloading Rocky Linux ISO...");
    check_disk_space(build_dir, size_bytes + 2147483648);  // ISO + 2GB buffer

    // Try BitTorrent first (faster for large files)
    torrent(torrent_url);

    let expected_path = build_dir + "/" + iso_name;
    if exists(expected_path) {
        print("  [acquire] Verifying ISO checksum...");
        verify_downloaded_sha256(expected_path, sha256);
        return;
    }

    // Fallback to HTTP
    print("  [acquire] Torrent failed, trying HTTP download...");
    download_with_resume(http_url);

    if exists(expected_path) {
        print("  [acquire] Verifying ISO checksum...");
        verify_downloaded_sha256(expected_path, sha256);
        return;
    }

    throw "Failed to download Rocky ISO";
}

/// PHASE 2: Build - Extract ISO and prepare rootfs
///
/// Idempotent: Skips if rootfs already exists with /usr directory.
fn build() {
    let build_dir = BUILD_DIR;
    let cwd = env("PWD");

    let rootfs_dir = build_dir + "/rootfs";
    let iso_contents = build_dir + "/iso-contents";

    // Skip if already built
    if exists(rootfs_dir + "/usr") {
        print("  [build] Rootfs already extracted at " + rootfs_dir);
        return;
    }

    // Find the ISO (acquire() should have downloaded it)
    let iso_path = find_existing_iso(build_dir, cwd);
    if iso_path == "" {
        throw "ISO not found. Run acquire() first.";
    }

    // Step 1: Extract ISO contents with 7z
    if !exists(iso_contents + "/BaseOS") {
        print("  [build] Extracting ISO contents with 7z...");
        mkdir(iso_contents);
        shell("7z x -y \"" + iso_path + "\" -o" + iso_contents);
    } else {
        print("  [build] ISO already extracted to " + iso_contents);
    }

    // Step 2: Extract squashfs to rootfs
    print("  [build] Extracting squashfs to rootfs...");
    let squashfs = iso_contents + "/images/install.img";

    if !exists(squashfs) {
        throw "Could not find squashfs image at: " + squashfs;
    }

    mkdir(rootfs_dir);
    // unsquashfs may warn about xattrs but still succeed
    let result = run_status("unsquashfs -d " + rootfs_dir + " -f -no-xattrs " + squashfs);

    // Verify extraction succeeded
    if !exists(rootfs_dir + "/usr") {
        throw "unsquashfs failed. Install: sudo dnf install squashfs-tools";
    }

    // Fix permissions: unsquashfs preserves root ownership
    print("  [build] Fixing permissions on extracted rootfs...");
    shell("chmod -R u+rwX " + rootfs_dir);

    // Step 3: Merge supplementary RPMs
    merge_rpms(iso_contents, rootfs_dir);

    print("  [build] Rocky rootfs ready at " + rootfs_dir);
}

/// PHASE 3: Install - Mark dependency as resolved
///
/// For dependency recipes, install() verifies the build succeeded.
/// The output path is BUILD_DIR/rootfs by convention.
fn install() {
    let build_dir = BUILD_DIR;
    let rootfs_dir = build_dir + "/rootfs";

    // Verify the build output exists
    if !exists(rootfs_dir + "/usr") {
        throw "Rocky rootfs not found at " + rootfs_dir + "/usr. Run build() first.";
    }

    print("  [install] Rocky Linux dependency ready: " + rootfs_dir);
}

// === Helper Functions ===

/// Find existing ISO in standard locations
fn find_existing_iso(build_dir, cwd) {
    // Priority 1: environment variable (must be absolute path)
    let env_path = env("ROCKY_ISO_PATH");
    if env_path != "" && exists(env_path) {
        return env_path;
    }

    // Priority 2: existing download in build_dir (used by torrent/http)
    let in_build = build_dir + "/" + iso_name;
    if exists(in_build) {
        return in_build;
    }

    // Priority 3: leviso downloads dir (relative to CWD)
    let relative_downloads = "downloads/" + iso_name;
    if exists(relative_downloads) {
        return cwd + "/" + relative_downloads;
    }

    // Priority 4: parent downloads dir (for monorepo structure)
    let parent_downloads = "../downloads/" + iso_name;
    if exists(parent_downloads) {
        return cwd + "/" + parent_downloads;
    }

    // Priority 5: system cache (absolute path)
    let in_cache = "/var/cache/leviso/" + iso_name;
    if exists(in_cache) {
        return in_cache;
    }

    return "";
}

/// Verify SHA256 checksum of a downloaded file
fn verify_downloaded_sha256(file_path, expected) {
    let result = run_output("sha256sum \"" + file_path + "\"");
    let computed = result.split(" ")[0];

    if computed.to_lower() != expected.to_lower() {
        throw "SHA256 mismatch for " + file_path + "\n  expected: " + expected + "\n  got: " + computed;
    }
}

/// Merge supplementary RPMs into the rootfs
fn merge_rpms(iso_contents, rootfs_dir) {
    print("  [build] Merging supplementary RPMs...");

    // === Supplementary RPMs ===
    // The install.img (Anaconda installer) is missing utilities that users expect.
    // These RPMs supplement the installer rootfs with essential utilities.
    let supplementary_rpms = [
        // COREUTILS (many binaries missing from installer)
        "coreutils", "coreutils-common",
        // PROCPS-NG (free, vmstat, uptime, w, watch, ps, top, pgrep, pkill)
        "procps-ng",
        // UTILITIES
        "which", "file", "file-libs", "diffutils", "ncurses", "ncurses-libs",
        // MISSING CORE UTILITIES
        "grep", "findutils", "tar",
        // AUTH
        "sudo", "util-linux", "util-linux-core", "shadow-utils",
        // NETWORK
        "iproute", "iproute-tc",
        // DISK
        "parted",
        // KEYBOARD/LOCALE
        "kbd", "kbd-misc",
        // DRACUT (initramfs generator)
        "dracut", "dracut-config-generic", "dracut-network",
        // GLIBC EXTRAS
        "glibc-common",
        // KMOD
        "kmod",
        // SQUASHFS-TOOLS (for installation)
        "squashfs-tools",
        // FIRMWARE
        "microcode_ctl",
        // SSH
        "openssh-server", "openssh-clients",
        // NETWORK TOOLS
        "NetworkManager-tui", "NetworkManager-wifi", "newt", "slang", "ethtool", "wireless-regdb",
        // HARDWARE SUPPORT
        "bluez", "bluez-libs", "pciutils", "usbutils", "alsa-utils",
        // BTRFS
        "btrfs-progs",
        // WIRELESS
        "iwd",
        // SOUND/GRAPHICS
        "alsa-sof-firmware", "libdrm", "mesa-dri-drivers",
        // TERMINAL TOOLS
        "tmux", "htop", "vim-minimal"
    ];

    // Track installed RPMs to avoid duplicates within this run
    let installed = [];

    for rpm_prefix in supplementary_rpms {
        let rpm_path = find_rpm(iso_contents, rpm_prefix);

        if rpm_path != "" {
            // Extract RPM name for display
            let parts = rpm_path.split("/");
            let rpm_name = parts[parts.len() - 1];
            print("    + " + rpm_name);

            // Extract RPM contents directly into rootfs
            shell("cd \"" + rootfs_dir + "\" && rpm2cpio '" + rpm_path + "' | cpio -idmu --quiet");
            installed.push(rpm_prefix);
        } else {
            // FAIL FAST: if an RPM is in supplementary_rpms, it's REQUIRED
            throw "Required RPM not found for prefix: " + rpm_prefix + "\n\n" +
                  "This RPM is in supplementary_rpms because it provides essential utilities.\n" +
                  "Without it, users will get 'command not found' errors.\n\n" +
                  "Check that the ISO has both BaseOS/Packages and AppStream/Packages.";
        }
    }

    print("  [build] Merged " + installed.len() + " supplementary RPMs");
}

/// Find an RPM file by prefix in the ISO contents
fn find_rpm(iso_contents, prefix) {
    let first_char = prefix.sub_string(0, 1);

    // Search in both BaseOS and AppStream
    let dirs = [
        iso_contents + "/BaseOS/Packages/" + first_char,
        iso_contents + "/AppStream/Packages/" + first_char
    ];

    for dir in dirs {
        if !exists(dir) {
            continue;
        }

        // Try x86_64 architecture first
        let pattern = dir + "/" + prefix + "-*.x86_64.rpm";
        let matches = glob_list(pattern);

        // Filter out -devel and -i18n packages
        for m in matches {
            if !m.contains("-devel") && !m.contains("-i18n") {
                return m;
            }
        }

        // Try noarch architecture
        let pattern = dir + "/" + prefix + "-*.noarch.rpm";
        let matches = glob_list(pattern);

        for m in matches {
            if !m.contains("-devel") && !m.contains("-i18n") {
                return m;
            }
        }
    }

    return "";
}
