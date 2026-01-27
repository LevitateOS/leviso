// Linux kernel - dependency recipe
//
// Provides: Built Linux kernel for LevitateOS
// Output: vmlinuz + modules in output/staging
//
// Phases:
// - acquire: Get source from submodule or clone LevitateOS fork
// - build: Compile kernel using leviso/kconfig
// - install: Copy vmlinuz and modules to staging
//
// Source priority:
// 1. Git submodule at MONOREPO/linux (preferred)
// 2. Git clone from github.com/LevitateOS/linux (fallback)

let ctx = #{
    build_dir: "/home/vince/Projects/LevitateOS/leviso/output/kernel-build",
    description: "Linux kernel source (LevitateOS fork)",
    git_url: "https://github.com/LevitateOS/linux.git",
    kernel_version: "6.19.0-rc6-levitate-gcf38b2340c0e",
    name: "linux",
    source_path: "/home/vince/Projects/LevitateOS/linux",
    submodule_path: "",
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

    // BUILD_DIR is leviso/downloads, submodule is at ../../linux (monorepo root)
    let leviso_dir = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "linux");
    ctx.submodule_path = submodule;

    // First, try to init submodule if it exists but isn't populated
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
        warn("Submodule init failed, falling back to git clone");
    }

    // Fallback: clone from LevitateOS fork
    let clone_dir = join_path(BUILD_DIR, "linux");

    if !is_dir(clone_dir) {
        log("Cloning LevitateOS linux fork...");
        shell("git clone --depth=1 " + ctx.git_url + " " + clone_dir);
    }

    if !is_file(join_path(clone_dir, "Makefile")) {
        throw "Linux source clone failed - no Makefile";
    }

    ctx.source_path = clone_dir;
    log("Linux source ready at " + clone_dir);
    ctx
}

// === BUILD ===
// Compile kernel using leviso's kconfig

fn is_built(ctx) {
    // Check if source is acquired
    if ctx.source_path == "" {
        throw "source not acquired";
    }

    // BUILD_DIR is leviso/downloads, output is at leviso/output
    let leviso_dir = dirname(BUILD_DIR);
    let output_dir = join_path(leviso_dir, "output");
    let kernel_build = join_path(output_dir, "kernel-build");
    let bzimage = join_path(kernel_build, "arch/x86/boot/bzImage");

    if !is_file(bzimage) {
        throw "kernel not built - no bzImage";
    }

    // Get version from kernel.release
    let release_file = join_path(kernel_build, "include/config/kernel.release");
    if is_file(release_file) {
        ctx.kernel_version = trim(read_file(release_file));
    }

    ctx.build_dir = kernel_build;
    ctx
}

fn build(ctx) {
    // Paths
    let leviso_dir = dirname(BUILD_DIR);
    let output_dir = join_path(leviso_dir, "output");
    let kernel_build = join_path(output_dir, "kernel-build");
    let kconfig_path = join_path(leviso_dir, "kconfig");

    // Determine source path (same logic as is_acquired)
    let monorepo = dirname(leviso_dir);
    let submodule = join_path(monorepo, "linux");
    if is_file(join_path(submodule, "Makefile")) {
        ctx.source_path = submodule;
    } else {
        let downloaded = join_path(BUILD_DIR, "linux");
        if is_file(join_path(downloaded, "Makefile")) {
            ctx.source_path = downloaded;
        }
    }

    // Validate
    if ctx.source_path == "" {
        throw "source not acquired - run acquire first";
    }
    if !is_file(kconfig_path) {
        throw "kconfig not found at " + kconfig_path;
    }

    mkdir(output_dir);
    mkdir(kernel_build);

    let src = ctx.source_path;
    let build_arg = "O=" + kernel_build;

    // Check if .config needs regeneration (kconfig changed)
    let config_file = join_path(kernel_build, ".config");
    let hash_file = join_path(kernel_build, ".config.kconfig-hash");
    let kconfig_hash = trim(shell_output("sha256sum " + kconfig_path + " | cut -d' ' -f1"));
    let cached_hash = trim(read_file_or_empty(hash_file));

    if cached_hash != kconfig_hash || !is_file(config_file) {
        log("Generating kernel config...");
        shell("make -C " + src + " " + build_arg + " x86_64_defconfig");

        // Merge kconfig options using shell (Rhai has complexity limits)
        log("Applying kconfig options...");
        let merge_cmd = "grep -E '^CONFIG_' " + kconfig_path + " | while read line; do " +
            src + "/scripts/config --file " + config_file + " --set-val $(echo $line | sed 's/=/ /'); " +
            "done";
        shell(merge_cmd);

        log("Resolving config dependencies...");
        shell("make -C " + src + " " + build_arg + " olddefconfig");
        write_file(hash_file, kconfig_hash);
    } else {
        log("Config unchanged, running olddefconfig...");
        shell("make -C " + src + " " + build_arg + " olddefconfig");
    }

    // Build kernel
    let cpus = trim(shell_output("nproc"));
    log("Building kernel with " + cpus + " jobs...");
    shell("make -C " + src + " " + build_arg + " -j" + cpus);

    // Build modules
    log("Building modules...");
    shell("make -C " + src + " " + build_arg + " -j" + cpus + " modules");

    // Get version
    let release_file = join_path(kernel_build, "include/config/kernel.release");
    ctx.kernel_version = trim(read_file(release_file));
    ctx.build_dir = kernel_build;

    log("Kernel " + ctx.kernel_version + " built successfully");
    ctx
}

// === INSTALL ===
// Install kernel and modules to staging

fn is_installed(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let staging = join_path(leviso_dir, "output/staging");
    let vmlinuz = join_path(staging, "boot/vmlinuz");

    if !is_file(vmlinuz) {
        throw "kernel not installed - no vmlinuz in staging";
    }

    // Check modules exist
    let modules_dir = join_path(staging, "usr/lib/modules");
    if !is_dir(modules_dir) {
        throw "modules not installed";
    }

    ctx
}

fn install(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let output_dir = join_path(leviso_dir, "output");
    let kernel_build = join_path(output_dir, "kernel-build");
    let staging = join_path(output_dir, "staging");

    // Validate build exists
    let bzimage = join_path(kernel_build, "arch/x86/boot/bzImage");
    if !is_file(bzimage) {
        throw "kernel not built - run build first";
    }

    // Get version
    let release_file = join_path(kernel_build, "include/config/kernel.release");
    if !is_file(release_file) {
        throw "kernel.release not found - build incomplete";
    }
    ctx.kernel_version = trim(read_file(release_file));

    // Determine source path
    let leviso_dir2 = dirname(BUILD_DIR);
    let monorepo = dirname(leviso_dir2);
    let submodule = join_path(monorepo, "linux");
    if is_file(join_path(submodule, "Makefile")) {
        ctx.source_path = submodule;
    } else {
        ctx.source_path = join_path(BUILD_DIR, "linux");
    }

    log("Installing kernel " + ctx.kernel_version + " to staging...");

    // Create staging directories
    let boot_dir = join_path(staging, "boot");
    let modules_dir = join_path(staging, "usr/lib/modules/" + ctx.kernel_version);
    mkdir(boot_dir);
    mkdir(modules_dir);

    // Copy kernel
    let vmlinuz = join_path(boot_dir, "vmlinuz");
    shell("cp " + bzimage + " " + vmlinuz);
    log("  Installed /boot/vmlinuz");

    // Install modules
    log("  Installing modules to /usr/lib/modules/" + ctx.kernel_version + "...");
    let build_arg = "O=" + kernel_build;
    let mod_install = "INSTALL_MOD_PATH=" + staging;
    shell("make -C " + ctx.source_path + " " + build_arg + " " + mod_install + " modules_install");

    // Fix UsrMerge: make puts modules in /lib/modules, move to /usr/lib/modules
    let lib_modules = join_path(staging, "lib/modules");
    let usr_lib_modules = join_path(staging, "usr/lib/modules");
    if is_dir(lib_modules) {
        log("  Moving modules to usr/lib/modules...");
        shell("mv " + lib_modules + "/* " + usr_lib_modules + "/ 2>/dev/null || true");
        shell("rm -rf " + join_path(staging, "lib"));
    }

    // Remove build/source symlinks (point to host paths)
    let ver_modules = join_path(usr_lib_modules, ctx.kernel_version);
    shell("rm -f " + join_path(ver_modules, "build") + " " + join_path(ver_modules, "source"));

    ctx.build_dir = kernel_build;
    log("Kernel installed to " + staging);
    ctx
}

// === CLEANUP ===
// Remove build artifacts, preserve installed kernel

fn cleanup(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let output_dir = join_path(leviso_dir, "output");
    let kernel_build = join_path(output_dir, "kernel-build");

    if is_dir(kernel_build) {
        log("Removing kernel build artifacts...");
        rm(kernel_build);
        log("Cleaned " + kernel_build);
    } else {
        log("No kernel build artifacts to clean");
    }

    // NOTE: staging/ is preserved - that's the final output
    // NOTE: source (submodule or downloaded) is preserved

    ctx
}
