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
- [x] `passwd` - interactive password setting
- [x] `nano` - text editor for config files

### Locale & Time
- [x] `localedef` - generate locales (in COREUTILS)
- [x] Timezone data (`/usr/share/zoneinfo/`)

### Bootloader
- [x] `bootctl` - systemd-boot installer (in COREUTILS)

### Networking (IMPLEMENTED in src/initramfs/network.rs - 482 lines)

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

### Recipe
- [ ] `recipe` binary in initramfs
- [ ] `recipe bootstrap /mnt` command

### Quality of Life
- [x] Proper shutdown/reboot (via `poweroff`/`reboot` aliases)
- [ ] Tab completion (bash-completion)
- [x] Welcome message with install instructions (/etc/motd)

### Documentation
- [ ] `levitate-docs` TUI documentation viewer (requires Node.js - post-install via recipe)

---

## Implementation Plan: Networking

> **STATUS: IMPLEMENTED** - See `src/initramfs/network.rs` (482 lines)
>
> This section documents what was implemented. The code is complete and wired up.

**Goal:** Add full networking support (NetworkManager + WiFi + Ethernet) to the live ISO so users can connect to networks and download packages.

**Scope:**
- Full NetworkManager with nmcli and wpa_supplicant
- Common WiFi firmware (Intel, Atheros, Realtek, Broadcom)
- Auto-start on boot

### Phase 1: Create network.rs module - DONE

**New file: `leviso/src/initramfs/network.rs`**

```rust
// Network binaries to copy
const NETWORK_BINARIES: &[&str] = &[
    "ip",              // iproute2 - network config
    "ping",            // connectivity test
];

const NETWORK_SBIN: &[&str] = &[
    "wpa_supplicant",  // WiFi authentication
    "wpa_cli",         // WiFi control
    "wpa_passphrase",  // PSK generation
];

const NETWORKMANAGER_BINARIES: &[&str] = &[
    "NetworkManager",  // Main daemon (usr/sbin)
    "nmcli",           // CLI tool (usr/bin)
];

// Systemd units to copy
const NETWORK_UNITS: &[&str] = &[
    "NetworkManager.service",
    "NetworkManager-wait-online.service",
    "NetworkManager-dispatcher.service",
    "wpa_supplicant.service",
];

pub fn setup_network(ctx: &BuildContext) -> Result<()>
```

**Functions needed:**
1. `copy_network_binaries()` - Copy NetworkManager, nmcli, wpa_supplicant, ip
2. `copy_networkmanager_configs()` - Copy /etc/NetworkManager/ configs
3. `copy_networkmanager_plugins()` - Copy /usr/lib64/NetworkManager/ plugins
4. `copy_network_units()` - Copy systemd service files
5. `copy_dbus_policies()` - Copy NetworkManager D-Bus policies
6. `enable_networkmanager()` - Symlink to multi-user.target.wants

### Phase 2: Add network kernel modules - DONE

**Already in `leviso/src/config.rs` lines 112-118:**

```rust
// Network - virtio (VM networking)
"kernel/drivers/net/virtio_net.ko.xz",
// Network - common ethernet drivers
"kernel/drivers/net/ethernet/intel/e1000/e1000.ko.xz",
"kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko.xz",
"kernel/drivers/net/ethernet/realtek/r8169.ko.xz",
```

**Note**: WiFi drivers are large. Use modprobe auto-loading rather than bundling all 88 drivers.

### Phase 3: Copy WiFi firmware - DONE

**Implemented in network.rs: `copy_wifi_firmware()`**

Copy firmware for common WiFi chipsets:
```
/usr/lib/firmware/iwlwifi-*      # Intel WiFi (most laptops)
/usr/lib/firmware/ath10k/        # Atheros
/usr/lib/firmware/ath11k/        # Newer Atheros
/usr/lib/firmware/rtlwifi/       # Realtek
/usr/lib/firmware/rtw88/         # Newer Realtek
/usr/lib/firmware/brcm/          # Broadcom
/usr/lib/firmware/mediatek/      # MediaTek
```

**Size estimate**: ~100-150MB for common firmware

### Phase 4: Update initramfs build - DONE

**Already in `leviso/src/initramfs/mod.rs`:**

```rust
pub mod network;  // Line 22

// In build_initramfs():
network::setup_network(&ctx)?;  // Line 144
```

### Files Created/Modified

| File | Status |
|------|--------|
| `src/initramfs/network.rs` | **DONE** - 482 lines |
| `src/initramfs/mod.rs` | **DONE** - `mod network` + `setup_network()` call |
| `src/config.rs` | **DONE** - virtio_net + ethernet modules at lines 112-118 |

### Directory Structure in Initramfs

```
/usr/sbin/NetworkManager
/usr/sbin/wpa_supplicant
/usr/sbin/wpa_cli
/usr/bin/nmcli
/sbin/ip
/etc/NetworkManager/NetworkManager.conf
/etc/NetworkManager/conf.d/
/usr/lib64/NetworkManager/          # Plugins
/usr/share/dbus-1/system.d/org.freedesktop.NetworkManager.conf
/usr/lib/systemd/system/NetworkManager.service
/usr/lib/firmware/iwlwifi-*         # Intel WiFi firmware
/usr/lib/firmware/ath10k/           # Atheros firmware
```

### Dependencies

NetworkManager requires (already in initramfs):
- D-Bus (already set up)
- systemd (already set up)
- polkit (may need to add)

### Verification

1. **Build test**: `cargo build` succeeds
2. **Boot test**: `cargo run -- run`
   - Check: `systemctl status NetworkManager` shows active
   - Check: `nmcli device` shows network interfaces
   - Check: `nmcli connection up <ethernet>` connects (if DHCP available)
3. **WiFi test** (on real hardware or with USB passthrough):
   - Check: `nmcli device wifi list` shows networks
   - Check: `nmcli device wifi connect <SSID> password <pass>` connects

### Estimated Size Impact

| Component | Size |
|-----------|------|
| NetworkManager + libs | ~15 MB |
| wpa_supplicant + libs | ~5 MB |
| iproute2 (ip) | ~1 MB |
| Common WiFi firmware | ~100-150 MB |
| **Total** | **~120-170 MB** |

### Boot Behavior

1. systemd starts
2. D-Bus socket activates
3. NetworkManager.service starts (WantedBy=multi-user.target)
4. NetworkManager auto-configures Ethernet via DHCP
5. WiFi available via `nmcli device wifi`

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Firmware bloat | Only include common chipsets, not all 354MB |
| Missing polkit | Test without, add if NetworkManager complains |
| D-Bus conflicts | NetworkManager D-Bus config must not conflict with existing |
| Boot slowdown | NetworkManager-wait-online.service can delay boot; don't enable it |

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
- **Initramfs IS the live environment** - No squashfs layer, no switch_root
- **`recipe` handles everything** - Both live queries AND installation to target disk

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
- [x] NetworkManager (in initramfs via network.rs)
- [x] systemd-resolved for DNS (in rootfs)
- [ ] /etc/resolv.conf configured
- [ ] /etc/hosts with localhost entries
- [ ] /etc/nsswitch.conf with proper hosts line

### 2.2 Ethernet
- [x] DHCP client works (NetworkManager)
- [x] Static IP configuration works (nmcli)
- [x] Link detection (cable plug/unplug) - NetworkManager
- [x] Gigabit speeds supported
- [x] Common drivers: e1000, e1000e, r8169 (in config.rs)

### 2.3 WiFi
- [x] wpa_supplicant installed (in initramfs via network.rs)
- [x] nmcli can scan networks (in initramfs)
- [ ] Can connect to WPA2-PSK network (untested)
- [ ] Can connect to WPA3 network (untested)
- [ ] Can connect to WPA2-Enterprise (802.1X)
- [x] WiFi firmware: Intel (iwlwifi), Atheros, Realtek, Broadcom (network.rs)
- [ ] wireless-regdb for regulatory compliance

### 2.4 Network Tools
- [x] `ip` - interface and routing configuration (in rootfs)
- [x] `ping` - connectivity testing (in rootfs)
- [x] `ss` - socket statistics (in rootfs)
- [x] `curl` - HTTP client (in rootfs)
- [x] `wget` - file downloads (in rootfs)
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
- [x] `fdisk` - MBR/GPT partitioning (in rootfs)
- [~] `parted` - GPT partitioning (in initramfs only, not rootfs)
- [ ] `gdisk` / `sgdisk` - GPT-specific tools
- [x] `lsblk` - list block devices (in rootfs)
- [x] `blkid` - show UUIDs and labels (in rootfs)
- [x] `wipefs` - clear filesystem signatures (in rootfs)

### 3.2 Filesystem Support
- [x] ext4 (mkfs.ext4, e2fsck, tune2fs, resize2fs) - in rootfs
- [x] FAT32/vfat (mkfs.fat, fsck.fat) - required for ESP, in rootfs
- [ ] XFS (mkfs.xfs, xfs_repair) - *optional*
- [ ] Btrfs (mkfs.btrfs, btrfs) - *optional but popular*
- [ ] NTFS read/write (ntfs-3g) - for Windows drives
- [ ] exFAT (exfatprogs) - for USB drives and SD cards
- [x] ISO9660 (mount -t iso9660) - kernel module in initramfs
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
- [ ] `/etc/gshadow` proper format

### 4.3 Default Groups
- [x] `wheel` - sudo access (created by install)
- [ ] `audio` - audio devices
- [ ] `video` - video devices
- [ ] `input` - input devices
- [ ] `storage` - removable media
- [ ] `optical` - CD/DVD drives
- [ ] `network` - network configuration
- [ ] `users` - standard users group

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
