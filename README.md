# leviso

Minimal, self-contained bootable Linux ISO builder for LevitateOS.

## Quick Start

```bash
cargo run -- build    # Full build
cargo run -- run      # Test in QEMU
```

## What It Builds

- Kernel + initramfs with systemd, PAM, D-Bus
- ~45 coreutils + ~12 sbin utilities
- UEFI (GRUB) and BIOS (isolinux) boot support

## Commands

| Command | Purpose |
|---------|---------|
| `build` | Full build from scratch |
| `download` | Fetch Rocky 10 ISO |
| `extract` | Extract rootfs |
| `initramfs` | Build initramfs |
| `iso` | Package final ISO |
| `test` | Quick debug (serial console) |
| `run` | Full test (QEMU GUI, UEFI) |

## Architecture

```
Downloads Rocky 10 Minimal
        ↓
Extracts rootfs (squashfs)
        ↓
Copies binaries + libraries
        ↓
Creates initramfs (cpio.gz)
        ↓
Packages final ISO
```

## Directory Structure

```
downloads/           # Downloaded dependencies (gitignored)
├── rocky.iso        # Rocky 10 Minimal ISO
├── iso-contents/    # Extracted ISO (kernel, EFI files)
├── rootfs/          # Extracted squashfs (userspace binaries)
└── syslinux-6.03/   # Syslinux for BIOS boot

output/              # Build outputs (gitignored)
├── initramfs-root/  # Unpacked initramfs contents
├── initramfs.cpio.gz
├── iso-root/        # ISO contents before packaging
└── leviso.iso       # Final bootable ISO

profile/
└── init             # Init script (runs as PID 1)
```

## License

MIT
