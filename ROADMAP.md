# Leviso Development Roadmap

This document tracks the step-by-step development of Leviso from a minimal bash shell to a full LevitateOS installation ISO.

## Architecture

The ISO contains a squashfs image that is mounted as an overlay at boot:

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel
│   └── initramfs.img     # Initial ramdisk (mounts squashfs)
├── leviso/
│   └── rootfs.sfs        # Squashfs: complete base system
└── EFI/...               # Bootloader
```

**Boot flow:**
1. Kernel + initramfs boot
2. initramfs mounts squashfs as overlay
3. Switch root to overlay filesystem
4. Systemd starts, user gets shell

**Installation flow:**
1. Boot ISO → live environment (squashfs overlay)
2. Partition and format target disk
3. Mount target to `/mnt`
4. `recipe install base` → installs base system to /mnt
5. Configure (fstab, bootloader, users)
6. Reboot into installed system

---

## Current State

- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts squashfs rootfs
- [x] Builds initramfs with bash + coreutils
- [x] Creates bootable hybrid BIOS/UEFI ISO
- [x] QEMU test command (`cargo run -- test` / `--gui`)
- [x] efivarfs mounted for UEFI verification
- [ ] Squashfs overlay mount (in progress)
- [ ] Systemd as init

---

## Phase 1: Minimal Shell Environment ✓

**Goal:** Boot to a bash shell

- [x] Boot with isolinux/syslinux (BIOS) and GRUB (UEFI)
- [x] Load kernel and initramfs
- [x] Mount proc, sys, devtmpfs, efivarfs
- [x] Basic coreutils (ls, cat, cp, mv, rm, mkdir, etc.)

---

## Phase 2: Squashfs Overlay (Current)

**Goal:** Mount squashfs as overlay filesystem for full base system

- [ ] Create squashfs image from extracted rootfs
- [ ] initramfs mounts squashfs from ISO
- [ ] OverlayFS with tmpfs upper layer (for writes)
- [ ] Switch root to overlay
- [ ] Systemd as PID 1

---

## Phase 3: Disk Utilities ✓

**Goal:** Partition and format disks for installation

- [x] `lsblk` - list block devices
- [x] `blkid` - show UUIDs and labels
- [x] `fdisk` - partition disks
- [x] `parted` - GPT partition table, create partitions
- [x] `wipefs` - wipe filesystem signatures
- [x] `mkfs.ext4` - format root partition
- [x] `mkfs.fat` - format EFI partition
- [x] `mount` / `umount` - mount filesystems

---

## Phase 4: Recipe Installation

**Goal:** Install base system to target disk using recipe

- [ ] `recipe install base` installs to /mnt
- [ ] Recipe reads system recipes from squashfs
- [ ] Copies files to target disk
- [ ] Generates fstab

---

## Phase 5: System Configuration Tools

**Goal:** Tools to configure the installed system

### 5.1 User Management
- [ ] `passwd` - set passwords
- [ ] `useradd` - create users
- [ ] `groupadd` - create groups

### 5.2 Text Editing
- [ ] `nano` - text editor for config files
- [ ] `sed` - stream editor

### 5.3 Locale & Time
- [ ] `loadkeys` - keyboard layout ✓
- [ ] Keymaps ✓
- [ ] `locale-gen` - generate locales
- [ ] Timezone data (`/usr/share/zoneinfo/`)

---

## Phase 6: Bootloader Support

**Goal:** Install bootloader to target system

- [ ] `bootctl` - systemd-boot installer
- [ ] Create `/boot/loader/loader.conf`
- [ ] Create `/boot/loader/entries/*.conf`
- [ ] Mount efivarfs in chroot

---

## Phase 7: Networking (Live Environment)

**Goal:** Network access in live environment

- [ ] `ip` - network interface configuration
- [ ] `ping` - test connectivity
- [ ] `dhcpcd` or `dhclient` - DHCP client
- [ ] Basic DNS resolution

---

## Phase 8: Hardware Support

**Goal:** Support real hardware, not just VMs

### 8.1 Kernel Modules
- [ ] Include essential kernel modules
- [ ] `modprobe` - load modules
- [ ] Storage drivers (SATA, NVMe, USB)

### 8.2 Firmware
- [ ] linux-firmware for WiFi, GPU
- [ ] CPU microcode (intel-ucode, amd-ucode)

### 8.3 Device Detection
- [ ] `udevd` - device manager
- [ ] Automatic module loading

---

## Phase 9: Quality of Life

**Goal:** Pleasant installation experience

- [ ] Proper terminal (no job control warnings)
- [ ] Tab completion, command history
- [ ] Welcome message with install instructions
- [ ] Graceful error messages

---

## Phase 10: Build System Independence

**Goal:** Don't depend on Rocky Linux

- [ ] Vanilla kernel from kernel.org
- [ ] Build binaries from source
- [ ] Remove Rocky dependency entirely

---

## Testing

```bash
# Build and test
cargo run -- build
cargo run -- test

# Or step by step
cargo run -- download
cargo run -- extract
cargo run -- initramfs
cargo run -- iso
cargo run -- test
```

### Installation Test (in QEMU)
```bash
# In live environment:
lsblk
parted /dev/vda mklabel gpt
parted /dev/vda mkpart "EFI" fat32 1MiB 513MiB
parted /dev/vda set 1 esp on
parted /dev/vda mkpart "root" ext4 513MiB 100%

mkfs.fat -F32 /dev/vda1
mkfs.ext4 /dev/vda2

mount /dev/vda2 /mnt
mkdir -p /mnt/boot
mount /dev/vda1 /mnt/boot

recipe install base --root /mnt

# Configure and reboot...
```

---

## Notes

- Rocky Linux is ONLY for sourcing userspace binaries (temporary)
- Kernel should eventually be vanilla from kernel.org
- Squashfs overlay = live environment with full base system
- `recipe` handles both live queries AND installation to target disk
