# leviso

Minimal bootable Linux ISO builder for LevitateOS.

## Overview

Leviso builds bootable ISO images containing:
- Linux kernel
- Initramfs with userspace binaries (sourced from Rocky 10)
- BIOS bootloader (syslinux/isolinux)
- UEFI bootloader (GRUB EFI)

## Building

```bash
# Full build from scratch
cargo run -- build

# Rebuild initramfs only
cargo run -- initramfs

# Rebuild ISO only
cargo run -- iso
```

## Testing

```bash
# Quick debug (terminal, direct kernel boot)
cargo run -- test

# Run command after boot and exit
cargo run -- test -c "timedatectl"

# Full ISO test (QEMU GUI, UEFI)
cargo run -- run

# Full ISO test (BIOS mode)
cargo run -- run --bios
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
