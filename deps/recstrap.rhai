// recstrap - installation tool recipe
//
// Provides: recstrap binary for LevitateOS installation
// Output: Binary in output/staging/usr/bin/recstrap
//
// Phases:
// - acquire: Get source from submodule or download pre-built binary
// - build: Compile with cargo (if source available)
// - install: Copy binary to staging
//
// Source priority:
// 1. Git submodule at MONOREPO/tools/recstrap (preferred - build from source)
// 2. Download pre-built binary from GitHub releases (fallback)

let ctx = #{
    description: "recstrap - LevitateOS system extractor",
    name: "recstrap",
    git_url: "https://github.com/LevitateOS/recstrap.git",
    releases_url: "https://api.github.com/repos/LevitateOS/recstrap/releases/latest",
    binary_name: "recstrap",
    tarball_name: "recstrap-x86_64-linux.tar.gz",
    // State (filled by phases)
    source_path: "",
    binary_path: "",
    from_source: false,
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    // Check for source in submodule
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "tools/recstrap");

    if is_file(join_path(submodule, "Cargo.toml")) {
        ctx.source_path = submodule;
        ctx.from_source = true;
        return ctx;
    }

    // Check for downloaded binary
    let binary = join_path(BUILD_DIR, ctx.binary_name);
    if is_file(binary) {
        ctx.binary_path = binary;
        ctx.from_source = false;
        return ctx;
    }

    throw "recstrap not acquired";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "tools/recstrap");

    // 1. Check if submodule source exists
    if is_file(join_path(submodule, "Cargo.toml")) {
        ctx.source_path = submodule;
        ctx.from_source = true;
        log("Found recstrap source at " + submodule);
        return ctx;
    }

    // 2. Try to init submodule
    let git_file = join_path(submodule, ".git");
    if exists(git_file) {
        log("Initializing recstrap submodule...");
        let status = shell_status("git -C " + monorepo + " submodule update --init tools/recstrap");
        if status == 0 && is_file(join_path(submodule, "Cargo.toml")) {
            ctx.source_path = submodule;
            ctx.from_source = true;
            log("recstrap submodule initialized");
            return ctx;
        }
        warn("Submodule init failed, falling back to binary download");
    }

    // 3. Download pre-built binary from GitHub releases
    log("Downloading recstrap binary from GitHub releases...");

    let tarball = join_path(BUILD_DIR, ctx.tarball_name);
    let binary = join_path(BUILD_DIR, ctx.binary_name);

    // Get latest release info and extract download URL
    // Using curl with GitHub API
    let api_response = shell_output("curl -sL -H 'Accept: application/vnd.github+json' '" + ctx.releases_url + "'");

    // Extract the browser_download_url for our tarball
    // Format: "browser_download_url": "https://..."
    let url_cmd = "echo '" + api_response + "' | grep -o '\"browser_download_url\"[^,]*" + ctx.tarball_name + "[^\"]*' | head -1 | cut -d'\"' -f4";
    let download_url = trim(shell_output(url_cmd));

    if download_url == "" {
        throw "Could not find download URL for " + ctx.tarball_name + " in GitHub releases";
    }

    log("Downloading from: " + download_url);
    shell("curl -L -o " + tarball + " '" + download_url + "'");

    if !is_file(tarball) {
        throw "Download failed - tarball not found";
    }

    // Extract binary from tarball
    log("Extracting binary...");
    shell("tar -xzf " + tarball + " -C " + BUILD_DIR);

    // Clean up tarball
    rm(tarball);

    if !is_file(binary) {
        throw "Extraction failed - binary not found at " + binary;
    }

    // Make executable
    shell("chmod 755 " + binary);

    ctx.binary_path = binary;
    ctx.from_source = false;
    log("recstrap binary downloaded to " + binary);
    ctx
}

// === BUILD ===

fn is_built(ctx) {
    // If from binary download, already built
    if !ctx.from_source && ctx.binary_path != "" {
        if is_file(ctx.binary_path) {
            return ctx;
        }
    }

    // If from source, check for compiled binary (workspace or local target)
    if ctx.from_source && ctx.source_path != "" {
        let leviso_dir = dirname(BUILD_DIR);
        let monorepo = dirname(leviso_dir);

        // Check workspace target first (most common for workspace members)
        let workspace_binary = join_path(monorepo, "target/release/" + ctx.binary_name);
        if is_file(workspace_binary) {
            ctx.binary_path = workspace_binary;
            return ctx;
        }

        // Fallback to crate-local target
        let local_binary = join_path(ctx.source_path, "target/release/" + ctx.binary_name);
        if is_file(local_binary) {
            ctx.binary_path = local_binary;
            return ctx;
        }
    }

    throw "recstrap not built";
}

fn build(ctx) {
    // Determine source path if not set
    if ctx.source_path == "" {
        let leviso_dir = dirname(BUILD_DIR);
        let monorepo = dirname(leviso_dir);
        let submodule = join_path(monorepo, "tools/recstrap");
        if is_file(join_path(submodule, "Cargo.toml")) {
            ctx.source_path = submodule;
            ctx.from_source = true;
        }
    }

    // If we have a pre-built binary, skip build
    if !ctx.from_source {
        let binary = join_path(BUILD_DIR, ctx.binary_name);
        if is_file(binary) {
            ctx.binary_path = binary;
            log("Using pre-built binary at " + binary);
            return ctx;
        }
        throw "No source to build from and no pre-built binary";
    }

    // Build from source
    log("Building " + ctx.binary_name + " from source...");
    log("  Source: " + ctx.source_path);

    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);

    shell("cargo build --release --manifest-path " + join_path(ctx.source_path, "Cargo.toml"));

    // Check workspace target first (most common for workspace members)
    let workspace_binary = join_path(monorepo, "target/release/" + ctx.binary_name);
    if is_file(workspace_binary) {
        ctx.binary_path = workspace_binary;
        log("Built: " + workspace_binary);
        return ctx;
    }

    // Fallback to crate-local target
    let local_binary = join_path(ctx.source_path, "target/release/" + ctx.binary_name);
    if is_file(local_binary) {
        ctx.binary_path = local_binary;
        log("Built: " + local_binary);
        return ctx;
    }

    throw "Build failed - binary not found in workspace or local target";
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let staging = join_path(leviso_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/" + ctx.binary_name);

    if !is_file(installed) {
        throw ctx.binary_name + " not installed";
    }

    ctx
}

fn install(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let staging = join_path(leviso_dir, "output/staging");
    let bin_dir = join_path(staging, "usr/bin");
    let installed = join_path(bin_dir, ctx.binary_name);

    // Determine binary path if not set
    if ctx.binary_path == "" {
        let monorepo = dirname(leviso_dir);

        // Check workspace target first
        let workspace_binary = join_path(monorepo, "target/release/" + ctx.binary_name);
        if is_file(workspace_binary) {
            ctx.binary_path = workspace_binary;
        }

        // Check crate-local target
        if ctx.binary_path == "" && ctx.source_path != "" {
            let local_binary = join_path(ctx.source_path, "target/release/" + ctx.binary_name);
            if is_file(local_binary) {
                ctx.binary_path = local_binary;
            }
        }

        // Check for downloaded binary
        if ctx.binary_path == "" {
            let binary = join_path(BUILD_DIR, ctx.binary_name);
            if is_file(binary) {
                ctx.binary_path = binary;
            }
        }
    }

    if ctx.binary_path == "" || !is_file(ctx.binary_path) {
        throw "No binary to install - run build first";
    }

    log("Installing recstrap to staging...");

    mkdir(bin_dir);
    shell("cp " + ctx.binary_path + " " + installed);
    shell("chmod 755 " + installed);

    log("Installed: " + installed);
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    // Remove downloaded binary (if any)
    let binary = join_path(BUILD_DIR, ctx.binary_name);
    if is_file(binary) {
        log("Removing downloaded binary...");
        rm(binary);
    }

    // Note: We don't clean source (submodule) or staging (final output)
    // Note: We don't clean cargo target dir (expensive to rebuild)

    log("Cleanup complete");
    ctx
}
