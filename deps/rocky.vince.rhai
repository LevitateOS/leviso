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
//   mkdir(path)               - Create directory (recursive)
//   rm(path)                  - Remove file or directory
//   mv(src, dst)              - Move/rename file or directory
//
// Commands:
//   shell(cmd)                - Run shell command, throw on failure
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
// Parsing:
//   parse_int(str)            - Parse string to integer

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
    if !exists(ctx.iso_path) {
        throw "ISO missing at " + ctx.iso_path;
    }
    verify_sha256(ctx.iso_path, ctx.sha256);
    ctx
}

fn acquire(ctx) {
    let dest = BUILD_DIR + "/" + ctx.iso_name;

    ctx.sha256 = fetch_sha256(ctx.sha256_url, ctx.iso_name);
    check_disk_space(BUILD_DIR, ctx.size_bytes + ctx.disk_buffer);

    // Try torrent, fallback to HTTP
    let path = torrent(ctx.torrent_url, dest);
    if path == "" {
        path = download(ctx.http_url, dest);
    }
    if path == "" {
        throw "download failed";
    }

    verify_sha256(path, ctx.sha256);
    ctx.iso_path = path;
    ctx
}

// === BUILD ===

fn is_built(ctx) {
    let rootfs_temp = BUILD_DIR + "/rootfs-temp";
    let iso_contents = BUILD_DIR + "/iso-contents";

    if !exists(rootfs_temp + "/usr") {
        throw "rootfs not extracted";
    }
    if !exists(iso_contents + "/BaseOS/Packages") {
        throw "packages not extracted";
    }
    ctx
}

fn build(ctx) {
    let iso_contents = BUILD_DIR + "/iso-contents";
    let rootfs_temp = BUILD_DIR + "/rootfs-temp";

    if ctx.iso_path == "" {
        throw "ISO not acquired";
    }

    // Extract ISO
    if !exists(iso_contents + "/BaseOS") {
        mkdir(iso_contents);
        shell("7z x -y \"" + ctx.iso_path + "\" -o" + iso_contents);
    }

    // Extract squashfs
    let squashfs = iso_contents + "/images/install.img";
    if !exists(squashfs) {
        throw "squashfs not found at: " + squashfs;
    }

    if !exists(rootfs_temp + "/usr") {
        mkdir(rootfs_temp);
        shell("unsquashfs -d " + rootfs_temp + " -f -no-xattrs " + squashfs);
        shell("chmod -R u+rwX " + rootfs_temp);
        // NOTE: RPM merging is done by leviso (src/extract.rs), not the recipe
    }

    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    if ctx.rootfs_path == "" || ctx.packages_path == "" {
        throw "not installed";
    }
    if !exists(ctx.rootfs_path + "/usr") {
        throw "rootfs missing";
    }
    if !exists(ctx.packages_path + "/BaseOS") {
        throw "packages missing";
    }
    ctx
}

fn install(ctx) {
    let rootfs_temp = BUILD_DIR + "/rootfs-temp";
    let iso_contents = BUILD_DIR + "/iso-contents";
    let rootfs_dest = PREFIX + "/rootfs";
    let packages_dest = PREFIX + "/packages";

    mv(rootfs_temp, rootfs_dest);

    mkdir(packages_dest);
    mv(iso_contents + "/BaseOS/Packages", packages_dest + "/BaseOS");
    mv(iso_contents + "/AppStream/Packages", packages_dest + "/AppStream");

    ctx.rootfs_path = rootfs_dest;
    ctx.packages_path = packages_dest;
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    let iso_contents = BUILD_DIR + "/iso-contents";

    if exists(iso_contents) {
        rm(iso_contents);
    }
    if ctx.iso_path != "" && exists(ctx.iso_path) {
        rm(ctx.iso_path);
        ctx.iso_path = "";
    }
    ctx
}

// === UPDATE ===

fn update(ctx) {
    let base_url = "https://download.rockylinux.org/pub/rocky/10/isos/x86_64";

    // Fetch CHECKSUM to find latest dvd1 ISO and its metadata
    let checksum_content = http_get(base_url + "/CHECKSUM");

    // Parse for dvd1 ISO entry (format: "# Rocky-X.Y-x86_64-dvd1.iso: NNNN bytes")
    let iso_info = parse_checksum_for_dvd(checksum_content);

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
    if ctx.rootfs_path != "" && exists(ctx.rootfs_path) {
        rm(ctx.rootfs_path);
    }
    if ctx.packages_path != "" && exists(ctx.packages_path) {
        rm(ctx.packages_path);
    }
    ctx.iso_path = "";
    ctx.rootfs_path = "";
    ctx.packages_path = "";
    ctx
}

// === Helpers ===

private fn parse_checksum_for_dvd(content) {
    // Find line matching: # Rocky-X.Y-x86_64-dvd1.iso: NNNN bytes
    // Then next line: SHA256 (Rocky-X.Y-x86_64-dvd1.iso) = HASH

    let lines = content.split("\n");
    let filename = "";
    let size_bytes = 0;
    let sha256 = "";
    let version = "";

    for i in 0..lines.len() {
        let line = lines[i];

        // Match: # Rocky-10.1-x86_64-dvd1.iso: 9278128128 bytes
        if line.contains("-dvd1.iso:") && line.starts_with("#") {
            // Extract filename and size
            let parts = line.split(": ");
            filename = parts[0].trim_start("#").trim();
            size_bytes = parse_int(parts[1].replace(" bytes", ""));

            // Extract version from filename (Rocky-10.1-x86_64-dvd1.iso -> 10.1)
            version = filename.replace("Rocky-", "").replace("-x86_64-dvd1.iso", "");

            // Next line should have SHA256
            if i + 1 < lines.len() {
                let sha_line = lines[i + 1];
                // SHA256 (Rocky-10.1-x86_64-dvd1.iso) = HASH
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

    #{
        filename: filename,
        version: version,
        size_bytes: size_bytes,
        sha256: sha256
    }
}
