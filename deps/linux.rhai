//! extends: linux-base.rhai
//
// Linux kernel - LevitateOS overlay
//
// Extends linux-base.rhai with:
// - defconfig + scripts/config kconfig merge (instead of cp)
// - UsrMerge: modules installed to /usr/lib/modules (not /lib/modules)
// - Cleanup removes kernel-build dir (not source tarball)
//
// Version injected from distro-spec via --define KERNEL_VERSION, KERNEL_SHA256, KERNEL_LOCALVERSION

let major = KERNEL_VERSION.split(".")[0];
let ctx = #{
    build_dir: "",
    description: "Linux kernel source (LevitateOS " + KERNEL_VERSION + ")",
    kernel_version: KERNEL_VERSION,
    name: "linux",
    source_dir_name: "linux-" + KERNEL_VERSION,
    source_path: "",
    tarball_filename: "linux-" + KERNEL_VERSION + ".tar.xz",
    tarball_url: "https://cdn.kernel.org/pub/linux/kernel/v" + major + ".x/linux-" + KERNEL_VERSION + ".tar.xz",
};

// === HOOKABLE OVERRIDES ===

/// Apply kconfig: defconfig + scripts/config merge (leviso-specific)
fn _apply_kconfig(src, build_dir, kconfig) {
    let config_file = join_path(build_dir, ".config");
    let hash_file = join_path(build_dir, ".config.kconfig-hash");
    let kconfig_hash = trim(shell_output("sha256sum " + kconfig + " | cut -d' ' -f1"));
    let cached_hash = trim(read_file_or_empty(hash_file));

    if cached_hash != kconfig_hash || !is_file(config_file) {
        log("Generating kernel config...");
        shell("make -C " + src + " O=" + build_dir + " x86_64_defconfig");

        log("Applying kconfig options...");
        let merge_cmd = "grep -E '^CONFIG_' " + kconfig + " | while read line; do " +
            src + "/scripts/config --file " + config_file + " --set-val $(echo $line | sed 's/=/ /'); " +
            "done";
        shell(merge_cmd);

        log("Resolving config dependencies...");
        shell("make -C " + src + " O=" + build_dir + " olddefconfig");
        write_file(hash_file, kconfig_hash);
    } else {
        log("Config unchanged, running olddefconfig...");
        shell("make -C " + src + " O=" + build_dir + " olddefconfig");
    }
}

/// Install modules + UsrMerge fix (modules to /usr/lib/modules)
fn _install_modules(staging, source_path, build_dir) {
    let build_arg = "O=" + build_dir;
    let mod_install = "INSTALL_MOD_PATH=" + staging;
    shell("make -C " + source_path + " " + build_arg + " " + mod_install + " modules_install");

    // Fix UsrMerge: make puts modules in /lib/modules, move to /usr/lib/modules
    let lib_modules = join_path(staging, "lib/modules");
    let usr_lib_modules = join_path(staging, "usr/lib/modules");
    if is_dir(lib_modules) {
        mkdir(usr_lib_modules);
        log("  Moving modules to usr/lib/modules...");
        shell("mv " + lib_modules + "/* " + usr_lib_modules + "/ 2>/dev/null || true");
        shell("rm -rf " + join_path(staging, "lib"));
    }
}

/// Clean up modules: remove build/source symlinks from /usr/lib/modules
fn _cleanup_modules(staging, version) {
    let ver_modules = join_path(staging, "usr/lib/modules/" + version);
    if is_dir(ver_modules) {
        shell("rm -f " + join_path(ver_modules, "build") + " " + join_path(ver_modules, "source"));
    }
}

// === BUILD (override: adds modules build step) ===

fn is_built(ctx) {
    if ctx.source_path == "" {
        throw "source not acquired";
    }

    let leviso_dir = dirname(BUILD_DIR);
    let kernel_build = join_path(leviso_dir, "output/kernel-build");
    let bzimage = join_path(kernel_build, "arch/x86/boot/bzImage");

    if !is_file(bzimage) {
        throw "kernel not built - no bzImage";
    }

    let release_file = join_path(kernel_build, "include/config/kernel.release");
    if is_file(release_file) {
        ctx.kernel_version = trim(read_file(release_file));
    }

    ctx.build_dir = kernel_build;
    ctx
}

fn build(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let output_dir = join_path(leviso_dir, "output");
    let kernel_build = join_path(output_dir, "kernel-build");
    let kconfig_path = join_path(leviso_dir, "kconfig");

    let tarball_source = join_path(BUILD_DIR, ctx.source_dir_name);
    if is_file(join_path(tarball_source, "Makefile")) {
        ctx.source_path = tarball_source;
    }

    if ctx.source_path == "" {
        throw "source not acquired - run acquire first";
    }
    if !is_file(kconfig_path) {
        throw "kconfig not found at " + kconfig_path;
    }

    mkdir(output_dir);
    mkdir(kernel_build);

    let src = ctx.source_path;

    _apply_kconfig(src, kernel_build, kconfig_path);

    let cpus = trim(shell_output("nproc"));
    log("Building kernel with " + cpus + " jobs...");
    shell("make -C " + src + " O=" + kernel_build + " -j" + cpus);

    log("Building modules...");
    shell("make -C " + src + " O=" + kernel_build + " -j" + cpus + " modules");

    let release_file = join_path(kernel_build, "include/config/kernel.release");
    ctx.kernel_version = trim(read_file(release_file));
    ctx.build_dir = kernel_build;

    log("Kernel " + ctx.kernel_version + " built successfully");
    ctx
}

// === INSTALL (override: checks /usr/lib/modules + reads kernel.release) ===

fn is_installed(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let staging = join_path(leviso_dir, "output/staging");
    let vmlinuz = join_path(staging, "boot/vmlinuz");

    if !is_file(vmlinuz) {
        throw "kernel not installed - no vmlinuz in staging";
    }

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

    let bzimage = join_path(kernel_build, "arch/x86/boot/bzImage");
    if !is_file(bzimage) {
        throw "kernel not built - run build first";
    }

    let release_file = join_path(kernel_build, "include/config/kernel.release");
    if !is_file(release_file) {
        throw "kernel.release not found - build incomplete";
    }
    ctx.kernel_version = trim(read_file(release_file));

    let tarball_source = join_path(BUILD_DIR, ctx.source_dir_name);
    if is_file(join_path(tarball_source, "Makefile")) {
        ctx.source_path = tarball_source;
    }

    if ctx.source_path == "" {
        throw "No kernel source path available — cannot install modules";
    }

    log("Installing kernel " + ctx.kernel_version + " to staging...");

    let boot_dir = join_path(staging, "boot");
    mkdir(boot_dir);
    mkdir(join_path(staging, "usr/lib/modules/" + ctx.kernel_version));

    let vmlinuz = join_path(boot_dir, "vmlinuz");
    shell("cp " + bzimage + " " + vmlinuz);
    log("  Installed /boot/vmlinuz");

    log("  Installing modules to /usr/lib/modules/" + ctx.kernel_version + "...");
    _install_modules(staging, ctx.source_path, kernel_build);

    _cleanup_modules(staging, ctx.kernel_version);

    ctx.build_dir = kernel_build;
    log("Kernel installed to " + staging);
    ctx
}

// === CLEANUP (override: remove kernel-build dir, not source tarball) ===

fn cleanup(ctx) {
    let leviso_dir = dirname(BUILD_DIR);
    let kernel_build = join_path(leviso_dir, "output/kernel-build");

    if is_dir(kernel_build) {
        log("Removing kernel build artifacts...");
        rm(kernel_build);
        log("Cleaned " + kernel_build);
    } else {
        log("No kernel build artifacts to clean");
    }

    ctx
}

// acquire, remove — inherited from linux-base.rhai
