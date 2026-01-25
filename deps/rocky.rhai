// Rocky Linux DVD ISO - dependency recipe
//
// Provides: extracted Rocky rootfs for leviso to build upon
// Output: PREFIX/rootfs, PREFIX/packages
//
// === Globals (provided by executor) ===
//
// BUILD_DIR  - Temporary build directory for intermediate files
// PREFIX     - Installation prefix (where final artifacts go)
//
// === Helper functions (provided by executor) ===
//
// Filesystem:
//   exists(path)              - Check if path exists
//   is_file(path)             - Check if path is a file
//   is_dir(path)              - Check if path is a directory
//   mkdir(path)               - Create directory (recursive)
//   rm(path)                  - Remove file or directory
//   mv(src, dst)              - Move/rename file or directory
//   cp(src, dst)              - Copy file or directory
//   ln(target, link)          - Create symlink
//   chmod(path, mode)         - Change permissions (mode as octal int, e.g. 0o755)
//   file_size(path)           - Get file size in bytes
//   list_dir(path)            - List directory contents as array of strings
//
// Paths:
//   join_path(a, b)           - Join path components safely
//   basename(path)            - Get filename from path
//   dirname(path)             - Get directory from path
//   extension(path)           - Get file extension (without dot)
//
// Commands:
//   shell(cmd)                - Run shell command, throw on failure
//   shell_output(cmd)         - Run command, return stdout as string
//   shell_status(cmd)         - Run command, return exit code (don't throw)
//
// Network:
//   download(url, dest)       - HTTP download, returns path or ""
//   torrent(url, dest)        - BitTorrent download, returns path or ""
//   http_get(url)             - Fetch URL content as string
//
// Verification:
//   verify_sha256(path, hash) - Verify file checksum, throw on mismatch
//   fetch_sha256(url, name)   - Fetch CHECKSUM file, extract hash for filename
//   check_disk_space(path, n) - Verify n bytes free at path, throw if not
//
// Archive:
//   extract(archive, dest)    - Auto-detect format and extract (tar, zip, 7z, squashfs)
//
// Parsing:
//   parse_int(str)            - Parse string to integer
//
// Version:
//   version_compare(a, b)     - Compare semver: -1 (a<b), 0 (a==b), 1 (a>b)
//   version_newer(a, b)       - Returns true if a > b
//
// Environment:
//   env(name)                 - Get environment variable, "" if not set
//
// File I/O:
//   read_file(path)           - Read file content as string, "" if not exists
//   write_file(path, content) - Write string to file
//
// String:
//   trim(str)                 - Remove leading/trailing whitespace
//
// Logging:
//   log(msg)                  - Print info message
//   warn(msg)                 - Print warning message

let ctx = #{
    // === Config ===
    name: "rocky",
    version: "10.1",
    description: "Rocky Linux rootfs (extracted from DVD ISO)",

    iso_name: "Rocky-10.1-x86_64-dvd1.iso",
    torrent_url: "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.torrent",
    http_url: "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/Rocky-10.1-x86_64-dvd1.iso",
    sha256_url: "https://download.rockylinux.org/pub/rocky/10/isos/x86_64/CHECKSUM",
    sha256: "",
    size_bytes: 9278128128,
    disk_buffer: 2147483648,

    // === State ===
    iso_path: "",
    rootfs_path: "",
    packages_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    if ctx.iso_path == "" {
        throw "not acquired";
    }
    if !is_file(ctx.iso_path) {
        throw "ISO missing at " + ctx.iso_path;
    }

    // Version check: filename must match expected
    let actual_name = basename(ctx.iso_path);
    if actual_name != ctx.iso_name {
        throw "version mismatch: have " + actual_name + ", want " + ctx.iso_name;
    }

    // Integrity check (skip if sha256 not yet fetched)
    if ctx.sha256 != "" {
        verify_sha256(ctx.iso_path, ctx.sha256);
    }

    ctx
}

fn acquire(ctx) {
    let dest = join_path(BUILD_DIR, ctx.iso_name);

    // Ensure BUILD_DIR exists
    mkdir(BUILD_DIR);

    log("Fetching checksum for " + ctx.iso_name);
    ctx.sha256 = fetch_sha256(ctx.sha256_url, ctx.iso_name);

    // Check if ISO already exists and is valid
    if is_file(dest) {
        log("Found existing ISO, verifying checksum...");
        let status = shell_status("sha256sum -c --status <<< '" + ctx.sha256 + "  " + dest + "'");
        if status == 0 {
            log("Existing ISO is valid, skipping download");
            ctx.iso_path = dest;
            return ctx;
        }
        log("Existing ISO is corrupted or wrong version, re-downloading...");
        rm(dest);
    }

    check_disk_space(BUILD_DIR, ctx.size_bytes + ctx.disk_buffer);

    // Try torrent first (faster, distributed), fallback to HTTP
    log("Downloading " + ctx.iso_name + " via torrent...");
    let path = torrent(ctx.torrent_url, dest);

    if path == "" {
        log("Torrent failed, falling back to HTTP...");
        path = download(ctx.http_url, dest);
    }

    if path == "" {
        throw "download failed for " + ctx.iso_name;
    }

    log("Verifying checksum...");
    verify_sha256(path, ctx.sha256);

    ctx.iso_path = path;
    ctx
}

// === BUILD ===

fn is_built(ctx) {
    let rootfs_temp = join_path(BUILD_DIR, "rootfs-temp");
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let version_file = join_path(BUILD_DIR, ".rocky-built-version");

    // Check artifacts exist
    if !is_dir(join_path(rootfs_temp, "usr")) {
        throw "rootfs not extracted";
    }
    if !is_dir(join_path(iso_contents, "BaseOS/Packages")) {
        throw "packages not extracted";
    }

    // Version check
    let built_version = trim(read_file(version_file));
    if built_version == "" {
        throw "version marker missing - rebuild required";
    }
    if built_version != ctx.version {
        throw "version mismatch: built " + built_version + ", want " + ctx.version;
    }

    ctx
}

fn build(ctx) {
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_temp = join_path(BUILD_DIR, "rootfs-temp");
    let version_file = join_path(BUILD_DIR, ".rocky-built-version");

    // Check for stale artifacts from a different version
    let existing_version = trim(read_file(version_file));
    if existing_version != "" && existing_version != ctx.version {
        log("Cleaning stale build artifacts from version " + existing_version);
        if is_dir(iso_contents) { rm(iso_contents); }
        if is_dir(rootfs_temp) { rm(rootfs_temp); }
        rm(version_file);
    }

    // Extract ISO (7z/iso format)
    if !is_dir(join_path(iso_contents, "BaseOS")) {
        log("Extracting ISO to " + iso_contents);
        mkdir(iso_contents);
        extract(ctx.iso_path, iso_contents);
    }

    // Extract squashfs (install.img contains the rootfs)
    let squashfs = join_path(iso_contents, "images/install.img");
    if !is_file(squashfs) {
        throw "squashfs not found at: " + squashfs;
    }

    if !is_dir(join_path(rootfs_temp, "usr")) {
        log("Extracting squashfs to " + rootfs_temp);
        mkdir(rootfs_temp);
        extract(squashfs, rootfs_temp);
        // NOTE: RPM merging is done by leviso (src/extract.rs), not the recipe
    }

    // Write version marker
    write_file(version_file, ctx.version);
    log("Build complete for Rocky " + ctx.version);

    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let version_file = join_path(PREFIX, ".rocky-version");

    if ctx.rootfs_path == "" || ctx.packages_path == "" {
        throw "not installed";
    }

    if !is_dir(join_path(ctx.rootfs_path, "usr")) {
        throw "rootfs missing at " + ctx.rootfs_path;
    }
    if !is_dir(join_path(ctx.packages_path, "BaseOS")) {
        throw "packages missing at " + ctx.packages_path;
    }

    // Version check
    let installed_version = trim(read_file(version_file));
    if installed_version == "" {
        throw "version marker missing - reinstall required";
    }
    if installed_version != ctx.version {
        throw "version mismatch: installed " + installed_version + ", want " + ctx.version;
    }

    ctx
}

fn install(ctx) {
    let rootfs_temp = join_path(BUILD_DIR, "rootfs-temp");
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_dest = join_path(PREFIX, "rootfs");
    let packages_dest = join_path(PREFIX, "packages");
    let version_file = join_path(PREFIX, ".rocky-version");

    // Check for existing installation
    let existing_version = trim(read_file(version_file));
    if existing_version != "" {
        if existing_version == ctx.version {
            log("Rocky " + ctx.version + " already installed, skipping");
            ctx.rootfs_path = rootfs_dest;
            ctx.packages_path = packages_dest;
            return ctx;
        }
        log("Removing existing installation (version " + existing_version + ")");
        if is_dir(rootfs_dest) { rm(rootfs_dest); }
        if is_dir(packages_dest) { rm(packages_dest); }
        rm(version_file);
    }

    log("Installing Rocky " + ctx.version + " to " + PREFIX);

    // Ensure PREFIX exists
    mkdir(PREFIX);

    // Move artifacts to final location
    mv(rootfs_temp, rootfs_dest);

    mkdir(packages_dest);
    mv(join_path(iso_contents, "BaseOS/Packages"), join_path(packages_dest, "BaseOS"));
    mv(join_path(iso_contents, "AppStream/Packages"), join_path(packages_dest, "AppStream"));

    // Write version marker
    write_file(version_file, ctx.version);

    ctx.rootfs_path = rootfs_dest;
    ctx.packages_path = packages_dest;

    log("Install complete");
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_temp = join_path(BUILD_DIR, "rootfs-temp");
    let version_file = join_path(BUILD_DIR, ".rocky-built-version");

    log("Cleaning up build artifacts...");

    if is_dir(iso_contents) {
        rm(iso_contents);
    }
    if is_dir(rootfs_temp) {
        rm(rootfs_temp);
    }
    if ctx.iso_path != "" && is_file(ctx.iso_path) {
        rm(ctx.iso_path);
        ctx.iso_path = "";
    }
    if is_file(version_file) {
        rm(version_file);
    }

    log("Cleanup complete");
    ctx
}

// === UPDATE ===

fn update(ctx) {
    let base_url = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64";

    log("Checking for Rocky Linux updates...");
    let checksum_content = http_get(base_url + "/CHECKSUM");

    let iso_info = parse_checksum_for_dvd(checksum_content);

    // Check if update is available
    if version_newer(iso_info.version, ctx.version) {
        log("Update available: " + ctx.version + " -> " + iso_info.version);
    } else if iso_info.version == ctx.version {
        log("Already at latest version: " + ctx.version);
    } else {
        warn("Remote version " + iso_info.version + " is older than current " + ctx.version);
    }

    // Update ctx with new version info (use string concat for URLs, not join_path)
    ctx.version = iso_info.version;
    ctx.iso_name = iso_info.filename;
    ctx.size_bytes = iso_info.size_bytes;
    ctx.sha256 = iso_info.sha256;
    ctx.torrent_url = base_url + "/" + iso_info.filename.replace(".iso", ".torrent");
    ctx.http_url = base_url + "/" + iso_info.filename;
    ctx.sha256_url = base_url + "/CHECKSUM";

    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let version_file = join_path(PREFIX, ".rocky-version");
    let rootfs_dest = join_path(PREFIX, "rootfs");
    let packages_dest = join_path(PREFIX, "packages");

    // Check if anything is installed
    let installed_version = trim(read_file(version_file));
    if installed_version == "" && !is_dir(rootfs_dest) && !is_dir(packages_dest) {
        log("Nothing to remove - Rocky not installed");
        return ctx;
    }

    log("Removing Rocky " + (if installed_version != "" { installed_version } else { "(unknown version)" }) + "...");

    if is_dir(rootfs_dest) {
        rm(rootfs_dest);
    }
    if is_dir(packages_dest) {
        rm(packages_dest);
    }
    if is_file(version_file) {
        rm(version_file);
    }

    ctx.iso_path = "";
    ctx.rootfs_path = "";
    ctx.packages_path = "";

    log("Remove complete");
    ctx
}

// === Helpers ===

private fn parse_checksum_for_dvd(content) {
    // Parse Rocky CHECKSUM format:
    // # Rocky-10.1-x86_64-dvd1.iso: 9278128128 bytes
    // SHA256 (Rocky-10.1-x86_64-dvd1.iso) = <hash>

    let lines = content.split("\n");
    let filename = "";
    let size_bytes = 0;
    let sha256 = "";
    let version = "";

    for i in 0..lines.len() {
        let line = lines[i];

        if line.contains("-dvd1.iso:") && line.starts_with("#") {
            // Extract filename and size from: # Rocky-10.1-x86_64-dvd1.iso: 9278128128 bytes
            let parts = line.split(": ");
            filename = parts[0].trim_start("#").trim();
            size_bytes = parse_int(parts[1].replace(" bytes", ""));

            // Extract version: Rocky-10.1-x86_64-dvd1.iso -> 10.1
            version = filename.replace("Rocky-", "").replace("-x86_64-dvd1.iso", "");

            // Next line has SHA256
            if i + 1 < lines.len() {
                let sha_line = lines[i + 1];
                if sha_line.starts_with("SHA256") {
                    sha256 = sha_line.split(" = ")[1].trim();
                }
            }
            break;
        }
    }

    if filename == "" {
        throw "Could not find dvd1 ISO in CHECKSUM";
    }
    if sha256 == "" {
        throw "Could not find SHA256 hash for " + filename + " in CHECKSUM";
    }
    if sha256.len() != 64 {
        throw "Invalid SHA256 hash length for " + filename + ": expected 64, got " + sha256.len();
    }
    if version == "" {
        throw "Could not extract version from filename: " + filename;
    }
    if size_bytes <= 0 {
        throw "Invalid size_bytes for " + filename + ": " + size_bytes;
    }

    #{
        filename: filename,
        version: version,
        size_bytes: size_bytes,
        sha256: sha256
    }
}
