# CLAUDE.md - Leviso

## Commands

```bash
cargo run -- test           # Quick debug (terminal, serial)
cargo run -- test -c "cmd"  # Run command after boot, exit
cargo run -- run            # Full test (QEMU GUI, UEFI)
cargo run -- run --bios     # BIOS boot
```

**Never pipe `cargo run -- test` to tail/head** - breaks output buffering.

## Architecture

```
leviso/
├── downloads/           # Rocky ISO, rootfs, syslinux (gitignored)
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
