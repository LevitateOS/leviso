# Leviso Development Roadmap

This document tracks the step-by-step development of Leviso from a minimal bash shell to a full LevitateOS installation ISO.

## Architecture

The ISO contains two separate components:

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel (live environment)
│   └── initramfs.img     # Live installer environment
├── levitateos-stage3.tar.xz  # Base system to install
└── EFI/...               # Bootloader
```

**Initramfs** = Live installer environment. Small, contains tools for partitioning, formatting, and extracting the stage3.

**Stage3 tarball** = Complete base system. Extracted to target disk during installation. Contains kernel, systemd, coreutils, networking, `recipe` package manager.

**Installation flow:**
1. Boot ISO → live environment (initramfs)
2. Partition and format target disk
3. Mount target to `/mnt`
4. Extract stage3: `tar xpf /run/media/stage3.tar.xz -C /mnt`
5. Configure (fstab, bootloader, users)
6. Reboot into installed system
7. Use `recipe` to install additional packages

---

## Current State

- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts squashfs rootfs
- [x] Builds minimal initramfs with bash + coreutils
- [x] Creates bootable hybrid BIOS/UEFI ISO
- [x] QEMU test command (`cargo run -- test` / `--gui`)
- [x] efivarfs mounted for UEFI verification
- [ ] Systemd as init (in progress)
- [ ] Stage3 tarball generation

---

## Phase 1: Minimal Shell Environment ✓

**Goal:** Boot to a bash shell

- [x] Boot with isolinux/syslinux (BIOS) and GRUB (UEFI)
- [x] Load kernel and initramfs
- [x] Mount proc, sys, devtmpfs, efivarfs
- [x] Basic coreutils (ls, cat, cp, mv, rm, mkdir, etc.)

---

## Phase 2: Disk Utilities ✓

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

## Phase 3: Systemd Integration (In Progress)

**Goal:** Systemd as init for the live environment

### 3.1 Live Environment
- [ ] Systemd as PID 1 (replacing bash)
- [ ] `systemctl` - service management
- [ ] `journalctl` - view logs
- [ ] `timedatectl` - time/date management
- [ ] Autologin to root shell on tty1

### 3.2 Tools for Target System
- [ ] `chroot` - enter installed system
- [ ] `systemd-machine-id-setup` - initialize machine ID

---

## Phase 4: Stage3 Tarball

**Goal:** Build a complete base system tarball

### 4.1 Stage3 Contents
- [ ] Kernel + initramfs (for installed system)
- [ ] Systemd + essential services
- [ ] Coreutils, bash, essential CLI tools
- [ ] NetworkManager for networking
- [ ] `recipe` package manager
- [ ] Proper `/etc` configuration

### 4.2 Build Process
- [ ] `leviso stage3` command to generate tarball
- [ ] Include stage3 in ISO
- [ ] Verify extraction works

---

## Phase 5: System Configuration Tools

**Goal:** Tools in live environment to configure the installed system

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

**Goal:** Network access in live environment (for troubleshooting, not required for install)

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
- [ ] GNU coreutils from source (or from trusted binary packages)
- [ ] Build binaries from source
- [ ] Remove Rocky dependency entirely

---

## Implementation Order

1. **Phase 3** - Systemd in live environment (current)
2. **Phase 4** - Stage3 tarball generation
3. **Phase 5** - User management & config tools
4. **Phase 6** - Bootloader support
5. **Phase 7** - Networking (optional for offline install)
6. **Phase 8** - Hardware support
7. **Phase 9** - Quality of life
8. **Phase 10** - Build independence

---

## Testing

```bash
# Build and test
cargo run -- build
cargo run -- test --gui

# Or step by step
cargo run -- download
cargo run -- extract
cargo run -- initramfs
cargo run -- stage3      # (future)
cargo run -- iso
cargo run -- test --gui
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

tar xpf /run/media/*/levitateos-stage3.tar.xz -C /mnt

# Configure and reboot...
```

---

## Notes

- Rocky Linux is ONLY for sourcing userspace binaries (temporary)
- Kernel should eventually be vanilla from kernel.org
- Stage3 = offline install, no network required
- `recipe` is for post-install package management
