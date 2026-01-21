# Leviso Development Roadmap

This document tracks the step-by-step development of Leviso from a minimal bash shell to a full LevitateOS installation ISO.

## Architecture

The ISO boots directly into an initramfs-based live environment (no squashfs):

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel
│   └── initramfs.img     # Complete live environment
└── EFI/...               # Bootloader
```

**Boot flow:**
1. Kernel + initramfs boot
2. Initramfs IS the live environment (no switch_root needed)
3. Systemd starts, user gets shell

**Installation flow:**
1. Boot ISO → live environment (initramfs)
2. Partition and format target disk
3. Mount target to `/mnt`
4. `recipe bootstrap /mnt` → installs base system
5. Configure (fstab, bootloader, users)
6. Reboot into installed system

---

## Current State

- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts rootfs for sourcing binaries
- [x] Builds initramfs with bash + coreutils
- [x] Creates bootable hybrid BIOS/UEFI ISO
- [x] QEMU test command (`cargo run -- test` / `--gui`)
- [x] efivarfs mounted for UEFI verification
- [x] Systemd as PID 1 (boots to multi-user.target)
- [x] Disk utilities (parted, fdisk, mkfs.ext4, mkfs.fat, mount)
- [x] Base tarball accessible from live environment (mount /dev/sr0 /media/cdrom)
- [ ] recipe bootstrap command

---

## Phase 1: Minimal Shell Environment ✓

**Goal:** Boot to a bash shell

- [x] Boot with isolinux/syslinux (BIOS) and GRUB (UEFI)
- [x] Load kernel and initramfs
- [x] Mount proc, sys, devtmpfs, efivarfs
- [x] Basic coreutils (ls, cat, cp, mv, rm, mkdir, etc.)

---

## Phase 2: Systemd Init ✓

**Goal:** Boot to systemd as PID 1

- [x] Systemd in initramfs
- [x] Basic systemd units for live environment (getty, serial-console, chronyd)
- [x] Boots to multi-user.target
- [ ] Proper shutdown/reboot (untested)

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

## Phase 3.5: Base Tarball Access ✓

**Goal:** Make base tarball accessible from live environment

- [x] ISO exposed as `/dev/sr0` via virtio-scsi CDROM
- [x] Kernel modules: `virtio_scsi`, `cdrom`, `sr_mod`, `isofs`
- [x] User mounts with: `mount /dev/sr0 /media/cdrom`
- [x] Tarball accessible at: `/media/cdrom/levitateos-base.tar.xz`

---

## Phase 4: Recipe Installation

**Goal:** Install base system to target disk using recipe

- [ ] `recipe bootstrap /mnt` installs base system
- [ ] Recipe binary included in initramfs
- [x] Base tarball extractable to target (`tar xpf /media/cdrom/levitateos-base.tar.xz -C /mnt`)
- [ ] Installs base packages (coreutils, systemd, linux, etc.)
- [ ] Copies recipe database to target

---

## Phase 5: System Configuration Tools

**Goal:** Tools to configure the installed system

Note: Many of these tools need to work INSIDE the chroot (from base tarball), not in live env.
The live environment needs: nano (or sed workarounds), and ability to extract tarball.

### 5.1 User Management
- [x] `useradd` - create users (in initramfs)
- [x] `groupadd` - create groups (in initramfs)
- [x] `chpasswd` - set passwords non-interactively (in initramfs)
- [ ] `passwd` - interactive password setting (MISSING from initramfs)

### 5.2 Text Editing
- [ ] `nano` - text editor for config files (MISSING from initramfs)
- [x] `sed` - stream editor (in initramfs)

### 5.3 Locale & Time
- [x] `loadkeys` - keyboard layout
- [x] Keymaps
- [ ] `locale-gen` / `localedef` - generate locales (MISSING from initramfs)
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

**Full installation workflow (TESTED - WORKS):**
```bash
# 1. Disk preparation
lsblk
parted -s /dev/vda mklabel gpt
parted -s /dev/vda mkpart EFI fat32 1MiB 513MiB
parted -s /dev/vda set 1 esp on
parted -s /dev/vda mkpart root ext4 513MiB 100%

mkfs.fat -F32 /dev/vda1
mkfs.ext4 -F /dev/vda2

mount /dev/vda2 /mnt
mkdir -p /mnt/boot
mount /dev/vda1 /mnt/boot

# 2. Mount installation media and extract base system
mkdir -p /media/cdrom
mount /dev/sr0 /media/cdrom
tar xpf /media/cdrom/levitateos-base.tar.xz -C /mnt

# 3. Configure and reboot (see docs)
```

**Still needs work:**
```bash
# recipe binary not yet in initramfs:
recipe bootstrap /mnt
```

---

## Notes

- Rocky Linux is ONLY for sourcing userspace binaries (temporary)
- Kernel should eventually be vanilla from kernel.org
- Initramfs IS the live environment (no squashfs layer)
- `recipe` handles both live queries AND installation to target disk
