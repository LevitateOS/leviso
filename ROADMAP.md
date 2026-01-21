# LevitateOS Daily Driver Specification

**Version:** 1.0
**Last Updated:** 2026-01-21
**Goal:** Everything a user needs to use LevitateOS as their primary operating system, competing directly with Arch Linux.

---

## How to Use This Document

- **[ ]** = Not implemented / Not tested
- **[~]** = Partially implemented / Needs work
- **[x]** = Fully implemented and tested

Each item should have:
1. A test in `install-tests` or `rootfs-tests`
2. The actual functionality in the rootfs tarball
3. Documentation in `docs-content`

---

## Architecture

The ISO boots directly into an initramfs-based live environment (no squashfs):

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel
│   └── initramfs.img     # Complete live environment
├── levitateos-base.tar.xz # Base system tarball
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
4. Extract base tarball OR `recipe bootstrap /mnt`
5. Configure (fstab, bootloader, users)
6. Reboot into installed system

---

## Current State (What's Already Working)

### Live Environment (Initramfs)
- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts rootfs for sourcing binaries
- [x] Builds initramfs with bash + coreutils
- [x] Creates bootable hybrid BIOS/UEFI ISO
- [x] QEMU test command (`cargo run -- test` / `cargo run -- run`)
- [x] efivarfs mounted for UEFI verification
- [x] Systemd as PID 1 (boots to multi-user.target)
- [x] Basic systemd units (getty, serial-console)

### Disk Utilities (in initramfs)
- [x] `lsblk` - list block devices
- [x] `blkid` - show UUIDs and labels
- [x] `fdisk` - partition disks
- [x] `parted` - GPT partition table
- [x] `wipefs` - wipe filesystem signatures
- [x] `mkfs.ext4` - format root partition
- [x] `mkfs.fat` - format EFI partition
- [x] `mount` / `umount`

### User Management (in initramfs)
- [x] `useradd` - create users
- [x] `groupadd` - create groups
- [x] `chpasswd` - set passwords non-interactively

### Keyboard/Locale (in initramfs)
- [x] `loadkeys` - keyboard layout
- [x] Keymaps in `/usr/share/kbd/keymaps/`

### Base Tarball Access
- [x] ISO exposed as `/dev/sr0` via virtio-scsi CDROM
- [x] Kernel modules: `virtio_scsi`, `cdrom`, `sr_mod`, `isofs`
- [x] Tarball accessible at: `/media/cdrom/levitateos-base.tar.xz`

---

## What's Missing from Initramfs

These are known gaps in the live environment:

### User Tools
- [ ] `passwd` - interactive password setting
- [ ] `nano` - text editor for config files

### Locale & Time
- [ ] `locale-gen` / `localedef` - generate locales
- [ ] Timezone data (`/usr/share/zoneinfo/`)

### Bootloader
- [ ] `bootctl` - systemd-boot installer

### Networking
- [ ] Full NetworkManager or systemd-networkd
- [ ] `dhcpcd` or DHCP in networkd
- [ ] WiFi tools (iwd, wpa_supplicant)

### Recipe
- [ ] `recipe` binary in initramfs
- [ ] `recipe bootstrap /mnt` command

### Quality of Life
- [ ] Proper shutdown/reboot (untested)
- [ ] Tab completion (bash-completion)
- [ ] Welcome message with install instructions

---

## Testing Commands

```bash
# Build and test
cargo run -- build      # Full build (kernel + rootfs + ISO)
cargo run -- test       # Quick terminal test (serial console)
cargo run -- run        # Full QEMU GUI with disk

# Step by step
cargo run -- download   # Download Rocky ISO
cargo run -- extract rocky  # Extract ISO contents
cargo run -- build rootfs   # Build rootfs tarball
cargo run -- build initramfs # Build initramfs
cargo run -- build iso      # Build ISO
```

### Manual Installation Test (in QEMU)

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

# 3. Configure (fstab, bootloader, users) and reboot
# See install-tests for automated version
```

---

## Important Notes

- **Rocky Linux is TEMPORARY** - Only used for sourcing userspace binaries
- **Kernel is independent** - Built from kernel.org, not a Rocky rebrand
- **Initramfs IS the live environment** - No squashfs layer, no switch_root
- **`recipe` handles everything** - Both live queries AND installation to target disk

---

## Future: Build Independence (Phase 10)

Goal: Don't depend on Rocky Linux at all

- [ ] Vanilla kernel from kernel.org (already done)
- [ ] Build coreutils from source
- [ ] Build systemd from source
- [ ] Build all binaries from source
- [ ] Remove Rocky dependency entirely

This is a long-term goal. For now, Rocky provides tested, compatible binaries.

---

## 1. BOOT & INSTALLATION

### 1.1 Boot Modes
- [ ] UEFI boot (GPT, ESP partition)
- [ ] BIOS/Legacy boot (MBR) - *optional but nice*
- [ ] Secure Boot signed - *future*

### 1.2 Boot Media
- [ ] ISO boots on real hardware
- [ ] ISO boots in VirtualBox
- [ ] ISO boots in VMware
- [ ] ISO boots in QEMU/KVM
- [ ] ISO boots in Hyper-V
- [ ] USB bootable (dd or Ventoy compatible)

### 1.3 Installation Process
- [ ] Partition disk (GPT for UEFI)
- [ ] Format partitions (ext4, FAT32 for ESP)
- [ ] Mount target filesystem
- [ ] Extract base tarball
- [ ] Generate fstab with UUIDs
- [ ] Set timezone
- [ ] Set locale
- [ ] Set hostname
- [ ] Set root password
- [ ] Create user account
- [ ] Add user to wheel group
- [ ] Generate initramfs
- [ ] Install bootloader (systemd-boot)
- [ ] Reboot into installed system

### 1.4 Post-Installation Verification
- [ ] System boots without ISO
- [ ] systemd is PID 1
- [ ] multi-user.target reached
- [ ] No failed systemd units
- [ ] User can log in
- [ ] sudo works
- [ ] Network is functional

---

## 2. NETWORKING

### 2.1 Network Stack
- [ ] systemd-networkd OR NetworkManager
- [ ] systemd-resolved for DNS
- [ ] /etc/resolv.conf configured
- [ ] /etc/hosts with localhost entries
- [ ] /etc/nsswitch.conf with proper hosts line

### 2.2 Ethernet
- [ ] DHCP client works (dhcpcd or systemd-networkd)
- [ ] Static IP configuration works
- [ ] Link detection (cable plug/unplug)
- [ ] Gigabit speeds supported
- [ ] Common drivers: e1000, e1000e, r8169, igb, ixgbe

### 2.3 WiFi
- [ ] iwd OR wpa_supplicant installed
- [ ] iwctl OR nmcli can scan networks
- [ ] Can connect to WPA2-PSK network
- [ ] Can connect to WPA3 network
- [ ] Can connect to WPA2-Enterprise (802.1X)
- [ ] WiFi firmware: Intel (iwlwifi), Atheros, Realtek, Broadcom
- [ ] wireless-regdb for regulatory compliance

### 2.4 Network Tools
- [ ] `ip` - interface and routing configuration
- [ ] `ping` - connectivity testing
- [ ] `ss` - socket statistics
- [ ] `curl` - HTTP client
- [ ] `wget` - file downloads (optional, curl sufficient)
- [ ] `dig` / `nslookup` - DNS queries (from bind-utils or ldns)
- [ ] `traceroute` / `tracepath` - path tracing
- [ ] `ethtool` - NIC configuration
- [ ] `nmap` - network scanning (optional)
- [ ] `tcpdump` - packet capture (optional)

### 2.5 VPN Support
- [ ] OpenVPN client
- [ ] WireGuard support (kernel module + tools)
- [ ] IPsec support (strongswan or libreswan) - *optional*

### 2.6 Remote Access
- [ ] SSH server (sshd) - installable
- [ ] SSH client (ssh, scp, sftp)
- [ ] Key-based authentication works

### 2.7 Firewall
- [ ] nftables OR iptables available
- [ ] firewalld OR ufw - *optional convenience*

---

## 3. STORAGE & FILESYSTEMS

### 3.1 Partitioning Tools
- [ ] `fdisk` - MBR/GPT partitioning
- [ ] `parted` - GPT partitioning
- [ ] `gdisk` / `sgdisk` - GPT-specific tools
- [ ] `lsblk` - list block devices
- [ ] `blkid` - show UUIDs and labels
- [ ] `wipefs` - clear filesystem signatures

### 3.2 Filesystem Support
- [ ] ext4 (mkfs.ext4, e2fsck, tune2fs)
- [ ] FAT32/vfat (mkfs.fat, fsck.fat) - required for ESP
- [ ] XFS (mkfs.xfs, xfs_repair) - *optional*
- [ ] Btrfs (mkfs.btrfs, btrfs) - *optional but popular*
- [ ] NTFS read/write (ntfs-3g) - for Windows drives
- [ ] exFAT (exfatprogs) - for USB drives and SD cards
- [ ] ISO9660 (mount -t iso9660)
- [ ] squashfs (for live systems)

### 3.3 LVM & RAID
- [ ] LVM2 (pvcreate, vgcreate, lvcreate)
- [ ] mdadm for software RAID - *optional*
- [ ] dmraid for fake RAID - *optional*

### 3.4 Encryption
- [ ] LUKS encryption (cryptsetup)
- [ ] Encrypted root partition support
- [ ] crypttab for automatic unlock

### 3.5 Mount & Automount
- [ ] `mount` / `umount`
- [ ] `findmnt` - show mounted filesystems
- [ ] fstab support with UUID
- [ ] systemd automount for removable media
- [ ] udisks2 for desktop automount - *optional*

### 3.6 Storage Drivers (Kernel Modules)
- [ ] SATA: ahci, ata_piix
- [ ] NVMe: nvme
- [ ] USB storage: usb-storage, uas
- [ ] SD cards: sdhci, mmc_block
- [ ] SCSI: sd_mod, sr_mod (CD/DVD)
- [ ] VirtIO: virtio_blk, virtio_scsi

### 3.7 Disk Health
- [ ] `smartctl` (smartmontools) - SMART monitoring
- [ ] `hdparm` - drive parameters
- [ ] `nvme-cli` - NVMe management

---

## 4. USER MANAGEMENT

### 4.1 User Operations
- [ ] `useradd` - create users
- [ ] `usermod` - modify users
- [ ] `userdel` - delete users
- [ ] `passwd` - change passwords
- [ ] `chpasswd` - batch password setting
- [ ] `chage` - password expiry
- [ ] `/etc/passwd` proper format
- [ ] `/etc/shadow` proper format and permissions (0400)

### 4.2 Group Operations
- [ ] `groupadd` - create groups
- [ ] `groupmod` - modify groups
- [ ] `groupdel` - delete groups
- [ ] `gpasswd` - group administration
- [ ] `/etc/group` proper format
- [ ] `/etc/gshadow` proper format

### 4.3 Default Groups
- [ ] `wheel` - sudo access
- [ ] `audio` - audio devices
- [ ] `video` - video devices
- [ ] `input` - input devices
- [ ] `storage` - removable media
- [ ] `optical` - CD/DVD drives
- [ ] `network` - network configuration
- [ ] `users` - standard users group

### 4.4 Privilege Escalation
- [ ] `sudo` installed and configured
- [ ] `/etc/sudoers` with `%wheel ALL=(ALL:ALL) ALL`
- [ ] `visudo` for safe editing
- [ ] `su` for user switching
- [ ] PAM configuration proper

### 4.5 Login System
- [ ] getty on TTY1-6
- [ ] agetty autologin option - *optional*
- [ ] Login shell works (bash)
- [ ] `.bashrc` / `.bash_profile` sourced
- [ ] `/etc/profile` and `/etc/profile.d/` executed

---

## 5. CORE UTILITIES

### 5.1 GNU Coreutils (or compatible)
- [ ] `ls`, `cp`, `mv`, `rm`, `mkdir`, `rmdir`
- [ ] `cat`, `head`, `tail`, `tee`
- [ ] `chmod`, `chown`, `chgrp`
- [ ] `ln` (symlinks and hardlinks)
- [ ] `touch`, `stat`, `file`
- [ ] `wc`, `sort`, `uniq`, `cut`
- [ ] `tr`, `fold`, `fmt`
- [ ] `echo`, `printf`, `yes`
- [ ] `date`, `cal`
- [ ] `df`, `du`
- [ ] `pwd`, `basename`, `dirname`, `realpath`
- [ ] `env`, `printenv`
- [ ] `sleep`, `timeout`
- [ ] `tty`, `stty`
- [ ] `id`, `whoami`, `groups`
- [ ] `uname`
- [ ] `seq`, `shuf`
- [ ] `md5sum`, `sha256sum`, `sha512sum`
- [ ] `base64`
- [ ] `install`

### 5.2 Text Processing
- [ ] `grep` (GNU grep with -P for PCRE)
- [ ] `sed` (GNU sed)
- [ ] `awk` (gawk)
- [ ] `diff`, `patch`
- [ ] `less` (pager)
- [ ] `nano` OR `vim` (text editor)

### 5.3 File Finding
- [ ] `find` (GNU findutils)
- [ ] `locate` / `mlocate` - *optional*
- [ ] `which`, `whereis`
- [ ] `xargs`

### 5.4 Archive Tools
- [ ] `tar` (GNU tar with xz, gzip, bzip2, zstd support)
- [ ] `gzip`, `gunzip`
- [ ] `bzip2`, `bunzip2`
- [ ] `xz`, `unxz`
- [ ] `zstd` - increasingly common
- [ ] `zip`, `unzip` - for Windows compatibility
- [ ] `cpio` - for initramfs

### 5.5 Shell
- [ ] `bash` as /bin/bash and /bin/sh
- [ ] Tab completion (bash-completion)
- [ ] Command history
- [ ] Job control (bg, fg, jobs)
- [ ] `zsh` - *optional alternative*

---

## 6. SYSTEM SERVICES (systemd)

### 6.1 Core systemd
- [ ] `systemctl` - service management
- [ ] `journalctl` - log viewing
- [ ] `systemd-analyze` - boot analysis
- [ ] `hostnamectl` - hostname management
- [ ] `timedatectl` - time/date management
- [ ] `localectl` - locale management
- [ ] `loginctl` - session management

### 6.2 Essential Services
- [ ] `systemd-journald` - logging
- [ ] `systemd-logind` - login management
- [ ] `systemd-networkd` - networking
- [ ] `systemd-resolved` - DNS
- [ ] `systemd-timesyncd` - NTP
- [ ] `systemd-udevd` - device management

### 6.3 Boot Services
- [ ] `getty@ttyN` - virtual consoles
- [ ] `serial-getty@` - serial console (for VMs)
- [ ] `systemd-boot` OR `grub` - bootloader
- [ ] `systemd-boot-update` - auto-update entries

### 6.4 Timer Support
- [ ] systemd timers work (replacement for cron)
- [ ] `systemd-tmpfiles` - temp file management
- [ ] `systemd-sysusers` - system user creation

---

## 7. HARDWARE SUPPORT

### 7.1 CPU
- [ ] Intel microcode (intel-ucode)
- [ ] AMD microcode (amd-ucode)
- [ ] CPU frequency scaling (cpupower)
- [ ] Temperature monitoring (lm_sensors)

### 7.2 Memory
- [ ] Swap partition/file support
- [ ] zram/zswap - *optional*
- [ ] `free` - memory stats
- [ ] `/proc/meminfo` readable

### 7.3 PCI/USB Detection
- [ ] `lspci` (pciutils)
- [ ] `lsusb` (usbutils)
- [ ] `lshw` - *optional but useful*
- [ ] `dmidecode` - SMBIOS/DMI info

### 7.4 Input Devices
- [ ] Keyboard works (all layouts via loadkeys)
- [ ] Mouse works (PS/2 and USB)
- [ ] Touchpad works (libinput)
- [ ] Keymaps in /usr/share/kbd/keymaps/

### 7.5 Display (Framebuffer/Console)
- [ ] Console fonts (terminus-font or similar)
- [ ] `setfont` - change console font
- [ ] 80x25 minimum, 1920x1080 framebuffer preferred
- [ ] Virtual consoles (Ctrl+Alt+F1-F6)

### 7.6 Audio (Console/Headless)
- [ ] ALSA utilities (alsa-utils) - *optional for server*
- [ ] `amixer`, `alsamixer` - *optional*

### 7.7 Graphics (Optional - for Desktop)
- [ ] Intel graphics (i915)
- [ ] AMD graphics (amdgpu)
- [ ] NVIDIA (nouveau or proprietary)
- [ ] VirtualBox Guest Additions
- [ ] VMware SVGA driver
- [ ] QXL for QEMU/KVM

### 7.8 Bluetooth - *optional*
- [ ] BlueZ stack
- [ ] `bluetoothctl`
- [ ] Firmware for common adapters

### 7.9 Printing - *optional*
- [ ] CUPS
- [ ] Common printer drivers

---

## 8. PACKAGE MANAGEMENT

### 8.1 Recipe Package Manager
- [ ] `recipe search <package>` - search packages
- [ ] `recipe install <package>` - install packages
- [ ] `recipe remove <package>` - remove packages
- [ ] `recipe update` - update package database
- [ ] `recipe upgrade` - upgrade installed packages
- [ ] `recipe info <package>` - show package info
- [ ] `recipe list` - list installed packages
- [ ] `recipe files <package>` - show package files

### 8.2 Package Sources
- [ ] Binary packages (pre-compiled)
- [ ] Source packages (compile from source)
- [ ] Local package installation
- [ ] Repository management

### 8.3 Dependencies
- [ ] Automatic dependency resolution
- [ ] Conflict detection
- [ ] Provides/replaces support

---

## 9. DEVELOPMENT (Optional but Expected)

### 9.1 Build Tools
- [ ] `gcc` / `clang` - installable
- [ ] `make` - installable
- [ ] `cmake` - installable
- [ ] `pkg-config`

### 9.2 Version Control
- [ ] `git` - installable

### 9.3 Scripting
- [ ] Python 3 - installable
- [ ] Perl - often required as dependency
- [ ] Node.js - installable

---

## 10. VIRTUALIZATION SUPPORT

### 10.1 Guest Additions
- [ ] QEMU guest agent (qemu-guest-agent)
- [ ] VirtualBox Guest Additions (virtualbox-guest-utils)
- [ ] VMware Tools (open-vm-tools)
- [ ] Hyper-V daemons (hyperv)

### 10.2 VirtIO Drivers
- [ ] virtio_blk - block devices
- [ ] virtio_net - networking
- [ ] virtio_scsi - SCSI
- [ ] virtio_console - console
- [ ] virtio_balloon - memory ballooning
- [ ] virtio_gpu - graphics

---

## 11. SECURITY

### 11.1 Basic Security
- [ ] `/etc/shadow` permissions 0400
- [ ] Root account locked by default? (configurable)
- [ ] Password hashing (SHA-512)
- [ ] Failed login delays

### 11.2 SSH Security
- [ ] Root login disabled by default
- [ ] Key-based auth preferred
- [ ] SSH host keys generated

### 11.3 Firewall
- [ ] nftables/iptables available
- [ ] Default deny policy - *optional*

### 11.4 SELinux/AppArmor - *future*
- [ ] Not required for v1.0

---

## 12. RECOVERY & DIAGNOSTICS

### 12.1 Recovery Tools
- [ ] Single-user mode (init=/bin/bash)
- [ ] Live ISO can rescue installed system
- [ ] `fsck` for all supported filesystems
- [ ] `testdisk` - *optional but useful*
- [ ] `ddrescue` - *optional*

### 12.2 Diagnostic Tools
- [ ] `dmesg` - kernel messages
- [ ] `journalctl -b` - boot logs
- [ ] `systemctl --failed` - failed services
- [ ] `/var/log/` directory structure

### 12.3 Performance Tools
- [ ] `top` / `htop`
- [ ] `ps` - process listing
- [ ] `kill`, `killall`, `pkill`
- [ ] `nice`, `renice`
- [ ] `iostat`, `vmstat` - *optional*

---

## 13. LOCALIZATION

### 13.1 Locale
- [ ] UTF-8 support (en_US.UTF-8 default)
- [ ] locale-gen or equivalent
- [ ] `/etc/locale.conf`
- [ ] `/etc/locale.gen`

### 13.2 Timezone
- [ ] tzdata installed
- [ ] `/etc/localtime` symlink
- [ ] `timedatectl set-timezone`

### 13.3 Keyboard
- [ ] US layout default
- [ ] Other layouts available
- [ ] `/etc/vconsole.conf`

### 13.4 Console Fonts
- [ ] Readable default font
- [ ] Unicode support

---

## 14. DOCUMENTATION

### 14.1 Man Pages
- [ ] `man` command works
- [ ] man-db or mandoc
- [ ] Core command man pages included

### 14.2 Info Pages - *optional*
- [ ] `info` command
- [ ] GNU info pages

### 14.3 Online Documentation
- [ ] Installation guide on website
- [ ] Wiki or knowledge base

---

## 15. ACCESSIBILITY - *optional for v1.0*

### 15.1 Console Accessibility
- [ ] Large console fonts
- [ ] High contrast
- [ ] Screen reader (espeakup) - *optional*
- [ ] Braille display (brltty) - *optional*

---

## TEST MATRIX

| Category | Items | Tested in install-tests | Tested in rootfs-tests |
|----------|-------|------------------------|------------------------|
| Boot | 14 | Partial | N/A |
| Network | 30+ | Partial | Partial |
| Storage | 25+ | Partial | No |
| Users | 20+ | Partial | Partial |
| Utilities | 50+ | Partial | Partial |
| systemd | 15+ | Partial | Partial |
| Hardware | 30+ | No | No |
| Packages | 8 | No | Partial |
| Security | 10+ | No | No |
| Recovery | 10+ | No | No |
| Locale | 8 | Partial | No |

---

## ARCH ISO COMPARISON

Arch Linux ISO includes these packages we should evaluate:

### Network & WiFi
- `dhcpcd` - DHCP client
- `iwd` - WiFi daemon
- `wpa_supplicant` - WPA authentication
- `wireless_tools` - iwconfig etc
- `wireless-regdb` - regulatory database
- `ethtool` - NIC config
- `modemmanager` - mobile broadband

### Filesystems
- `btrfs-progs`
- `dosfstools`
- `e2fsprogs`
- `exfatprogs`
- `f2fs-tools`
- `jfsutils`
- `ntfs-3g`
- `xfsprogs`

### Disk Tools
- `cryptsetup` - LUKS
- `dmraid`
- `gptfdisk`
- `hdparm`
- `lvm2`
- `mdadm`
- `nvme-cli`
- `parted`
- `sdparm`
- `smartmontools`

### Hardware
- `amd-ucode`
- `intel-ucode`
- `linux-firmware`
- `linux-firmware-marvell`
- `sof-firmware` - sound
- `dmidecode`
- `usbutils`

### Utilities
- `arch-install-scripts` - genfstab, arch-chroot
- `diffutils`
- `less`
- `man-db`
- `man-pages`
- `nano`
- `rsync`
- `sudo`
- `vim`

### VPN
- `openconnect`
- `openvpn`
- `ppp`
- `vpnc`
- `wireguard-tools` (in kernel)

### Recovery
- `clonezilla`
- `ddrescue`
- `fsarchiver`
- `gpart`
- `partclone`
- `partimage`
- `testdisk`

### VM Support
- `hyperv`
- `open-vm-tools`
- `qemu-guest-agent`
- `virtualbox-guest-utils-nox`

---

## PRIORITY LEVELS

### P0 - Must Have (Blocking Release)
- Boot and installation works
- Network (Ethernet + DHCP)
- User management + sudo
- Core utilities
- Package manager basics

### P1 - Should Have (Important)
- WiFi support
- Full filesystem support
- LUKS encryption
- SSH
- All man pages

### P2 - Nice to Have (Enhancement)
- VPN support
- VM guest tools
- Recovery tools
- Bluetooth
- Printing

### P3 - Future
- Secure Boot
- Full accessibility
- SELinux/AppArmor

---

## UPDATING THIS DOCUMENT

When you implement something:
1. Change `[ ]` to `[x]`
2. Add test coverage to appropriate test crate
3. Update the TEST MATRIX section
4. Commit with message: `spec: Mark <item> as complete`

When you find something missing:
1. Add it to the appropriate section
2. Mark it as `[ ]`
3. Add a note about priority
4. Commit with message: `spec: Add <item> requirement`
