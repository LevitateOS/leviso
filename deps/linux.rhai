// Linux kernel source - dependency recipe
//
// Provides: Linux kernel source tree for building
// Output: Kernel source at submodule path or BUILD_DIR/linux
//
// The linux kernel source can come from:
// 1. Git submodule at MONOREPO/linux (preferred)
// 2. Downloaded tarball from kernel.org
//
// This recipe only handles acquisition. Building is done by leviso.

let ctx = #{
    description: "Linux kernel source",
    name: "linux",
    source_path: "/home/vince/Projects/LevitateOS/linux",
    submodule_path: "/home/vince/Projects/LevitateOS/linux",
    tarball_sha256: "",
    tarball_url: "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.12.10.tar.xz",
    version: "6.12.10",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    // First, check for submodule (preferred)
    // BUILD_DIR is leviso/downloads, submodule is at ../../linux (monorepo root)
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "linux");
    ctx.submodule_path = submodule;

    // Check if submodule is populated (has Makefile)
    if is_file(join_path(submodule, "Makefile")) {
        ctx.source_path = submodule;
        return ctx;
    }

    // Check for downloaded source in BUILD_DIR
    let downloaded = join_path(BUILD_DIR, "linux");
    if is_file(join_path(downloaded, "Makefile")) {
        ctx.source_path = downloaded;
        return ctx;
    }

    throw "linux source not acquired";
}

fn acquire(ctx) {
    // Ensure BUILD_DIR exists
    mkdir(BUILD_DIR);

    // First, try to init submodule if it exists but isn't populated
    // BUILD_DIR is leviso/downloads, submodule is at ../../linux (monorepo root)
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "linux");
    ctx.submodule_path = submodule;

    // Check if .git file exists (submodule is registered but not init'd)
    let git_file = join_path(submodule, ".git");
    if exists(git_file) {
        log("Initializing linux submodule...");
        let status = shell_status("git -C " + monorepo + " submodule update --init --depth=1 linux");
        if status == 0 {
            if is_file(join_path(submodule, "Makefile")) {
                ctx.source_path = submodule;
                log("Linux submodule initialized at " + submodule);
                return ctx;
            }
        }
        warn("Submodule init failed, falling back to tarball download");
    }

    // Fallback: download tarball
    let dest = join_path(BUILD_DIR, "linux-" + ctx.version + ".tar.xz");
    let extract_dir = join_path(BUILD_DIR, "linux");

    if !is_file(dest) {
        log("Downloading Linux " + ctx.version + " tarball...");
        let path = download(ctx.tarball_url, dest);
        if path == "" {
            throw "Failed to download linux tarball";
        }
    }

    // Extract tarball
    if !is_dir(extract_dir) {
        log("Extracting Linux source...");
        mkdir(extract_dir);
        // tar extracts to linux-6.12.10/, we want it in linux/
        shell("tar -xf " + dest + " -C " + BUILD_DIR);
        // Rename extracted directory
        let extracted = join_path(BUILD_DIR, "linux-" + ctx.version);
        if is_dir(extracted) {
            mv(extracted, extract_dir);
        }
    }

    if !is_file(join_path(extract_dir, "Makefile")) {
        throw "Linux source extraction failed - no Makefile";
    }

    ctx.source_path = extract_dir;
    log("Linux source ready at " + extract_dir);
    ctx
}

// === BUILD ===
// Not implemented - leviso handles kernel building

fn is_built(ctx) {
    // Kernel building is handled by leviso, not this recipe
    // Just pass through if source is acquired
    if ctx.source_path == "" {
        throw "source not acquired";
    }
    ctx
}

fn build(ctx) {
    // No-op - leviso builds the kernel
    log("Linux kernel building is handled by leviso");
    ctx
}

// === INSTALL ===
// Not implemented - source stays where it is

fn is_installed(ctx) {
    // Source is "installed" if it's acquired
    if ctx.source_path == "" {
        throw "source not acquired";
    }
    ctx
}

fn install(ctx) {
    // Determine source path (same logic as is_acquired)
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "linux");

    // Prefer submodule, fall back to downloaded
    if is_file(join_path(submodule, "Makefile")) {
        ctx.source_path = submodule;
        ctx.submodule_path = submodule;
    } else {
        let downloaded = join_path(BUILD_DIR, "linux");
        if is_file(join_path(downloaded, "Makefile")) {
            ctx.source_path = downloaded;
        }
    }

    log("Linux source at " + ctx.source_path);
    ctx
}
