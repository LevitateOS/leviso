# Leviso Development Roadmap

This document tracks the step-by-step development of Leviso from a minimal bash shell to a full LevitateOS installation ISO.

## Current State

- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts squashfs rootfs
- [x] Builds minimal initramfs with bash + coreutils
- [x] Creates bootable ISO with isolinux
- [x] QEMU test command (`cargo run -- test` / `--gui`)

---

## Phase 1: Minimal Shell Environment ✓

**Goal:** Boot to a bash shell

- [x] Boot with isolinux/syslinux
- [x] Load kernel and initramfs
- [x] Mount proc, sys, devtmpfs
- [x] Exec bash as init
- [x] Basic coreutils (ls, cat, cp, mv, rm, mkdir, etc.)

---

## Phase 2: Disk Utilities

**Goal:** Partition and format disks for installation

### 2.1 Disk Information
- [x] `lsblk` - list block devices
- [x] `blkid` - show UUIDs and labels
- [x] `fdisk` - partition disks

### 2.2 Partitioning
- [x] `parted` - GPT partition table, create partitions
- [x] `wipefs` - wipe filesystem signatures

### 2.3 Filesystem Creation
- [x] `mkfs.ext4` - format root partition
- [x] `mkfs.fat` (or `mkfs.vfat`) - format EFI partition
- [ ] `e2label` / `fatlabel` - set partition labels (optional)

### 2.4 Mount Operations
- [x] `mount` - mount filesystems
- [x] `umount` - unmount filesystems
- [ ] Support for bind mounts (`mount --bind`) - needs testing

---

## Phase 3: System Configuration Tools

**Goal:** Configure the installed system

### 3.1 User Management
- [ ] `passwd` - set passwords
- [ ] `useradd` - create users
- [ ] `groupadd` - create groups
- [x] `chown` - change ownership
- [x] `chmod` - change permissions

### 3.2 System Configuration
- [x] `chroot` - enter new root
- [x] `hostname` - set hostname
- [x] `ln` - create symlinks
- [x] `hwclock` - sync hardware clock
- [x] `date` - show/set date

### 3.3 Text Editing
- [ ] `nano` or `vi` - text editor for config files
- [ ] `sed` - stream editor

### 3.4 Locale & Time
- [ ] `timedatectl` - time/date management (requires systemd)
- [x] `loadkeys` - keyboard layout
- [x] Keymaps (`/usr/lib/kbd/keymaps/`)
- [ ] `locale-gen` - generate locales
- [ ] Timezone data (`/usr/share/zoneinfo/`)

---

## Phase 4: Networking

**Goal:** Connect to the internet for package downloads

### 4.1 Basic Networking
- [ ] `ip` - network interface configuration
- [ ] `ping` - test connectivity
- [ ] `dhclient` or `dhcpcd` - DHCP client

### 4.2 NetworkManager (for WiFi)
- [ ] `nmcli` - NetworkManager CLI
- [ ] `NetworkManager` daemon
- [ ] WiFi drivers/firmware

### 4.3 DNS Resolution
- [ ] `/etc/resolv.conf` setup
- [ ] `nslookup` or `dig` (optional)

---

## Phase 5: Package Manager Integration

**Goal:** `recipe` package manager in live environment

### 5.1 Recipe Binary
- [ ] Include `recipe` binary in initramfs
- [ ] `recipe bootstrap /mnt` - install base system
- [ ] `recipe install` - install packages

### 5.2 Dependencies
- [ ] `curl` or `wget` - download packages
- [ ] `tar` - extract archives
- [ ] `gzip`/`xz` - decompress

---

## Phase 6: Bootloader Support

**Goal:** Install bootloader to target system

### 6.1 UEFI Boot (Live ISO)
- [ ] Add UEFI boot support to ISO (currently BIOS-only with isolinux)
- [ ] Use OVMF in QEMU for UEFI testing
- [ ] `/sys/firmware/efi/efivars` available when booted UEFI

### 6.2 UEFI Boot (Target System)
- [ ] Mount `/sys/firmware/efi/efivars`
- [ ] `bootctl` - systemd-boot installer
- [ ] EFI system partition support

### 6.3 Boot Configuration
- [ ] Create `/boot/loader/loader.conf`
- [ ] Create `/boot/loader/entries/*.conf`
- [ ] Kernel + initramfs installation

---

## Phase 7: Systemd Integration

**Goal:** Full systemd support for services

### 7.1 Core Systemd
- [ ] `systemctl` - service management
- [ ] `systemd-machine-id-setup` - initialize machine ID
- [ ] `journalctl` - view logs

### 7.2 Services
- [ ] Enable `NetworkManager` on target
- [ ] Enable other essential services

---

## Phase 8: Hardware Support

**Goal:** Support real hardware, not just VMs

### 8.1 Kernel Modules
- [ ] Include essential kernel modules in initramfs
- [ ] `modprobe` - load modules
- [ ] Storage drivers (SATA, NVMe, USB)
- [ ] Filesystem drivers (ext4, vfat)

### 8.2 Firmware
- [ ] Include linux-firmware for WiFi, GPU, etc.
- [ ] CPU microcode (intel-ucode, amd-ucode)

### 8.3 Device Detection
- [ ] `udevd` - device manager
- [ ] Automatic module loading

---

## Phase 9: Quality of Life

**Goal:** Pleasant installation experience

### 9.1 Terminal Experience
- [ ] Proper terminal setup (no job control warnings)
- [ ] Tab completion
- [ ] Command history
- [ ] Colors in output

### 9.2 Documentation
- [ ] Include `/usr/share/doc/leviso/INSTALL.md`
- [ ] Help command or motd with instructions

### 9.3 Error Handling
- [ ] Graceful error messages
- [ ] Recovery shell on failure

---

## Phase 10: Build System Independence

**Goal:** Don't depend on Rocky Linux

### 10.1 Vanilla Kernel
- [ ] Build or download kernel from kernel.org
- [ ] Custom kernel config for live environment

### 10.2 Userspace
- [ ] Use uutils (Rust coreutils) instead of GNU
- [ ] Build binaries from source or use minimal distro

### 10.3 Custom Initramfs
- [ ] Remove dependency on Rocky squashfs
- [ ] Minimal custom rootfs

---

## Implementation Order (Suggested)

1. **Phase 2** - Disk utilities (needed first for installation)
2. **Phase 3.1-3.2** - User management & basic config
3. **Phase 4.1** - Basic networking (dhcp, ping)
4. **Phase 5** - Recipe package manager
5. **Phase 6** - Bootloader support
6. **Phase 3.3-3.4** - Text editor, locale, time
7. **Phase 7** - Systemd integration
8. **Phase 4.2** - NetworkManager for WiFi
9. **Phase 8** - Hardware support
10. **Phase 9** - Quality of life
11. **Phase 10** - Build independence

---

## Testing Checklist

For each phase, verify in QEMU:

```bash
# Serial console
cargo run -- test

# GUI mode
cargo run -- test --gui
```

### Phase 2 Test
```bash
lsblk
parted /dev/vda print
```

### Phase 4 Test
```bash
ip addr
ping -c 1 8.8.8.8
```

### Full Installation Test
```bash
# Follow the installation guide step by step
# Verify each command works
```

---

## Notes

- Rocky Linux is ONLY for userspace binaries temporarily
- The kernel should eventually be vanilla or custom-built
- Goal is a self-contained, independent distribution
- Keep initramfs small - only include what's needed for installation
