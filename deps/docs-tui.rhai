// docs-tui - terminal documentation viewer recipe
//
// Provides: levitate-docs binary for live ISO documentation
// Output: Binary in output/staging/usr/bin/levitate-docs
//
// Phases:
// - acquire: Get source from docs/tui submodule
// - build: Compile with bun (bun build --compile)
// - install: Copy binary to staging
//
// Requirements:
// - bun (JavaScript runtime/bundler)
// - docs/tui submodule initialized

let ctx = #{
    binary_name: "levitate-docs",
    binary_path: "/home/vince/Projects/LevitateOS/docs/tui/levitate-docs",
    description: "levitate-docs - Terminal documentation viewer",
    from_source: true,
    name: "docs-tui",
    source_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let docs_dir = join_path(monorepo, "docs/tui");

    // Check for package.json (indicates source is present)
    if is_file(join_path(docs_dir, "package.json")) {
        ctx.source_path = docs_dir;
        ctx.from_source = true;
        return ctx;
    }

    throw "docs-tui source not acquired";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let docs_dir = join_path(monorepo, "docs/tui");

    // 1. Check if source already exists
    if is_file(join_path(docs_dir, "package.json")) {
        ctx.source_path = docs_dir;
        ctx.from_source = true;
        log("Found docs-tui source at " + docs_dir);
        return ctx;
    }

    // 2. Try to init submodule
    let git_file = join_path(docs_dir, ".git");
    if exists(git_file) || exists(join_path(monorepo, "docs/tui")) {
        log("Initializing docs/tui submodule...");
        let status = shell_status("git -C " + monorepo + " submodule update --init docs/tui");
        if status == 0 && is_file(join_path(docs_dir, "package.json")) {
            ctx.source_path = docs_dir;
            ctx.from_source = true;
            log("docs/tui submodule initialized");
            return ctx;
        }
    }

    throw "docs-tui source not available. Initialize the docs/tui submodule:\n  git submodule update --init docs/tui";
}

// === BUILD ===

fn is_built(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let docs_dir = join_path(monorepo, "docs/tui");

    // Check for compiled binary
    let binary = join_path(docs_dir, ctx.binary_name);
    if is_file(binary) {
        ctx.binary_path = binary;
        ctx.source_path = docs_dir;
        return ctx;
    }

    throw "docs-tui not built";
}

fn build(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let docs_dir = join_path(monorepo, "docs/tui");

    // Ensure source exists
    if !is_file(join_path(docs_dir, "package.json")) {
        throw "docs-tui source not found at " + docs_dir;
    }

    ctx.source_path = docs_dir;

    // Check if bun is available
    let bun_check = shell_status("command -v bun");
    if bun_check != 0 {
        throw "bun not found. Install bun: curl -fsSL https://bun.sh/install | bash";
    }

    log("Building docs-tui with bun...");
    log("  Source: " + docs_dir);

    // Install dependencies first
    log("  Installing dependencies...");
    shell("cd " + docs_dir + " && bun install");

    // Build the standalone binary
    log("  Compiling standalone binary...");
    shell("cd " + docs_dir + " && bun build --compile --minify --outfile " + ctx.binary_name + " src/index.tsx");

    let binary = join_path(docs_dir, ctx.binary_name);
    if !is_file(binary) {
        throw "Build failed - binary not found at " + binary;
    }

    ctx.binary_path = binary;
    log("Built: " + binary);
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
        let docs_dir = join_path(monorepo, "docs/tui");
        let binary = join_path(docs_dir, ctx.binary_name);
        if is_file(binary) {
            ctx.binary_path = binary;
        }
    }

    if ctx.binary_path == "" || !is_file(ctx.binary_path) {
        throw "No binary to install - run build first";
    }

    log("Installing levitate-docs to staging...");

    mkdir(bin_dir);
    shell("cp " + ctx.binary_path + " " + installed);
    shell("chmod 755 " + installed);

    log("Installed: " + installed);
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    // Note: We don't remove the built binary from docs/tui
    // It's useful to keep for faster rebuilds
    log("Cleanup complete (binary preserved in source dir)");
    ctx
}
