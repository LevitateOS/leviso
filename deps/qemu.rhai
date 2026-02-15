// QEMU + OVMF for boot testing
//
// Downloads pre-built RPMs from Rocky mirrors.
// Used by `leviso test` and `leviso run`.

let deps = ["qemu-deps"];

let ctx = #{
    description: "QEMU and OVMF for boot testing",
    name: "qemu",
};

fn is_installed(ctx) {
    if shell_status("which qemu-system-x86_64 2>/dev/null") != 0 {
        if shell_status("which qemu-kvm 2>/dev/null") != 0 {
            throw "qemu not available";
        }
    }
    // Check OVMF
    let paths = [
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
    ];
    let found = false;
    for p in paths {
        if is_file(p) { found = true; }
    }
    if !found { throw "OVMF not found"; }
    ctx
}

fn acquire(ctx) { ctx }
fn install(ctx) { ctx }

// === CLEANUP ===
fn cleanup(ctx, reason) { ctx }
