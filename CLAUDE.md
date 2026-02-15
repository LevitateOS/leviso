# CLAUDE.md - leviso

## What is leviso?

Builds the LevitateOS ISO and rootfs tarball. Downloads Rocky Linux, extracts packages, assembles initramfs, creates bootable ISO.

## What Belongs Here

- ISO/initramfs/rootfs building (`src/artifact/`)
- System component definitions (`src/component/`)
- Build orchestration (`src/build/`)
- Unit tests for leviso internals (`tests/`)

## What Does NOT Belong Here

| Don't put here | Put it in |
|----------------|-----------|
| E2E installation tests | `testing/install-tests/` |
| Boot/partition/user specs | `distro-spec/` |
| User-facing installer tools | `tools/` |
| ELF analysis utilities | `leviso-elf/` |

## Commands

```bash
cargo run -- build              # Full build (download, extract, assemble)
cargo xtask kernels build leviso      # Build kernel (nightly policy)
cargo run -- build rootfs       # Rebuild rootfs image only
cargo run -- build initramfs    # Rebuild tiny initramfs only
cargo run -- build iso          # Rebuild ISO only
cargo run -- run                # Boot ISO in QEMU (GUI, UEFI)
cargo run -- test               # Boot ISO in QEMU (terminal, serial)
cargo run -- test -c "cmd"      # Run command after boot, exit
```

## Directory Structure

```
leviso/
├── downloads/           # Rocky ISO, extracted rootfs (gitignored)
├── output/              # initramfs, ISO, tarball outputs (gitignored)
├── profile/init_tiny    # Init script for initramfs
├── src/
│   ├── artifact/        # initramfs.rs, iso.rs, rootfs.rs
│   ├── build/           # kernel.rs, users.rs, libdeps.rs
│   ├── component/       # System component builders
│   └── common/          # Shared utilities (use these, don't duplicate)
└── tests/               # Unit tests ONLY (not E2E)
```

## Common Mistakes

1. **Creating install tests here** - E2E tests go in `testing/install-tests/`
2. **Hardcoding specs** - Use `distro-spec` crate for boot entries, partitions, etc.
3. **Duplicating utilities** - Check `src/common/` before writing helpers
4. **Piping QEMU output** - Never pipe `cargo run -- test` to tail/head (breaks buffering)

## System Requirements

QEMU testing uses 4GB RAM minimum. Never use toy values like 512MB.
