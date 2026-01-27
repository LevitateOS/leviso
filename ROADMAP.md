# LevitateOS Daily Driver Specification

> **DOCUMENTATION NOTE:** This roadmap will be used to create the installation guide
> and user documentation. Keep descriptions clear, complete, and user-facing.

**Version:** 1.3
**Last Updated:** 2026-01-28
**Goal:** Everything a user needs to use LevitateOS as their primary operating system, competing directly with Arch Linux.

---

## ARCHISO PARITY STATUS

> **Reference:** See `docs/archiso-parity-checklist.md` for full details (verified 2026-01-22)

LevitateOS aims for parity with archiso - the Arch Linux installation ISO. This section tracks critical gaps.

### P0 - Critical Gaps (Blocking for Daily Driver)

| Gap | Impact | Status |
|-----|--------|--------|
| ~~Intel/AMD microcode~~ | ~~CPU bugs, security vulnerabilities~~ | INCLUDED (firmware) |
| ~~cryptsetup (LUKS)~~ | ~~No encrypted disk support~~ | ADDED 2026-01-24 |
| ~~lvm2~~ | ~~No LVM support~~ | ADDED 2026-01-24 |
| ~~btrfs-progs~~ | ~~No Btrfs support~~ | ADDED 2026-01-25 |
| ~~docs-tui~~ | ~~No on-screen installation docs~~ | ADDED 2026-01-24 |
| ~~tmux~~ | ~~No split-screen for docs~~ | ADDED 2026-01-24 |
| ~~terminfo~~ | ~~Terminal apps fail~~ | ADDED 2026-01-24 |

### P1 - Important Gaps

| Gap | Impact | Status |
|-----|--------|--------|
| ~~Volatile journal storage~~ | ~~Logs may fill tmpfs~~ | CONFIGURED |
| ~~do-not-suspend config~~ | ~~Live session may sleep during install~~ | CONFIGURED |
| ~~SSH server (sshd)~~ | ~~No remote installation/rescue~~ | AVAILABLE |
| ~~pciutils (lspci)~~ | ~~Cannot identify PCI hardware~~ | INCLUDED |
| ~~usbutils (lsusb)~~ | ~~Cannot identify USB devices~~ | ADDED 2026-01-25 |
| ~~dmidecode~~ | ~~Cannot read BIOS/DMI info~~ | ADDED 2026-01-24 |
| ~~ethtool~~ | ~~Cannot diagnose NICs~~ | ADDED 2026-01-24 |
| ~~gdisk/sgdisk~~ | ~~Only fdisk for GPT~~ | N/A - NOT IN ROCKY 10.1 (parted/sfdisk sufficient) |
| ~~iwd~~ | ~~Only wpa_supplicant for WiFi~~ | ADDED 2026-01-25 |
| ~~wireless-regdb~~ | ~~WiFi may violate regulations~~ | INCLUDED |
| ~~sof-firmware~~ | ~~Modern laptop sound may not work~~ | ADDED 2026-01-25 |
| ~~ISO SHA512 checksum~~ | ~~Users cannot verify downloads~~ | GENERATED |
| ~~checksums (md5/sha256/sha512)~~ | ~~Cannot verify file integrity~~ | ADDED 2026-01-24 |
| ~~network diag (dig/nslookup/tracepath)~~ | ~~Cannot debug DNS/routing~~ | ADDED 2026-01-24 |
| ~~disk health (smartctl/hdparm/nvme)~~ | ~~Cannot check drive health~~ | ADDED 2026-01-24 |
| ~~XFS (mkfs.xfs/xfs_repair)~~ | ~~No XFS support~~ | ADDED 2026-01-24 |

### What's Working (Verified)

| Feature | Status |
|---------|--------|
| UEFI boot | Verified in E2E test |
| NetworkManager + WiFi firmware | Enabled, firmware included |
| Autologin to root shell | Like archiso |
| machine-id empty | Regenerates on first boot |
| Hostname set ("levitateos") | Configured |
| recstrap (EROFS extraction) | Working |
| recfstab (genfstab equivalent) | Implemented |
| recchroot (arch-chroot equivalent) | Implemented |
| systemd as PID 1 | Verified |
| Serial console | Enabled |
| **docs-tui (levitate-docs)** | Auto-launches with tmux on tty1 |
| **tmux split-screen** | Shell left, docs right |
| **Keyboard shortcuts** | Shift+Tab switch, Ctrl+Left/Right resize, F1 help |
| **QEMU Performance** | KVM acceleration + 4 cores + 1920x1080 |
| Intel/AMD microcode | Included in firmware |
| LUKS encryption (cryptsetup) | Included |
| LVM (lvm2) | Included |
| Hardware detection (dmidecode, ethtool, lsusb) | Included |
| Disk health (smartctl, hdparm, nvme) | Included |
| XFS filesystem | Included |
| Btrfs filesystem | Included |
| Checksums (base64, md5sum, sha256sum, sha512sum) | Included |
| Network diagnostics (dig, nslookup, tracepath) | Included |
| Binary inspection (strings, hexdump) | Included |
| iwd WiFi daemon | Included |
| Intel SOF audio firmware | Included |

### Remaining Work Summary

| Category | Count | Priority | Notes |
|----------|-------|----------|-------|
| docs-tui Integration | DONE | P0 | Shell+docs split screen working (TUI UX refinements pending) |
| Microcode | DONE | P0 | Intel/AMD included in firmware |
| Encryption/Storage | DONE | P0 | cryptsetup, lvm2, btrfs-progs included |
| Hardware Detection | DONE | P1 | lspci, lsusb included |
| Networking | DONE | P1 | iwd, wireless-regdb, sof-firmware included |
| Shell UX | DONE | P1 | tmux, terminfo included |
| Package Manager | 5 | P1 | recipe commands not fully implemented |
| Filesystems | DONE | P2 | XFS, Btrfs, ext4, fat32 working |
| Network Tools | DONE | P2 | dig, nslookup, tracepath added |
| Disk Health | DONE | P2 | smartctl, hdparm, nvme added |
| Real HW Testing | 10 | P1 | Needs testing on physical hardware |

**All archiso-parity RPMs now extracted** - btrfs-progs, usbutils, iwd, wireless-regdb, sof-firmware are all included.

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

The ISO uses an EROFS-based live environment (Enhanced Read-Only File System):

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel
│   └── initramfs.img     # Tiny (~5MB) - mounts EROFS
├── live/
│   └── filesystem.erofs  # COMPLETE system (~350MB compressed)
└── EFI/...               # Bootloader
```

**Boot flow:**
1. Kernel + tiny initramfs boot
2. Initramfs mounts filesystem.erofs (read-only)
3. Initramfs mounts overlay (tmpfs for writes)
4. switch_root to overlay
5. User has FULL daily driver system

**Installation flow:**
1. Boot ISO -> live environment (from EROFS)
2. Run: `recstrap /dev/vda`
3. Reboot into installed system

**Key insight:** The EROFS rootfs IS the complete system. Live = Installed.

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
> It extracts rootfs to target. User does EVERYTHING else manually.

### Why EROFS + recstrap?

Like Arch's `pacstrap`, LevitateOS uses `recstrap` (recipe + strap) to extract the system.

**Problems with cpio initramfs + tarball:**
- ~400MB RAM usage just for live environment
- Two sources of truth (initramfs + tarball have different content)
- Need complex logic to copy networking from initramfs to installed system

**Solution - EROFS architecture:**
- Single source of truth: filesystem.erofs has EVERYTHING
- Less RAM: EROFS reads from disk, not all in RAM
- Better random-access performance than squashfs
- **Live = Installed:** exact same files

### What recstrap does

```bash
recstrap /mnt                    # Extract rootfs to /mnt
recstrap /mnt --rootfs /path     # Custom rootfs location
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

### EROFS architecture

```
ISO (~400MB):
├── initramfs.img (~5MB) - tiny, just mounts EROFS
└── live/filesystem.erofs (~350MB compressed)
    ├── All binaries (bash, coreutils, systemd...)
    ├── NetworkManager + wpa_supplicant
    ├── ALL firmware (WiFi, GPU, sound, BT)
    ├── recipe package manager
    └── recstrap binary

Live boot: EROFS mounted + overlay for writes
Installation: extract EROFS to /mnt
Result: Live = Installed (same files!)
```

### Implementation status

**EROFS rootfs builder:**
- [x] Create `src/artifact/rootfs.rs` - build complete system
- [x] Include ALL binaries (combine rootfs + initramfs content)
- [x] Include NetworkManager, wpa_supplicant
- [x] Include ALL firmware (~350MB)
- [x] Generate filesystem.erofs with mkfs.erofs

**Tiny initramfs:**
- [x] Create `src/artifact/initramfs.rs` - minimal boot initramfs (~5MB)
- [x] Mount EROFS read-only
- [x] Mount overlay (tmpfs) for writes
- [x] switch_root to live system

**recstrap (sibling directory: ../recstrap/):**
- [x] Extract EROFS rootfs to target directory

**Integration:**
- [x] Update ISO builder for new layout
- [x] Include recstrap in rootfs
- [x] Update welcome message to show manual install steps

---

## Build System & CLI

### CLI Commands

**Build Commands:**
- [x] `leviso build` - Full build (kernel + rootfs + initramfs + ISO)
- [x] `leviso build kernel [--clean]` - Kernel compilation only
- [x] `leviso build rootfs` - EROFS image only
- [x] `leviso build initramfs` - Live initramfs only
- [x] `leviso build iso` - ISO only

**Runtime Commands:**
- [x] `leviso run [--no-disk] [--disk-size SIZE]` - GUI boot in QEMU (UEFI)
- [x] `leviso test [--timeout SECS]` - Headless boot verification with serial capture

**Information Commands:**
- [x] `leviso show config` - Show build configuration
- [x] `leviso show rootfs` - Show EROFS image info
- [x] `leviso show status` - Show build status (what needs rebuilding)

**Cleanup Commands:**
- [x] `leviso clean` - Remove output artifacts (default)
- [x] `leviso clean kernel` - Remove kernel build
- [x] `leviso clean iso` - Remove ISO and initramfs
- [x] `leviso clean rootfs` - Remove EROFS rootfs
- [x] `leviso clean downloads` - Remove downloaded sources
- [x] `leviso clean cache` - Clear tool cache (~/.cache/levitate/)
- [x] `leviso clean all` - Remove everything

**Dependency Management:**
- [x] `leviso download [linux|rocky|tools]` - Download dependencies manually
- [x] `leviso preflight [--strict]` - Validate build environment

**Extraction & Inspection:**
- [x] `leviso extract rocky` - Extract Rocky ISO contents
- [x] `leviso extract rootfs [-o OUTPUT]` - Extract EROFS contents

### Build Intelligence

**Incremental Building:**
- [x] Kernel compile detection (skips if unchanged)
- [x] Kernel install detection (skips if unchanged)
- [x] Rootfs rebuild detection (hash-based)
- [x] Initramfs rebuild detection (hash-based)
- [x] ISO rebuild detection (component-based)

**License Tracking:**
- [x] Package registration
- [x] Binary tracking
- [x] License file copying
- [x] SPDX compliance

**Preflight Checks:**
- [x] Host tools validation (erofs-utils, mkfs commands, etc.)
- [x] Dependency resolution
- [x] Environment verification
- [x] KVM/QEMU availability

---

## Current State (What's Already Working)

### Build Artifacts
- [x] EROFS rootfs image (~350MB compressed)
- [x] Tiny live initramfs (~5MB, busybox-based)
- [x] Install initramfs (~30-50MB, systemd-based)
- [x] Bootable hybrid BIOS/UEFI ISO
- [x] UKI (Unified Kernel Images) for installed systems
- [x] ISO SHA512 checksum generation

### Live Environment (Initramfs)
- [x] Downloads Rocky 10 ISO for userspace binaries
- [x] Extracts rootfs for sourcing binaries
- [x] Builds initramfs with bash + coreutils
- [x] Creates bootable hybrid BIOS/UEFI ISO
- [x] QEMU test command (`cargo run -- test` / `cargo run -- run`)
- [x] efivarfs mounted for UEFI verification
- [x] Systemd as PID 1 (boots to multi-user.target)
- [x] Basic systemd units (getty, serial-console)
- [x] Serial console with CLOCAL support for virtual serial

### System Components (9 Build Phases)
- [x] Phase 1: Filesystem (FHS directories, merged /usr)
- [x] Phase 2: Binaries (shell, coreutils, auth, systemd)
- [x] Phase 3: Systemd (units, udev, tmpfiles, getty)
- [x] Phase 4: D-Bus (dbus-broker, socket activation)
- [x] Phase 5: Services (network, NTP, SSH, PAM, kernel modules)
- [x] Phase 6: Configuration (/etc files, timezone, locale, terminfo)
- [x] Phase 7: Packages (recipe, bootloader)
- [x] Phase 8: Firmware (linux-firmware, microcode, keymaps)
- [x] Phase 9: Final (welcome message, installation tools, docs-tui)

### Desktop Services (Optional Components)
- [x] Bluetooth (bluez)
- [x] PipeWire audio (with PulseAudio compatibility)
- [x] Polkit authorization
- [x] UDisks2 (disk management)
- [x] UPower (power management)

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

These are known gaps in the live environment (EROFS rootfs):

### Critical Tools (P0 - IMPLEMENTED)
- [x] `recfstab` - generate fstab from mounted filesystems (genfstab equivalent)
- [x] `recchroot` - enter installed system with proper mounts (arch-chroot equivalent)
- [x] `cryptsetup` - LUKS disk encryption
- [x] `lvm2` - Logical Volume Manager (pvcreate, vgcreate, lvcreate)
- [x] `btrfs-progs` - Btrfs filesystem tools

### Important Tools (P1)
- [~] `gdisk` / `sgdisk` - N/A - NOT IN ROCKY 10.1 (parted/sfdisk sufficient)
- [x] `pciutils` (lspci) - identify PCI hardware
- [x] `usbutils` (lsusb) - identify USB devices
- [x] `dmidecode` - BIOS/DMI information
- [x] `ethtool` - NIC diagnostics and configuration
- [x] `iwd` - alternative WiFi daemon (iwctl)
- [x] `wireless-regdb` - WiFi regulatory database

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

## Networking Implementation

> **STATUS: IMPLEMENTED** - See `src/build/network.rs`

| Component | Status | Location |
|-----------|--------|----------|
| NetworkManager | Working | `src/build/network.rs` |
| wpa_supplicant | Working | `src/build/network.rs` |
| nmcli / nmtui | Working | Included with NetworkManager |
| WiFi firmware | Working | Intel, Atheros, Realtek, Broadcom, MediaTek |
| Ethernet modules | Working | virtio_net, e1000, e1000e, r8169 |
| Auto-start | Working | NetworkManager.service enabled |

### Verification Commands
```bash
systemctl status NetworkManager   # Should show active
nmcli device                      # Should list interfaces
nmcli device wifi list            # Should show networks (real hardware)
```

### WiFi & Audio (Included)
- [x] iwd - alternative WiFi daemon (iwctl)
- [x] wireless-regdb - regulatory compliance
- [x] sof-firmware - Intel Sound Open Firmware

### Implementation status
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
- [x] `recipe` binary in rootfs (/usr/bin/recipe)
- [x] Recipe configuration in /etc/recipe/config.toml
- [ ] `recipe search` / `recipe install` commands (package manager functionality)

### Quality of Life
- [x] Proper shutdown/reboot (via `poweroff`/`reboot` aliases)
- [ ] Tab completion (bash-completion)
- [x] Welcome message with install instructions (/etc/motd)

### Documentation
- [x] `levitate-docs` TUI documentation viewer (standalone binary, auto-launches on live boot)

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
cargo run -- build rootfs   # Build EROFS rootfs image
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
- **EROFS-based architecture** - Tiny initramfs (~5MB) mounts EROFS rootfs + overlay, then switch_root
- **`recipe` handles everything** - Both live queries AND installation to target disk

---

## 1. BOOT & INSTALLATION

### 1.0 ISO Integrity Verification (P1)

archiso provides checksum files so users can verify their download.

- [x] **SHA512 checksum** - Generate `levitateos-YYYY.MM.DD.iso.sha512` during build
- [ ] **GPG signature** - Sign checksum file for release verification
- [ ] Document verification process on website

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
- [x] Extract rootfs to disk - verified in E2E test
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
- [x] wpa_supplicant installed (in rootfs via network.rs)
- [x] nmcli can scan networks (in rootfs)
- [ ] Can connect to WPA2-PSK network (untested on real hardware)
- [ ] Can connect to WPA3 network (untested)
- [ ] Can connect to WPA2-Enterprise (802.1X)
- [x] WiFi firmware: Intel (iwlwifi), Atheros, Realtek, Broadcom (network.rs)
- [x] **wireless-regdb** - Required for legal WiFi operation
- [x] **iwd** - Alternative WiFi daemon
- [x] **sof-firmware** - Modern laptop sound (Intel SOF)

### 2.4 Network Tools
- [x] `ip` - interface and routing configuration (in rootfs)
- [x] `ping` - connectivity testing (in rootfs)
- [x] `ss` - socket statistics (in rootfs)
- [x] `curl` - HTTP client (in rootfs)
- [x] `wget` - file downloads (in rootfs)
- [x] `dig` / `nslookup` - DNS queries (from bind-utils)
- [x] `tracepath` - path tracing
- [x] **`ethtool`** - NIC diagnostics and configuration
- [ ] `nmap` - network scanning (optional) - P3
- [ ] `tcpdump` - packet capture (optional) - P3

### 2.5 VPN Support
- [ ] OpenVPN client
- [ ] WireGuard support (kernel module + tools)
- [ ] IPsec support (strongswan or libreswan) - *optional*

### 2.6 Remote Access
- [x] **SSH server (sshd) available** - Essential for remote installation/rescue
- [x] SSH client (ssh, scp, sftp)
- [x] SSH host keys pre-generated
- [ ] Key-based authentication works (needs testing)

### 2.7 Firewall
- [ ] nftables OR iptables available
- [ ] firewalld OR ufw - *optional convenience*

---

## 3. STORAGE & FILESYSTEMS

### 3.1 Partitioning Tools
- [x] `fdisk` - MBR/GPT partitioning (in rootfs)
- [x] `parted` - GPT partitioning (in rootfs)
- [~] **`gdisk` / `sgdisk`** - N/A - NOT IN ROCKY 10.1 (parted/sfdisk sufficient)
- [x] `lsblk` - list block devices (in rootfs)
- [x] `blkid` - show UUIDs and labels (in rootfs)
- [x] `wipefs` - clear filesystem signatures (in rootfs)

### 3.2 Filesystem Support
- [x] ext4 (mkfs.ext4, e2fsck, tune2fs, resize2fs) - in rootfs
- [x] FAT32/vfat (mkfs.fat, fsck.fat) - required for ESP, in rootfs
- [x] XFS (mkfs.xfs, xfs_repair) - in rootfs
- [x] **Btrfs (mkfs.btrfs, btrfs)** - in rootfs
- [ ] NTFS read/write (ntfs-3g) - P2: for Windows drives
- [ ] exFAT (exfatprogs) - P2: for USB drives and SD cards
- [x] ISO9660 (mount -t iso9660) - kernel module in initramfs
- [x] EROFS (for live systems) - kernel module + mkfs.erofs

### 3.3 LVM & RAID
- [x] **LVM2 (pvcreate, vgcreate, lvcreate)** - in rootfs
- [ ] mdadm for software RAID - P2
- [ ] dmraid for fake RAID - P2

### 3.4 Encryption
- [x] **LUKS encryption (cryptsetup)** - in rootfs
- [ ] Encrypted root partition support - depends on initramfs integration
- [ ] crypttab for automatic unlock - depends on cryptsetup

### 3.5 Mount & Automount
- [x] `mount` / `umount` - in rootfs
- [x] `findmnt` - show mounted filesystems (in rootfs)
- [x] fstab support with UUID (install-tests generates it)
- [ ] systemd automount for removable media
- [x] udisks2 for desktop automount

### 3.6 Storage Drivers (Kernel Modules)
- [x] SATA: ahci, ata_piix
- [x] NVMe: nvme
- [x] USB storage: usb-storage, uas
- [ ] SD cards: sdhci, mmc_block
- [x] SCSI: sr_mod (CD/DVD) - in config.rs
- [x] VirtIO: virtio_blk, virtio_scsi - in config.rs

### 3.7 Disk Health
- [x] `smartctl` (smartmontools) - SMART monitoring
- [x] `hdparm` - drive parameters
- [x] `nvme-cli` - NVMe management

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
- [x] `md5sum`, `sha256sum`, `sha512sum` - in rootfs
- [x] `base64` - in rootfs
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

### 5.6 Binary Inspection
- [x] `strings` - extract printable strings
- [x] `hexdump` - hex/binary viewer

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
- [x] **Intel microcode (intel-ucode)** - CPU security/stability (microcode_ctl)
- [x] **AMD microcode (amd-ucode)** - CPU security/stability (linux-firmware)
- [ ] CPU frequency scaling (cpupower) - P2
- [ ] Temperature monitoring (lm_sensors) - P2

### 7.2 Memory
- [ ] Swap partition/file support
- [ ] zram/zswap - *optional*
- [x] `free` - memory stats (in rootfs)
- [x] `/proc/meminfo` readable

### 7.3 PCI/USB Detection
- [x] `lspci` (pciutils) - identify PCI hardware
- [x] **`lsusb` (usbutils)** - identify USB hardware
- [ ] `lshw` - *optional but useful*
- [x] **`dmidecode`** - SMBIOS/DMI info for hardware identification

### 7.4 Input Devices
- [x] Keyboard works (all layouts via loadkeys)
- [ ] Mouse works (PS/2 and USB)
- [ ] Touchpad works (libinput)
- [x] Keymaps in /usr/share/kbd/keymaps/

### 7.5 Display (Framebuffer/Console)
- [ ] Console fonts (terminus-font or similar)
- [ ] `setfont` - change console font
- [ ] 80x25 minimum, 1920x1080 framebuffer preferred
- [ ] Virtual consoles (Ctrl+Alt+F1-F6)

### 7.6 Audio (Console/Headless)
- [x] PipeWire audio daemon (with PulseAudio compatibility)
- [ ] ALSA utilities (alsa-utils) - *optional for server*
- [ ] `amixer`, `alsamixer` - *optional*

### 7.7 Graphics (Optional - for Desktop)
- [ ] Intel graphics (i915)
- [ ] AMD graphics (amdgpu)
- [ ] NVIDIA (nouveau or proprietary)
- [ ] VirtualBox Guest Additions
- [ ] VMware SVGA driver
- [ ] QXL for QEMU/KVM

### 7.8 Bluetooth
- [x] BlueZ stack
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
- [x] `recipe` binary installed in rootfs
- [x] Recipe configuration in /etc/recipe/config.toml
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
- [x] virtio_blk - block devices
- [x] virtio_net - networking
- [x] virtio_scsi - SCSI
- [x] virtio_console - console
- [ ] virtio_balloon - memory ballooning
- [ ] virtio_gpu - graphics

---

## 11. SECURITY

### 11.1 Basic Security
- [x] `/etc/shadow` permissions 0400
- [ ] Root account locked by default? (configurable)
- [x] Password hashing (SHA-512)
- [ ] Failed login delays

### 11.2 SSH Security
- [ ] Root login disabled by default
- [ ] Key-based auth preferred
- [x] SSH host keys generated

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
- [x] `fsck` for all supported filesystems
- [ ] `testdisk` - *optional but useful*
- [ ] `ddrescue` - *optional*

### 12.2 Diagnostic Tools
- [x] `dmesg` - kernel messages
- [x] `journalctl -b` - boot logs
- [x] `systemctl --failed` - failed services
- [x] `/var/log/` directory structure

### 12.3 Performance Tools
- [x] `top` / `htop`
- [x] `ps` - process listing
- [x] `kill`, `killall`, `pkill`
- [ ] `nice`, `renice`
- [ ] `iostat`, `vmstat` - *optional*

---

## 13. LOCALIZATION

### 13.1 Locale
- [x] UTF-8 support (en_US.UTF-8 default)
- [x] locale-gen or equivalent
- [x] `/etc/locale.conf`
- [ ] `/etc/locale.gen`

### 13.2 Timezone
- [x] tzdata installed
- [x] `/etc/localtime` symlink
- [x] `timedatectl set-timezone`

### 13.3 Keyboard
- [x] US layout default
- [x] Other layouts available
- [x] `/etc/vconsole.conf`

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

### 14.4 Offline Documentation
- [x] `levitate-docs` TUI documentation viewer
- [x] Auto-launches with tmux on live boot

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
- `iwd` - WiFi daemon - INCLUDED
- `wpa_supplicant` - WPA authentication - INCLUDED
- `wireless_tools` - iwconfig etc (deprecated)
- `wireless-regdb` - regulatory database - INCLUDED
- `ethtool` - NIC config - INCLUDED
- `modemmanager` - mobile broadband - MISSING (P2)

### Filesystems
- `btrfs-progs` - INCLUDED
- `dosfstools` - INCLUDED
- `e2fsprogs` - INCLUDED
- `exfatprogs` - MISSING (P2)
- `f2fs-tools` - MISSING (P3)
- `jfsutils` - MISSING (P3)
- `ntfs-3g` - MISSING (P2)
- `xfsprogs` - INCLUDED

### Disk Tools
- `cryptsetup` - LUKS - INCLUDED
- `dmraid` - MISSING (P3)
- `gptfdisk` (gdisk) - N/A - NOT IN ROCKY 10.1 (parted/sfdisk sufficient)
- `hdparm` - INCLUDED
- `lvm2` - INCLUDED
- `mdadm` - MISSING (P2)
- `nvme-cli` - INCLUDED
- `parted` - INCLUDED (in rootfs)
- `sdparm` - MISSING (P3)
- `smartmontools` - INCLUDED

### Hardware
- `amd-ucode` - INCLUDED (via linux-firmware)
- `intel-ucode` - INCLUDED (microcode_ctl)
- `linux-firmware` - INCLUDED
- `linux-firmware-marvell` - MISSING (minor)
- `sof-firmware` - sound - INCLUDED (alsa-sof-firmware)
- `dmidecode` - INCLUDED
- `usbutils` - INCLUDED
- `pciutils` - INCLUDED

### Utilities
- `arch-install-scripts` - genfstab, arch-chroot - IMPLEMENTED (recfstab, recchroot)
- `diffutils` - INCLUDED
- `less` - INCLUDED
- `man-db` - MISSING (P1)
- `man-pages` - MISSING (P1)
- `nano` - INCLUDED
- `rsync` - MISSING (P2)
- `sudo` - INCLUDED
- `vim` - INCLUDED (vi)

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
- [x] **Intel/AMD microcode** - CPU security/stability
- [x] **cryptsetup (LUKS)** - disk encryption
- [x] **lvm2** - Logical Volume Manager
- [x] **btrfs-progs** - Btrfs filesystem support

### P1 - Should Have (archiso Parity)
- [x] Volatile journal storage (prevent tmpfs fill)
- [x] do-not-suspend config (prevent sleep during install)
- [x] SSH server available (remote installation/rescue) - not enabled by default
- [x] Hardware probing: lspci, lsusb, dmidecode
- [x] ethtool (NIC diagnostics)
- [~] gdisk/sgdisk - NOT IN ROCKY 10.1 (parted/sfdisk sufficient)
- [x] iwd (alternative WiFi)
- [x] wireless-regdb (regulatory compliance)
- [x] sof-firmware (Intel laptop sound)
- [x] ISO SHA512 checksum generation
- [ ] Man pages

### P2 - Nice to Have (Enhancement)
- BIOS/Legacy boot
- VPN support (WireGuard, OpenVPN)
- VM guest tools
- Recovery tools (testdisk, ddrescue)
- ModemManager (mobile broadband)
- exFAT, NTFS support
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
