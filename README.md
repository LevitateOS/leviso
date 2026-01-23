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

1. Downloads Rocky Linux 10 ISO (9.3GB)
2. Extracts rootfs via `unsquashfs`
3. Copies binaries + library dependencies
4. Builds squashfs live filesystem
5. Creates busybox initramfs (~1MB)
6. Packages ISO with xorriso

## What It Produces

| File | Size | Description |
|------|------|-------------|
| `output/levitateos.iso` | ~800MB | Bootable ISO (UEFI + BIOS) |
| `output/filesystem.squashfs` | ~700MB | Compressed root filesystem |
| `output/initramfs-tiny.cpio.gz` | ~1MB | Busybox init + kernel modules |

Sizes are approximate. Actual sizes depend on package selection.

## Usage

```bash
cargo run -- build       # Full build (downloads 9.3GB Rocky ISO on first run)
cargo run -- run         # Boot in QEMU (GUI, UEFI)
cargo run -- preflight   # Check dependencies before building
```

### Build Subcommands

```bash
cargo run -- build squashfs    # Build squashfs only
cargo run -- build initramfs   # Build initramfs only
cargo run -- build iso         # Build ISO only
cargo run -- build kernel      # Build kernel only
```

### Download/Extract

```bash
cargo run -- download rocky    # Fetch Rocky ISO
cargo run -- download linux    # Fetch Linux kernel source
cargo run -- download tools    # Build/fetch recstrap, recfstab, recchroot
cargo run -- extract rocky     # Extract Rocky ISO contents
cargo run -- extract squashfs  # Extract squashfs for inspection
```

### Other Commands

```bash
cargo run -- clean             # Remove build outputs (preserves downloads)
cargo run -- clean all         # Remove everything including downloads
cargo run -- show config       # Show current configuration
cargo run -- show squashfs     # List squashfs contents
```

## Boot Sequence

1. GRUB/isolinux loads kernel + initramfs
2. Busybox init mounts ISO, finds squashfs
3. Creates overlay: squashfs (ro) + tmpfs (rw)
4. `switch_root` to overlay
5. systemd starts as PID 1

## Directory Layout

```
downloads/                       # Cached downloads (gitignored)
├── Rocky-10.1-x86_64-dvd1.iso   # 9.3GB
├── rootfs/                      # Extracted squashfs from Rocky
├── iso-contents/                # EFI files, kernel, etc.
├── linux/                       # Kernel source (git clone)
├── busybox-static               # Static busybox binary
└── syslinux-6.03/               # BIOS bootloader source

output/                          # Build artifacts (gitignored)
├── filesystem.squashfs
├── initramfs-tiny.cpio.gz
├── initramfs-tiny-root/         # Initramfs staging directory
├── squashfs-root/               # Squashfs staging directory
└── levitateos.iso

profile/                         # Live system customization
├── init_tiny                    # Busybox init script
├── etc/                         # Config file overlays
├── live-overlay/                # Files overlaid on squashfs
└── root/                        # Root home directory overlay
```

## Requirements

- Rust (edition 2021)
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
