# CLAUDE.md - Leviso

## Context

Leviso builds the LevitateOS ISO and rootfs tarball. LevitateOS is a **daily driver Linux distribution competing with Arch Linux** - minimal base that users build up via `recipe`.

The rootfs tarball (~70 MB) is intentionally small. Users install firmware, kernels, and applications via `recipe install`. This is NOT an embedded/container OS - it's a desktop/workstation OS.

## System Requirements (MODERN HARDWARE)

**LevitateOS targets modern computers. It will ship with a local LLM. Stop treating it like a toy.**

| Resource | Minimum | QEMU Default |
|----------|---------|--------------|
| RAM | 8 GB | 4 GB |
| Storage | 64 GB SSD | 32 GB |
| CPU | x86-64-v3 (2015+) | Skylake |

**NEVER** use minimal resource values (512MB RAM, etc). This is not an embedded OS.

---

## Commands

```bash
cargo run -- test           # Quick debug (terminal, serial)
cargo run -- test -c "cmd"  # Run command after boot, exit
cargo run -- run            # Full test (QEMU GUI, UEFI only)
```

**Never pipe `cargo run -- test` to tail/head** - breaks output buffering.

## Architecture

```
leviso/
├── downloads/           # Rocky ISO, extracted rootfs (gitignored)
├── output/              # initramfs, ISO outputs (gitignored)
├── profile/init         # Init script (PID 1)
└── src/                 # Rust source
```

## Adding Binaries

Edit `src/initramfs.rs` arrays: `coreutils`, `sbin_utils`. Build copies binary + library deps from Rocky rootfs.

## Critical Rules

1. **Answer questions first** - Don't start coding when user asks "why?"
2. **Rocky 10 is non-negotiable** - Never suggest downgrading
3. **Fix tool usage, not requirements** - QEMU needs `-cpu host`, don't change distro
4. **No host dependencies** - Download everything, never use `/usr/share/...`
5. **Rocky = userspace only** - Kernel must be independent (not a Rocky rebrand)
6. **No false positives** - Missing binary = build fails, not "optional"
