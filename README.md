# leviso

ISO builder for LevitateOS. Downloads Rocky Linux 10, extracts packages, builds EROFS rootfs, outputs bootable ISO.

## Status

**Alpha.** Boots in QEMU. Limited bare metal testing.

| Works | Doesn't work / Not tested |
|-------|---------------------------|
| QEMU boot (UEFI + BIOS) | Most bare metal hardware |
| EROFS live root | Custom kernel (uses Rocky's) |
| busybox initramfs | Secure boot |
| Rocky package extraction | Non-x86_64 architectures |

## What It Does

```
Rocky Linux 10 ISO → extract packages → EROFS rootfs → initramfs → bootable ISO
```

1. Downloads Rocky Linux 10 ISO (9.3GB)
2. Extracts rootfs via `unsquashfs`
3. Copies binaries + library dependencies
4. Builds EROFS live filesystem
5. Creates busybox initramfs (~1MB)
6. Packages ISO with xorriso

## What It Produces

| File | Size | Description |
|------|------|-------------|
| `output/levitateos.iso` | ~800MB | Bootable ISO (UEFI + BIOS) |
| `output/filesystem.erofs` | ~700MB | EROFS compressed root filesystem |
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
cargo run -- build rootfs      # Build EROFS rootfs only
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
cargo run -- extract rootfs    # Extract rootfs for inspection
```

### Other Commands

```bash
cargo run -- clean             # Remove build outputs (preserves downloads)
cargo run -- clean all         # Remove everything including downloads
cargo run -- show config       # Show current configuration
cargo run -- show rootfs       # List rootfs contents
```

## Boot Sequence

1. systemd-boot loads UKI from /EFI/Linux/
2. UKI contains kernel + initramfs + cmdline
3. Busybox init mounts ISO, finds EROFS rootfs
4. Creates overlay: EROFS (ro) + tmpfs (rw)
5. `switch_root` to overlay
6. systemd starts as PID 1

Boot entries:
- `levitateos-live.efi` - Normal boot
- `levitateos-emergency.efi` - Emergency shell
- `levitateos-debug.efi` - Debug mode

## Directory Layout

```
downloads/                       # Cached downloads (gitignored)
├── Rocky-10.1-x86_64-dvd1.iso   # 9.3GB
├── rootfs/                      # Extracted squashfs from Rocky
├── iso-contents/                # EFI files, kernel, etc.
├── linux/                       # Kernel source (git clone)
└── busybox-static               # Static busybox binary

output/                          # Build artifacts (gitignored)
├── filesystem.erofs             # EROFS compressed rootfs
├── initramfs-tiny.cpio.gz
├── initramfs-tiny-root/         # Initramfs staging directory
├── rootfs-staging/              # Rootfs staging directory
└── levitateos.iso

profile/                         # Live system customization
├── init_tiny                    # Busybox init script
├── etc/                         # Config file overlays
├── live-overlay/                # Files overlaid on rootfs
└── root/                        # Root home directory overlay
```

## Requirements

- Rust (edition 2021)
- unsquashfs (squashfs-tools)
- xorriso
- mkfs.erofs (erofs-utils 1.8+)
- ukify (systemd-ukify)
- systemd-boot
- 20GB free disk space

## Known Issues

- Uses Rocky's kernel, not custom-built
- No secure boot support
- Kernel module selection is hardcoded
- WiFi firmware included but not tested on real hardware

## License

MIT
