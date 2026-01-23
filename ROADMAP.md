# LevitateOS Daily Driver Specification

> **DOCUMENTATION NOTE:** This roadmap will be used to create the installation guide
> and user documentation. Keep descriptions clear, complete, and user-facing.

**Version:** 1.1
**Last Updated:** 2026-01-22
**Goal:** Everything a user needs to use LevitateOS as their primary operating system, competing directly with Arch Linux.

---

## ⚠️ ARCHISO PARITY STATUS

> **Reference:** See `docs/archiso-parity-checklist.md` for full details (verified 2026-01-22)

LevitateOS aims for parity with archiso - the Arch Linux installation ISO. This section tracks critical gaps.

### P0 - Critical Gaps (Blocking for Daily Driver)

| Gap | Impact | Status |
|-----|--------|--------|
| **Intel/AMD microcode** | CPU bugs, security vulnerabilities | NOT IN BUILD |
| **cryptsetup (LUKS)** | No encrypted disk support | NOT IN BUILD |
| **lvm2** | No LVM support | NOT IN BUILD |
| **btrfs-progs** | No Btrfs support | NOT IN BUILD |

### P1 - Important Gaps

| Gap | Impact | Status |
|-----|--------|--------|
| ~~Volatile journal storage~~ | ~~Logs may fill tmpfs~~ | ✅ CONFIGURED |
| ~~do-not-suspend config~~ | ~~Live session may sleep during install~~ | ✅ CONFIGURED |
| ~~SSH server (sshd)~~ | ~~No remote installation/rescue~~ | ✅ AVAILABLE |
| ~~pciutils (lspci)~~ | ~~Cannot identify PCI hardware~~ | ✅ INCLUDED |
| usbutils (lsusb) | Cannot identify USB devices | NOT IN BUILD |
| dmidecode | Cannot read BIOS/DMI info | NOT IN BUILD |
| ethtool | Cannot diagnose NICs | NOT IN BUILD |
| ~~gdisk/sgdisk~~ | ~~Only fdisk for GPT~~ | N/A - NOT IN ROCKY 10.1 (parted/sfdisk sufficient) |
| iwd | Only wpa_supplicant for WiFi | NOT IN BUILD |
| wireless-regdb | WiFi may violate regulations | NOT IN BUILD |
| sof-firmware | Modern laptop sound may not work | NOT IN BUILD |
| ~~ISO SHA512 checksum~~ | ~~Users cannot verify downloads~~ | ✅ GENERATED |

### What's Working (Verified)

| Feature | Status |
|---------|--------|
| UEFI boot | ✅ Verified in E2E test |
| NetworkManager + WiFi firmware | ✅ Enabled, firmware included |
| Autologin to root shell | ✅ Like archiso |
| machine-id empty | ✅ Regenerates on first boot |
| Hostname set ("levitateos") | ✅ Configured |
| recstrap (squashfs extraction) | ✅ Working |
| recfstab (genfstab equivalent) | ✅ Implemented |
| recchroot (arch-chroot equivalent) | ✅ Implemented |
| systemd as PID 1 | ✅ Verified |
| Serial console | ✅ Enabled |

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

The ISO uses a squashfs-based live environment (like Arch, Ubuntu, Fedora):

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel
│   └── initramfs.img     # Tiny (~10MB) - mounts squashfs
├── live/
│   └── filesystem.squashfs  # COMPLETE system (~350MB compressed)
└── EFI/...               # Bootloader
```

**Boot flow:**
1. Kernel + tiny initramfs boot
2. Initramfs mounts filesystem.squashfs (read-only)
3. Initramfs mounts overlay (tmpfs for writes)
4. switch_root to squashfs
5. User has FULL daily driver system

**Installation flow:**
1. Boot ISO → live environment (from squashfs)
2. Run: `recstrap /dev/vda`
3. Reboot into installed system

**Key insight:** The squashfs IS the complete system. Live = Installed.

---

## Installation Guide Outline

> This roadmap will be converted into user-facing installation documentation.
> Each section maps to a documentation page.

### Pre-Installation (Website)
1. **System Requirements** - CPU, RAM, storage
2. **Download ISO** - Links, SHA512 verification
3. **Create Boot Media** - dd, Ventoy, Rufus

### Live Environment (Installation Guide)
1. **Boot the ISO** - UEFI boot process
2. **Connect to Network** - `nmcli` for WiFi, automatic for Ethernet
3. **Prepare Disks** - `fdisk`/`parted`, `mkfs`
4. **Extract System** - `recstrap /mnt`
5. **Configure System** - fstab, hostname, users, passwords
6. **Install Bootloader** - `bootctl install`
7. **Reboot** - First boot into installed system

### Post-Installation (Wiki)
1. **First Boot** - What to expect
2. **Package Management** - Using `recipe`
3. **Desktop Environment** - Installing Sway, GNOME, etc.
4. **Troubleshooting** - Common issues

---

## System Extractor: `recstrap`

> **NOTE:** recstrap is like pacstrap. NOT like archinstall!
> It extracts squashfs to target. User does EVERYTHING else manually.

### Why squashfs + recstrap?

Like Arch's `pacstrap`, LevitateOS uses `recstrap` (recipe + strap) to extract the system.

**Problems with cpio initramfs + tarball:**
- ~400MB RAM usage just for live environment
- Two sources of truth (initramfs + tarball have different content)
- Need complex logic to copy networking from initramfs to installed system

**Solution - squashfs architecture:**
- Single source of truth: filesystem.squashfs has EVERYTHING
- Less RAM: squashfs reads from disk, not all in RAM
- Simple installation: just unsquash to disk
- **Live = Installed:** exact same files

### What recstrap does

```bash
recstrap /mnt                    # Extract squashfs to /mnt
recstrap /mnt --squashfs /path   # Custom squashfs location
```

That's it. **recstrap only extracts**. Like pacstrap, NOT like archinstall.

User does EVERYTHING else manually (like Arch):
- Partitioning (fdisk, parted)
- Formatting (mkfs.ext4, mkfs.fat)
- Mounting (/mnt, /mnt/boot)
- fstab generation (recfstab)
- Bootloader (bootctl install)
- Password (passwd)
- Users (useradd)
- Timezone, locale, hostname

### Squashfs architecture

```
ISO (~400MB):
├── initramfs.img (~10MB) - tiny, just mounts squashfs
└── live/filesystem.squashfs (~350MB compressed)
    ├── All binaries (bash, coreutils, systemd...)
    ├── NetworkManager + wpa_supplicant
    ├── ALL firmware (WiFi, GPU, sound, BT)
    ├── recipe package manager
    └── recstrap binary

Live boot: squashfs mounted + overlay for writes
Installation: unsquash squashfs to /mnt
Result: Live = Installed (same files!)
```

### Implementation status

**Squashfs builder:**
- [x] Create `src/squashfs/mod.rs` - build complete system
- [x] Include ALL binaries (combine rootfs + initramfs content)
- [x] Include NetworkManager, wpa_supplicant
- [x] Include ALL firmware (~350MB)
- [x] Generate filesystem.squashfs with mksquashfs

**Tiny initramfs:**
- [x] Create `src/initramfs/mod.rs` - minimal boot initramfs (~5MB)
- [x] Mount squashfs read-only
- [x] Mount overlay (tmpfs) for writes
- [x] switch_root to live system

**recstrap (sibling directory: ../recstrap/):**
- [x] Extract squashfs to target directory

**Integration:**
- [x] Update ISO builder for new layout
- [x] Include recstrap in squashfs
- [x] Update welcome message to show manual install steps

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

## What's Missing from Live Environment

These are known gaps in the live environment (squashfs):

### Critical Tools (P0 - IMPLEMENTED)
- [x] `recfstab` - generate fstab from mounted filesystems (genfstab equivalent)
- [x] `recchroot` - enter installed system with proper mounts (arch-chroot equivalent)

> **✅ NOTE:** The /etc/motd welcome message has been updated to reference `recfstab` and `recchroot`.
- [ ] `cryptsetup` - LUKS disk encryption
- [ ] `lvm2` - Logical Volume Manager (pvcreate, vgcreate, lvcreate)
- [ ] `btrfs-progs` - Btrfs filesystem tools

### Important Tools (P1)
- [ ] `gdisk` / `sgdisk` - GPT partitioning (better than fdisk for GPT)
- [x] `pciutils` (lspci) - identify PCI hardware
- [ ] `usbutils` (lsusb) - identify USB devices
- [ ] `dmidecode` - BIOS/DMI information
- [ ] `ethtool` - NIC diagnostics and configuration
- [ ] `iwd` - alternative WiFi daemon (often more reliable)
- [ ] `wireless-regdb` - WiFi regulatory database

### Live Environment Config (P1)
- [x] Volatile journal storage (`Storage=volatile` in journald.conf)
- [x] do-not-suspend logind config (prevent sleep during install)

### User Tools (Working)
- [x] `passwd` - interactive password setting
- [x] `nano` - text editor for config files

### Locale & Time (Working)
- [x] `localedef` - generate locales (in COREUTILS)
- [x] Timezone data (`/usr/share/zoneinfo/`)

### Bootloader (Working)
- [x] `bootctl` - systemd-boot installer (in COREUTILS)

---

## Live Environment Configuration Gaps

These are configuration issues that don't require new binaries, just systemd config files.

### Volatile Journal Storage (P1)

archiso configures `journald` to use volatile storage so logs don't fill the tmpfs overlay.

**Fix needed:** Create `/etc/systemd/journald.conf.d/volatile.conf`:
```ini
[Journal]
Storage=volatile
RuntimeMaxUse=64M
```

**File to modify:** `leviso/src/build/systemd.rs` - add `setup_volatile_journal()`

### Do-Not-Suspend Config (P1)

archiso prevents the live session from suspending/hibernating during installation.

**Fix needed:** Create `/etc/systemd/logind.conf.d/do-not-suspend.conf`:
```ini
[Login]
HandleSuspendKey=ignore
HandleHibernateKey=ignore
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
IdleAction=ignore
```

**File to modify:** `leviso/src/build/systemd.rs` - add `setup_do_not_suspend()`

### KMS (Kernel Mode Setting)

archiso has a KMS hook for proper graphics mode switching.

**Status:** Not implemented. Investigate if needed for LevitateOS.

---

### Networking (IMPLEMENTED in src/build/network.rs)

- [x] Create `src/initramfs/network.rs` module
- [x] Copy NetworkManager + nmcli + nmtui + nm-online binaries
- [x] Copy wpa_supplicant + wpa_cli + wpa_passphrase binaries
- [x] Copy iproute2 (ip) binary
- [x] Copy /etc/NetworkManager/ configs (or create minimal)
- [x] Copy /usr/lib64/NetworkManager/ plugins
- [x] Copy NetworkManager D-Bus policies
- [x] Copy wpa_supplicant D-Bus policies
- [x] Copy systemd service units (NetworkManager.service, wpa_supplicant.service)
- [x] Enable NetworkManager in multi-user.target.wants
- [x] Copy WiFi firmware: iwlwifi (Intel), ath10k/ath11k (Atheros), rtlwifi/rtw88/rtw89 (Realtek), brcm/cypress (Broadcom), mediatek (MediaTek)
- [x] Create nm-openconnect user for NM plugins
- [x] Add virtio_net + e1000 + e1000e + r8169 kernel modules to config.rs
- [x] Test: `systemctl status NetworkManager` shows active
- [x] Test: `nmcli device` shows interfaces (eth0 connected, DHCP works)
- [ ] Test: `nmcli device wifi list` shows networks (on real hardware)

### Recipe Package Manager
- [x] `recipe` binary in squashfs (/usr/bin/recipe)
- [x] Recipe configuration in /etc/recipe/config.toml
- [ ] `recipe search` / `recipe install` commands (package manager functionality)

### Quality of Life
- [x] Proper shutdown/reboot (via `poweroff`/`reboot` aliases)
- [ ] Tab completion (bash-completion)
- [x] Welcome message with install instructions (/etc/motd)

### Documentation
- [ ] `levitate-docs` TUI documentation viewer (requires Node.js - post-install via recipe)

---

## Networking Implementation

> **STATUS: ✅ IMPLEMENTED** - See `src/build/network.rs`

| Component | Status | Location |
|-----------|--------|----------|
| NetworkManager | ✅ Working | `src/build/network.rs` |
| wpa_supplicant | ✅ Working | `src/build/network.rs` |
| nmcli / nmtui | ✅ Working | Included with NetworkManager |
| WiFi firmware | ✅ Working | Intel, Atheros, Realtek, Broadcom, MediaTek |
| Ethernet modules | ✅ Working | virtio_net, e1000, e1000e, r8169 |
| Auto-start | ✅ Working | NetworkManager.service enabled |

### Verification Commands
```bash
systemctl status NetworkManager   # Should show active
nmcli device                      # Should list interfaces
nmcli device wifi list            # Should show networks (real hardware)
```

### Still Missing (P1)
- [ ] iwd - alternative WiFi daemon
- [ ] wireless-regdb - regulatory compliance
- [ ] sof-firmware - Intel Sound Open Firmware

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

- **Rocky Linux is the binary source** - Userspace binaries extracted from Rocky/Fedora RPMs (build in minutes, not hours)
- **Kernel is independent** - Built from kernel.org, not a Rocky rebrand
- **Squashfs-based architecture** - Tiny initramfs (~5MB) mounts squashfs + overlay, then switch_root
- **`recipe` handles everything** - Both live queries AND installation to target disk

---

## 1. BOOT & INSTALLATION

### 1.0 ISO Integrity Verification (P1)

archiso provides checksum files so users can verify their download.

- [ ] **SHA512 checksum** - Generate `levitateos-YYYY.MM.DD.iso.sha512` during build
- [ ] **GPG signature** - Sign checksum file for release verification
- [ ] Document verification process on website

**File to modify:** `leviso/src/iso.rs` - add checksum generation after xorriso

### 1.1 Boot Modes
- [x] UEFI boot (GPT, ESP partition) - verified in E2E test
- [ ] BIOS/Legacy boot (MBR) - P2 (optional but nice)
- [ ] Secure Boot signed - P3 (future)

### 1.2 Boot Media
- [ ] ISO boots on real hardware (needs testing)
- [ ] ISO boots in VirtualBox (needs testing)
- [ ] ISO boots in VMware (needs testing)
- [x] ISO boots in QEMU/KVM - verified in E2E test
- [ ] ISO boots in Hyper-V (needs testing)
- [ ] USB bootable (dd or Ventoy compatible) (needs testing)

### 1.3 Installation Process (recstrap)

**Installation Helper Scripts (archiso parity - IMPLEMENTED):**
- [x] **`recfstab`** - Generate fstab from mounted filesystems (like genfstab)
- [x] **`recchroot`** - Enter installed system like arch-chroot

**Installation Steps:**
- [x] Partition disk (GPT for UEFI) - verified in E2E test
- [x] Format partitions (ext4, FAT32 for ESP) - verified in E2E test
- [x] Mount target filesystem - verified in E2E test
- [x] Extract squashfs to disk - verified in E2E test
- [x] Generate fstab with UUIDs - `recfstab -U /mnt >> /mnt/etc/fstab`
- [ ] Set timezone (manual: timedatectl)
- [ ] Set locale (manual: localectl)
- [ ] Set hostname (manual: hostnamectl)
- [x] Set root password - verified in E2E test
- [ ] Create user account (manual: useradd)
- [ ] Add user to wheel group (manual: usermod)
- [x] Install bootloader (systemd-boot) - verified in E2E test
- [x] Reboot into installed system - verified in E2E test

> **NOTE:** Users can use `recfstab -U /mnt >> /mnt/etc/fstab` for archiso-like fstab generation.

### 1.4 Post-Installation Verification
- [x] System boots without ISO - verified in E2E test
- [x] systemd is PID 1 - verified in E2E test
- [x] multi-user.target reached - verified in E2E test
- [x] No failed systemd units - verified in E2E test (systemctl is-system-running = running)
- [x] User can log in - verified in E2E test
- [ ] sudo works (needs testing)
- [ ] Network is functional (needs testing on installed system)

---

## 2. NETWORKING

### 2.1 Network Stack
- [x] NetworkManager (in initramfs via network.rs)
- [x] systemd-resolved for DNS (in rootfs)
- [x] /etc/resolv.conf configured (symlink to systemd-resolved stub)
- [x] /etc/hosts with localhost entries
- [x] /etc/nsswitch.conf with proper hosts line

### 2.2 Ethernet
- [x] DHCP client works (NetworkManager)
- [x] Static IP configuration works (nmcli)
- [x] Link detection (cable plug/unplug) - NetworkManager
- [x] Gigabit speeds supported
- [x] Common drivers: e1000, e1000e, r8169 (in config.rs)

### 2.3 WiFi
- [x] wpa_supplicant installed (in squashfs via network.rs)
- [x] nmcli can scan networks (in squashfs)
- [ ] Can connect to WPA2-PSK network (untested on real hardware)
- [ ] Can connect to WPA3 network (untested)
- [ ] Can connect to WPA2-Enterprise (802.1X)
- [x] WiFi firmware: Intel (iwlwifi), Atheros, Realtek, Broadcom (network.rs)
- [ ] **wireless-regdb** - P1: Required for legal WiFi operation in many countries
- [ ] **iwd** - P1: Alternative WiFi daemon, often more reliable than wpa_supplicant
- [ ] **sof-firmware** - P1: Modern laptop sound (Intel SOF)

### 2.4 Network Tools
- [x] `ip` - interface and routing configuration (in rootfs)
- [x] `ping` - connectivity testing (in rootfs)
- [x] `ss` - socket statistics (in rootfs)
- [x] `curl` - HTTP client (in rootfs)
- [x] `wget` - file downloads (in rootfs)
- [ ] `dig` / `nslookup` - DNS queries (from bind-utils or ldns) - P2
- [ ] `traceroute` / `tracepath` - path tracing - P2
- [ ] **`ethtool`** - P1: NIC diagnostics and configuration
- [ ] `nmap` - network scanning (optional) - P3
- [ ] `tcpdump` - packet capture (optional) - P3

### 2.5 VPN Support
- [ ] OpenVPN client
- [ ] WireGuard support (kernel module + tools)
- [ ] IPsec support (strongswan or libreswan) - *optional*

### 2.6 Remote Access
- [ ] **SSH server (sshd) enabled** - P1: Essential for remote installation/rescue (archiso enables this!)
- [ ] SSH client (ssh, scp, sftp) - P1
- [ ] Key-based authentication works

### 2.7 Firewall
- [ ] nftables OR iptables available
- [ ] firewalld OR ufw - *optional convenience*

---

## 3. STORAGE & FILESYSTEMS

### 3.1 Partitioning Tools
- [x] `fdisk` - MBR/GPT partitioning (in rootfs)
- [x] `parted` - GPT partitioning (in squashfs)
- [ ] **`gdisk` / `sgdisk`** - P1: GPT-specific tools (better than fdisk for GPT)
- [x] `lsblk` - list block devices (in rootfs)
- [x] `blkid` - show UUIDs and labels (in rootfs)
- [x] `wipefs` - clear filesystem signatures (in rootfs)

### 3.2 Filesystem Support
- [x] ext4 (mkfs.ext4, e2fsck, tune2fs, resize2fs) - in rootfs
- [x] FAT32/vfat (mkfs.fat, fsck.fat) - required for ESP, in rootfs
- [ ] XFS (mkfs.xfs, xfs_repair) - P2
- [ ] **Btrfs (mkfs.btrfs, btrfs)** - P0 CRITICAL: Popular default, users expect it
- [ ] NTFS read/write (ntfs-3g) - P2: for Windows drives
- [ ] exFAT (exfatprogs) - P2: for USB drives and SD cards
- [x] ISO9660 (mount -t iso9660) - kernel module in initramfs
- [x] squashfs (for live systems) - kernel module + mksquashfs

### 3.3 LVM & RAID
- [ ] **LVM2 (pvcreate, vgcreate, lvcreate)** - P0 CRITICAL: Common storage setup
- [ ] mdadm for software RAID - P2
- [ ] dmraid for fake RAID - P2

### 3.4 Encryption
- [ ] **LUKS encryption (cryptsetup)** - P0 CRITICAL: Many users require encrypted root
- [ ] Encrypted root partition support - depends on cryptsetup
- [ ] crypttab for automatic unlock - depends on cryptsetup

### 3.5 Mount & Automount
- [x] `mount` / `umount` - in rootfs
- [x] `findmnt` - show mounted filesystems (in rootfs)
- [x] fstab support with UUID (install-tests generates it)
- [ ] systemd automount for removable media
- [ ] udisks2 for desktop automount - *optional*

### 3.6 Storage Drivers (Kernel Modules)
- [ ] SATA: ahci, ata_piix
- [ ] NVMe: nvme
- [ ] USB storage: usb-storage, uas
- [ ] SD cards: sdhci, mmc_block
- [x] SCSI: sr_mod (CD/DVD) - in config.rs
- [x] VirtIO: virtio_blk, virtio_scsi - in config.rs

### 3.7 Disk Health
- [ ] `smartctl` (smartmontools) - SMART monitoring
- [ ] `hdparm` - drive parameters
- [ ] `nvme-cli` - NVMe management

---

## 4. USER MANAGEMENT

### 4.1 User Operations
- [x] `useradd` - create users (in rootfs)
- [x] `usermod` - modify users (in rootfs)
- [x] `userdel` - delete users (in rootfs)
- [x] `passwd` - change passwords (in rootfs)
- [x] `chpasswd` - batch password setting (in rootfs + initramfs)
- [ ] `chage` - password expiry
- [x] `/etc/passwd` proper format (created by rootfs builder)
- [x] `/etc/shadow` proper format and permissions (0400)

### 4.2 Group Operations
- [x] `groupadd` - create groups (in rootfs + initramfs)
- [x] `groupmod` - modify groups (in rootfs)
- [x] `groupdel` - delete groups (in rootfs)
- [ ] `gpasswd` - group administration
- [x] `/etc/group` proper format (created by rootfs builder)
- [x] `/etc/gshadow` proper format (with 0600 permissions)

### 4.3 Default Groups
- [x] `wheel` - sudo access (created by install)
- [x] `audio` - audio devices
- [x] `video` - video devices
- [ ] `input` - input devices
- [ ] `storage` - removable media
- [ ] `optical` - CD/DVD drives
- [ ] `network` - network configuration
- [x] `users` - standard users group
- [x] `disk` - disk devices
- [x] `tty` - tty devices

### 4.4 Privilege Escalation
- [x] `sudo` installed and configured (in rootfs)
- [x] `/etc/sudoers` with `%wheel ALL=(ALL:ALL) ALL`
- [x] `visudo` for safe editing (in rootfs)
- [x] `su` for user switching (in rootfs)
- [x] PAM configuration proper (pam.rs)

### 4.5 Login System
- [x] getty on TTY1-6 (getty@.service)
- [x] agetty autologin option (systemd override)
- [x] Login shell works (bash)
- [x] `.bashrc` / `.bash_profile` sourced (created by filesystem.rs)
- [x] `/etc/profile` and `/etc/profile.d/` executed

---

## 5. CORE UTILITIES

### 5.1 GNU Coreutils (or compatible)
- [x] `ls`, `cp`, `mv`, `rm`, `mkdir`, `rmdir` - in rootfs
- [x] `cat`, `head`, `tail`, `tee` - in rootfs
- [x] `chmod`, `chown`, `chgrp` - in rootfs
- [x] `ln` (symlinks and hardlinks) - in rootfs
- [x] `touch`, `stat`, `file` - in rootfs
- [x] `wc`, `sort`, `uniq`, `cut` - in rootfs
- [x] `tr` - in rootfs
- [ ] `fold`, `fmt`
- [x] `echo`, `printf`, `yes` - in rootfs
- [x] `date` - in rootfs
- [ ] `cal`
- [x] `df`, `du` - in rootfs
- [x] `pwd`, `basename`, `dirname`, `realpath` - in rootfs
- [x] `env`, `printenv` - in rootfs
- [x] `sleep` - in rootfs
- [ ] `timeout`
- [ ] `tty`, `stty`
- [x] `id`, `whoami`, `groups` - in rootfs
- [x] `uname` - in rootfs
- [x] `seq` - in rootfs
- [ ] `shuf`
- [ ] `md5sum`, `sha256sum`, `sha512sum`
- [ ] `base64`
- [ ] `install`

### 5.2 Text Processing
- [x] `grep` (GNU grep with -P for PCRE) - in rootfs
- [x] `sed` (GNU sed) - in rootfs
- [x] `awk` (gawk) - in rootfs
- [x] `diff` - in rootfs
- [ ] `patch`
- [x] `less` (pager) - in rootfs
- [x] `nano` OR `vim` (text editor) - both in rootfs (vi, nano)

### 5.3 File Finding
- [x] `find` (GNU findutils) - in rootfs
- [ ] `locate` / `mlocate` - *optional*
- [x] `which` - in rootfs
- [ ] `whereis`
- [x] `xargs` - in rootfs

### 5.4 Archive Tools
- [x] `tar` (GNU tar with xz, gzip, bzip2 support) - in rootfs
- [x] `gzip`, `gunzip` - in rootfs
- [x] `bzip2`, `bunzip2` - in rootfs
- [x] `xz`, `unxz` - in rootfs
- [ ] `zstd` - increasingly common
- [ ] `zip`, `unzip` - for Windows compatibility
- [x] `cpio` - for initramfs, in rootfs

### 5.5 Shell
- [x] `bash` as /bin/bash and /bin/sh - in rootfs
- [ ] Tab completion (bash-completion)
- [x] Command history - built into bash
- [x] Job control (bg, fg, jobs) - built into bash
- [ ] `zsh` - *optional alternative*

---

## 6. SYSTEM SERVICES (systemd)

### 6.1 Core systemd
- [x] `systemctl` - service management (in rootfs + initramfs)
- [x] `journalctl` - log viewing (in rootfs + initramfs)
- [ ] `systemd-analyze` - boot analysis
- [x] `hostnamectl` - hostname management (in rootfs + initramfs)
- [x] `timedatectl` - time/date management (in rootfs + initramfs)
- [x] `localectl` - locale management (in rootfs + initramfs)
- [x] `loginctl` - session management (in rootfs)

### 6.2 Essential Services
- [x] `systemd-journald` - logging (in rootfs)
- [x] `systemd-logind` - login management (in rootfs)
- [x] `systemd-networkd` - networking (in rootfs)
- [x] `systemd-resolved` - DNS (in rootfs)
- [x] `chronyd` - NTP (using chrony instead of timesyncd)
- [x] `systemd-udevd` - device management (in rootfs)

### 6.3 Boot Services
- [x] `getty@ttyN` - virtual consoles (in systemd.rs)
- [x] `serial-getty@` - serial console (for VMs, in systemd.rs)
- [x] `systemd-boot` - bootloader (bootctl in rootfs)
- [ ] `systemd-boot-update` - auto-update entries

### 6.4 Timer Support
- [ ] systemd timers work (replacement for cron)
- [x] `systemd-tmpfiles` - temp file management (in rootfs)
- [ ] `systemd-sysusers` - system user creation

---

## 7. HARDWARE SUPPORT

### 7.1 CPU
- [ ] **Intel microcode (intel-ucode)** - P0 CRITICAL: CPU security/stability
- [ ] **AMD microcode (amd-ucode)** - P0 CRITICAL: CPU security/stability
- [ ] CPU frequency scaling (cpupower) - P2
- [ ] Temperature monitoring (lm_sensors) - P2

### 7.2 Memory
- [ ] Swap partition/file support
- [ ] zram/zswap - *optional*
- [x] `free` - memory stats (in rootfs)
- [x] `/proc/meminfo` readable

### 7.3 PCI/USB Detection
- [x] `lspci` (pciutils) - identify PCI hardware
- [ ] **`lsusb` (usbutils)** - P1: Users need to identify hardware
- [ ] `lshw` - *optional but useful*
- [ ] **`dmidecode`** - P1: SMBIOS/DMI info for hardware identification

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

### 7.10 LLM Inference & GPU Compute

> **Target:** 24GB+ RAM systems with dedicated GPU for local LLM inference

**Kernel Support (in kconfig):**
- [x] CONFIG_DRM_AMDGPU_USERPTR - GPU direct userspace memory access
- [x] CONFIG_HSA_AMD - ROCm heterogeneous compute support
- [x] CONFIG_LRU_GEN - MGLRU for better memory management under pressure
- [x] CONFIG_SCHED_CLASS_EXT - BPF schedulers (sched_ext)
- [x] CONFIG_TRANSPARENT_HUGEPAGE - Large memory allocations for models
- [x] CONFIG_HUGETLBFS - Explicit huge pages

**Userspace (installable via recipe):**
- [ ] ROCm stack (AMD GPUs) - `recipe install rocm`
- [ ] CUDA toolkit (NVIDIA GPUs) - `recipe install cuda`
- [ ] Vulkan compute - `recipe install vulkan-tools`
- [ ] llama.cpp - `recipe install llama-cpp`
- [ ] ollama - `recipe install ollama`

**System Tuning (P2):**
- [ ] `levitate-tune` daemon - auto-configure sysctls for hardware profile
  - vm.swappiness=10 (prefer RAM over swap)
  - vm.dirty_ratio tuned for NVMe
  - Transparent hugepage settings
  - CPU governor selection
- [ ] First-boot GPU detection and driver recommendation
- [ ] scx schedulers package for workload-specific scheduling

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

### 8.4 Future: Recipe as Universal Dependency Manager

> **Goal:** Replace leviso's custom DependencyResolver with `recipe` for ALL build dependencies.

Currently, leviso has a hardcoded DependencyResolver that downloads:
- Linux kernel source (git clone from kernel.org)
- Rocky ISO (curl from Rocky mirrors)
- recstrap, recfstab, recchroot (GitHub releases)
- recipe binary (manual build required)

**Future state:** ALL of these should be installable via `recipe`:
```bash
recipe install linux-source      # Kernel source tree
recipe install rocky-iso         # Rocky ISO for RPM extraction
recipe install leviso-tools      # recstrap, recfstab, recchroot, recipe
```

**Benefits:**
- Single tool for all dependency management
- Consistent versioning and updates
- Reproducible builds (recipe lockfile)
- No hardcoded GitHub URLs in leviso
- Standalone users get same experience as monorepo users

**Priority:** P2 (after recipe package manager is fully functional)

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

## 16. EU COMPLIANCE

> **Goal:** Meet EU Cyber Resilience Act (CRA) and GDPR requirements, enabling LevitateOS
> for use in EU public sector and enterprise environments.

### 16.1 Privacy by Default (GDPR)
- [x] No telemetry in base system - documented in PRIVACY.md
- [x] No phone-home behavior without explicit opt-in - documented in PRIVACY.md
- [x] Clear documentation of any data collection (if added later) - documented in PRIVACY.md
- [x] Data stays local unless user explicitly configures otherwise - documented in PRIVACY.md

### 16.2 Cyber Resilience Act (CRA) - Effective Dec 2027
- [ ] Reproducible builds (binary matches source)
- [x] Documented vulnerability disclosure process - see SECURITY.md
- [x] Security update policy (how long, how delivered) - see SECURITY.md
- [ ] SBOM (Software Bill of Materials) generation
- [ ] No known exploitable vulnerabilities at release
- [ ] CE marking documentation (when required)

### 16.3 Supply Chain Transparency
- [x] Document all upstream sources (Rocky RPMs, kernel.org, etc.) - see SUPPLY-CHAIN.md
- [x] Verify package signatures from upstream - ISO checksum verified, see SUPPLY-CHAIN.md
- [x] Mirror capability for EU organizations (self-hostable repos) - documented in SUPPLY-CHAIN.md
- [ ] Audit trail for package provenance

### 16.4 Sovereignty Features
- [x] Full offline installation capability - ISO contains complete system
- [x] No mandatory cloud dependencies - no telemetry, no required services
- [x] Recipe repos can be self-hosted - documented in SUPPLY-CHAIN.md
- [x] All crypto keys user-controllable - no vendor-locked keys

### 16.5 Auditability
- [x] Open source (already satisfied) - all code on GitHub
- [x] Build process documented and reproducible - see SUPPLY-CHAIN.md
- [ ] Configuration changes logged (optional audit mode)
- [ ] Kernel config documented and justified

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

> **Verified 2026-01-22** - See `docs/archiso-parity-checklist.md` for full analysis

Arch Linux ISO includes these packages. Status in LevitateOS noted.

### Network & WiFi
- `dhcpcd` - DHCP client (N/A - using NetworkManager)
- `iwd` - WiFi daemon - **MISSING (P1)**
- `wpa_supplicant` - WPA authentication - ✅ INCLUDED
- `wireless_tools` - iwconfig etc (deprecated)
- `wireless-regdb` - regulatory database - **MISSING (P1)**
- `ethtool` - NIC config - **MISSING (P1)**
- `modemmanager` - mobile broadband - MISSING (P2)

### Filesystems
- `btrfs-progs` - **MISSING (P0)**
- `dosfstools` - ✅ INCLUDED
- `e2fsprogs` - ✅ INCLUDED
- `exfatprogs` - MISSING (P2)
- `f2fs-tools` - MISSING (P3)
- `jfsutils` - MISSING (P3)
- `ntfs-3g` - MISSING (P2)
- `xfsprogs` - MISSING (P2)

### Disk Tools
- `cryptsetup` - LUKS - **MISSING (P0)**
- `dmraid` - MISSING (P3)
- `gptfdisk` (gdisk) - **MISSING (P1)**
- `hdparm` - MISSING (P2)
- `lvm2` - **MISSING (P0)**
- `mdadm` - MISSING (P2)
- `nvme-cli` - MISSING (P2)
- `parted` - ✅ INCLUDED (in squashfs)
- `sdparm` - MISSING (P3)
- `smartmontools` - MISSING (P2)

### Hardware
- `amd-ucode` - **MISSING (P0)**
- `intel-ucode` - **MISSING (P0)**
- `linux-firmware` - ✅ INCLUDED (partial)
- `linux-firmware-marvell` - MISSING (minor)
- `sof-firmware` - sound - **MISSING (P1)**
- `dmidecode` - **MISSING (P1)**
- `usbutils` - **MISSING (P1)**
- `pciutils` - ✅ INCLUDED

### Utilities
- `arch-install-scripts` - genfstab, arch-chroot - ✅ IMPLEMENTED (recfstab, recchroot)
- `diffutils` - ✅ INCLUDED
- `less` - ✅ INCLUDED
- `man-db` - MISSING (P1)
- `man-pages` - MISSING (P1)
- `nano` - ✅ INCLUDED
- `rsync` - MISSING (P2)
- `sudo` - ✅ INCLUDED
- `vim` - ✅ INCLUDED (vi)

### VPN
- `openconnect` - MISSING (P2)
- `openvpn` - MISSING (P2)
- `ppp` - MISSING (P2)
- `vpnc` - MISSING (P2)
- `wireguard-tools` - MISSING (P2)

### Recovery
- `ddrescue` - MISSING (P2)
- `testdisk` - MISSING (P2)
- Others - MISSING (P3)

### VM Support
- `hyperv` - MISSING (P2)
- `open-vm-tools` - MISSING (P2)
- `qemu-guest-agent` - MISSING (P2)
- `virtualbox-guest-utils-nox` - MISSING (P2)

---

## PRIORITY LEVELS

> **Updated based on archiso parity verification (2026-01-22)**

### P0 - Must Have (Blocking Daily Driver)
- [x] Boot and installation works (UEFI)
- [x] Network (Ethernet + DHCP via NetworkManager)
- [x] WiFi support (wpa_supplicant + firmware)
- [x] User management + sudo
- [x] Core utilities
- [x] **recfstab** - fstab generation helper (genfstab equivalent)
- [x] **recchroot** - arch-chroot equivalent
- [ ] **Intel/AMD microcode** - CPU security/stability
- [ ] **cryptsetup (LUKS)** - disk encryption
- [ ] **lvm2** - Logical Volume Manager
- [ ] **btrfs-progs** - Btrfs filesystem support

### P1 - Should Have (archiso Parity)
- [x] Volatile journal storage (prevent tmpfs fill)
- [x] do-not-suspend config (prevent sleep during install)
- [x] SSH server available (remote installation/rescue) - not enabled by default
- [~] Hardware probing: ~~lspci~~, lsusb, dmidecode (lspci done, others pending)
- [ ] ethtool (NIC diagnostics)
- [~] gdisk/sgdisk - NOT IN ROCKY 10.1 (parted/sfdisk sufficient)
- [ ] iwd (alternative WiFi)
- [ ] wireless-regdb (regulatory compliance)
- [ ] sof-firmware (Intel laptop sound)
- [x] ISO SHA512 checksum generation
- [ ] Man pages

### P2 - Nice to Have (Enhancement)
- BIOS/Legacy boot
- VPN support (WireGuard, OpenVPN)
- VM guest tools
- Recovery tools (testdisk, ddrescue)
- ModemManager (mobile broadband)
- XFS, exFAT, NTFS support
- **System tuner daemon** - auto-tune sysctls for hardware (vm.swappiness, vm.dirty_ratio)
- **First-boot GPU detection** - detect GPU, suggest ROCm/CUDA install via recipe
- **scx schedulers package** - BPF schedulers for sched_ext (kernel has CONFIG_SCHED_CLASS_EXT=y)

### P3 - Future
- Secure Boot signing
- Full accessibility (brltty, espeakup)
- SELinux/AppArmor
- Guided installer (like archinstall)

---

## UPDATING THIS DOCUMENT

When you implement something:
1. Change `[ ]` to `[x]`
2. Add test coverage to appropriate test crate
3. Update the TEST MATRIX section
4. Update `docs/archiso-parity-checklist.md` if it's an archiso parity item
5. Commit with message: `spec: Mark <item> as complete`

When you find something missing:
1. Add it to the appropriate section
2. Mark it as `[ ]`
3. Add a note about priority (P0/P1/P2/P3)
4. If archiso has it, add to `docs/archiso-parity-checklist.md`
5. Commit with message: `spec: Add <item> requirement`

### Priority Definitions

| Priority | Meaning | Example |
|----------|---------|---------|
| P0 | Blocking daily driver use | microcode, cryptsetup, lvm2, btrfs |
| P1 | archiso parity / should have | lsusb, SSH, ethtool |
| P2 | Nice to have | VPN, VM tools, recovery |
| P3 | Future / optional | Secure Boot, accessibility |
