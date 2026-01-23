# leviso

ISO builder for LevitateOS. Downloads Rocky Linux 10, extracts packages, builds squashfs, outputs bootable ISO.

## Status

**Alpha.** Boots in QEMU. Limited bare metal testing.

| Works | Doesn't work / Not tested |
|-------|---------------------------|
| QEMU boot (UEFI + BIOS) | Most bare metal hardware |
| squashfs live root | Custom kernel (uses Rocky's) |
| busybox initramfs | Secure boot |
| Rocky package extraction | Non-x86_64 architectures |

## What It Does

```
Rocky Linux 10 ISO → extract packages → squashfs → initramfs → bootable ISO
```

1. Downloads Rocky Linux 10 ISO (~2GB)
2. Extracts rootfs via `unsquashfs`
3. Copies binaries + library dependencies
4. Builds squashfs live filesystem
5. Creates busybox initramfs (~1MB)
6. Packages ISO with xorriso

## What It Produces

| File | Size | Description |
|------|------|-------------|
| `levitateos.iso` | ~800MB | Bootable ISO (UEFI + BIOS) |
| `filesystem.squashfs` | ~700MB | Compressed root filesystem |
| `initramfs-tiny.cpio.gz` | ~1MB | Busybox init + kernel modules |
| `levitateos-base.tar.xz` | ~400MB | Tarball for installation |

Sizes are approximate. Actual sizes depend on package selection.

## Usage

```bash
cargo run -- build    # Full build (downloads ~2GB first run)
cargo run -- run      # Boot in QEMU (GUI, UEFI)
cargo run -- test     # Boot in QEMU (serial console)
```

### Individual Steps

```bash
cargo run -- download   # Fetch Rocky ISO
cargo run -- extract    # Extract rootfs
cargo run -- initramfs  # Build initramfs
cargo run -- iso        # Package final ISO
```

## Boot Sequence

1. GRUB/isolinux loads kernel + initramfs
2. Busybox init mounts ISO, finds squashfs
3. Creates overlay: squashfs (ro) + tmpfs (rw)
4. `switch_root` to overlay
5. systemd starts as PID 1

## Directory Layout

```
downloads/           # Cached downloads (gitignored)
├── rocky.iso
├── rootfs/          # Extracted squashfs
└── iso-contents/    # EFI files, kernel

output/              # Build artifacts (gitignored)
├── filesystem.squashfs
├── initramfs-tiny.cpio.gz
├── levitateos-base.tar.xz
└── levitateos.iso

profile/
└── init_tiny        # Busybox init script
```

## Requirements

- Rust 1.75+
- unsquashfs (squashfs-tools)
- xorriso
- mksquashfs
- 20GB free disk space

## Known Issues

- Uses Rocky's kernel, not custom-built
- No secure boot support
- Kernel module selection is hardcoded
- WiFi firmware included but not tested on real hardware

## License

MIT
