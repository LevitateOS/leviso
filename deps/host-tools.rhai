// Host build tool dependencies for leviso
//
// This recipe ensures all host tools needed for ISO building are available.
// The actual building happens in leviso-deps.rhai (in distro-builder/recipes/).
//
// Tools provided: mkfs.erofs, xorriso, mkfs.fat, mmd, mcopy, ukify, isoinfo

let deps = ["leviso-deps"];

let ctx = #{
    name: "host-tools",
    description: "LevitateOS host build tool dependencies",
};

fn is_installed(ctx) {
    // Check that all required tools are in PATH (either system or TOOLS_PREFIX)
    let bins = ["mkfs.erofs", "xorriso", "mkfs.fat", "mmd", "mcopy", "ukify", "isoinfo"];
    for bin in bins {
        if shell_status("which " + bin + " 2>/dev/null") != 0 {
            throw bin + " not available";
        }
    }
    ctx
}

fn acquire(ctx) { ctx }
fn install(ctx) { ctx }
