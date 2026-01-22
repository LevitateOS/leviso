# leviso

> **STOP. READ. THEN ACT.** E2E installation tests belong in `install-tests/`, NOT in `leviso/tests/`. See `tests/README.md`.

Builds complete bootable LevitateOS ISOs with full hardware support. Extracts pre-built Rocky Linux 10 packages for minute-fast builds instead of hour-long compilations. Produces hybrid UEFI/BIOS ISOs with squashfs live boot and base system tarball for installation.

## Quick Start

```bash
cargo run -- build    # Full build
cargo run -- run      # Test in QEMU
```

## What It Builds

- Kernel + tiny initramfs (busybox, boots squashfs+overlay)
- 125+ utilities (coreutils, editors, networking, compression, system tools)
- Full networking stack: NetworkManager, wpa_supplicant, iproute2
- Complete WiFi firmware: Intel, Atheros, Realtek, Broadcom, MediaTek
- All kernel modules and hardware firmware from Rocky
- SquashFS-compressed live root filesystem
- Base system tarball for installation (levitateos-base.tar.xz)
- UEFI (GRUB) and BIOS (isolinux) boot support

## Boot Architecture

The live ISO uses a custom tiny initramfs (~1MB):
1. GRUB/isolinux loads kernel + initramfs-tiny.cpio.gz
2. Busybox init mounts ISO, finds `filesystem.squashfs`
3. Sets up three-layer overlay:
   - Lower: squashfs (base system, read-only)
   - Middle: /live/overlay (live-specific configs like autologin)
   - Upper: tmpfs (runtime writes)
4. switch_root to overlay, systemd takes over as PID 1

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
Downloads Rocky Linux 10
        ↓
Extracts rootfs + firmware
        ↓
Builds squashfs system image
        ↓
Creates initramfs (dracut)
        ↓
Packages final ISO
```

## Directory Structure

```
downloads/           # Downloaded dependencies (gitignored)
├── rocky.iso        # Rocky Linux 10 ISO
├── iso-contents/    # Extracted ISO (kernel, EFI files)
├── rootfs/          # Extracted squashfs (userspace binaries)
└── syslinux-6.03/   # Syslinux for BIOS boot

output/              # Build outputs (gitignored)
├── initramfs-tiny-root/ # Unpacked initramfs contents
├── initramfs-tiny.cpio.gz # Tiny initramfs (~1MB, busybox + modules)
├── filesystem.squashfs  # Live root filesystem (squashfs-compressed)
├── levitateos-base.tar.xz # Base system tarball for installation
├── efiboot.img      # EFI boot image for ISO
├── iso-root/        # ISO contents before packaging
│   └── live/overlay/    # Live-specific configs (autologin, serial-console)
└── levitateos.iso   # Final bootable ISO

profile/
└── init_tiny        # Init script (busybox, mounts squashfs + 3-layer overlay)
```

## License

MIT
