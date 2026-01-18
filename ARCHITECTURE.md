# LevitateOS ISO Builder - Architecture Sources

This document explains what concepts were borrowed from Arch Linux's `archiso` tool versus what comes from the Rocky Linux minimal ISO.

## From Arch Linux (archiso)

### Profile-Based Configuration

```
profile/
├── airootfs/      # Files overlaid onto the root filesystem
└── packages.txt   # List of packages to include
```

**Why:** Archiso's profile system is clean and intuitive. You define what packages you want and what files to overlay - the tool handles the rest.

### airootfs Overlay

The `airootfs/` directory is copied directly onto the root filesystem after packages are installed. This is how archiso handles customization:

- `airootfs/etc/hostname` → `/etc/hostname`
- `airootfs/etc/motd` → `/etc/motd`
- `airootfs/etc/os-release` → `/etc/os-release`

**Why:** Simple file-based customization without complex scripting.

### Autologin via systemd Drop-in

```
airootfs/etc/systemd/system/getty@tty1.service.d/autologin.conf
```

```ini
[Service]
ExecStart=
ExecStart=-/sbin/agetty -o '-p -f -- \\u' --noclear --autologin root - $TERM
```

**Why:** This is the standard systemd way to override service behavior, used by archiso for live environments.

### Live Boot with SquashFS

The concept of:
1. Building a complete rootfs
2. Compressing it into squashfs
3. Booting from RAM with overlay

**Why:** Proven approach for live Linux systems. Fast boot, no disk writes needed.

### Boot Menu Structure

GRUB and ISOLINUX configurations with:
- Default "Live" entry
- "Safe Mode" entry (nomodeset)
- "Boot from local drive" option

**Why:** Standard live ISO boot menu pattern.

---

## From Rocky Linux Minimal ISO

### Pre-built Binary Packages

```
vendor/images/Rocky-10-latest-x86_64-minimal.iso
└── Minimal/Packages/{a-z}/*.rpm
```

Instead of compiling from source, we extract pre-built RPMs:
- kernel-6.12.0
- systemd-257
- glibc, bash, coreutils, etc.

**Why:**
- Faster builds (minutes vs hours)
- Known-working binaries
- Proper EL10 compatibility
- Security patches included

### RPM-based Installation

```rust
run_cmd("rpm", &["--root", &rootfs, "-ivh", "--nodeps", rpm_path])?;
```

**Why:** Rocky packages are RPMs. Using `rpm --root` installs them into our custom rootfs.

### dracut for initramfs

Rocky uses dracut (not mkinitcpio like Arch). We use dracut with the `dmsquash-live` module:

```rust
chroot(&rootfs, "dracut --add 'dmsquash-live' /boot/initramfs-live.img")
```

**Why:** dracut's `dmsquash-live` module handles live boot from squashfs, matching how Fedora/Rocky live ISOs work.

### EFI Boot Chain

From Rocky's boot infrastructure:
- `shim-x64` - Secure Boot shim
- `grub2-efi-x64` - GRUB for EFI
- Standard Red Hat EFI layout

**Why:** Proven EFI boot chain with Secure Boot support.

### Package Selection

Our `packages.txt` is based on Rocky's minimal install group, plus additions:

| Category | Packages | Source |
|----------|----------|--------|
| Core | glibc, filesystem, basesystem | Rocky minimal |
| Init | systemd, systemd-udev, dbus | Rocky minimal |
| Kernel | kernel, kernel-core, kernel-modules | Rocky minimal |
| Network | NetworkManager | Rocky minimal |
| Live boot | dracut-live, squashfs-tools | Rocky repos |
| User tools | tmux, nano | Added for convenience |

---

## Comparison Table

| Aspect | Arch (archiso) | Rocky ISO | LevitateOS |
|--------|----------------|-----------|------------|
| Package format | pacman (.pkg.tar.zst) | RPM | RPM (from Rocky) |
| Package source | Arch repos | Rocky ISO | Rocky ISO |
| Profile system | archiso profiles | N/A | archiso-style |
| Overlay method | airootfs/ | N/A | airootfs/ |
| initramfs tool | mkinitcpio | dracut | dracut |
| Live module | archiso hooks | dmsquash-live | dmsquash-live |
| Bootloader | syslinux/GRUB | GRUB/shim | GRUB/shim |
| Compression | squashfs | squashfs | squashfs |

---

## Why This Hybrid?

**Arch's strengths:**
- Clean, declarative profile system
- Simple overlay mechanism
- Well-documented ISO building process

**Rocky's strengths:**
- Enterprise-grade packages
- Long-term support
- Pre-built binaries (no compilation)
- Secure Boot support out of the box

**LevitateOS combines both:**
- Arch's simple, hackable build system
- Rocky's stable, pre-built packages
- Fast builds without sacrificing quality

---

## Build Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    ARCH CONCEPTS                            │
├─────────────────────────────────────────────────────────────┤
│  profile/packages.txt    →  Package list                    │
│  profile/airootfs/       →  File overlay                    │
│  Boot menu structure     →  GRUB/ISOLINUX configs           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  LEVITATEISO BUILD                          │
├─────────────────────────────────────────────────────────────┤
│  1. Read packages.txt (Arch-style)                          │
│  2. Extract RPMs from Rocky ISO (Rocky packages)            │
│  3. Install with rpm --root (RPM tooling)                   │
│  4. Apply airootfs overlay (Arch-style)                     │
│  5. Generate initramfs with dracut (Rocky/Fedora)           │
│  6. Create squashfs (common)                                │
│  7. Assemble ISO with xorriso (common)                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    ROCKY PACKAGES                           │
├─────────────────────────────────────────────────────────────┤
│  kernel-6.12.0           →  Linux kernel                    │
│  systemd-257             →  Init system                     │
│  glibc, bash, coreutils  →  Userspace                       │
│  dracut + dmsquash-live  →  Live boot support               │
│  shim-x64 + grub2-efi    →  EFI boot chain                  │
└─────────────────────────────────────────────────────────────┘
```

---

## From LevitateOS (`recipe` Package Manager)

### The `recipe` Program

LevitateOS has its own package manager called `recipe` that uses S-expression recipes (`.recipe` files). It's designed to be **self-sufficient** - no external package managers required.

**Key Commands:**
```bash
recipe install <package>     # Install a package with dependencies
recipe remove <package>      # Remove a package
recipe list                  # List available packages
recipe info <package>        # Show package info
recipe deps <package>        # Show dependency tree
recipe bootstrap /mnt        # Install base system (like pacstrap)
```

### `recipe bootstrap` - Like Arch's `pacstrap`

The `recipe bootstrap` command is modeled directly after Arch's `pacstrap`:

| Arch | LevitateOS |
|------|------------|
| `pacstrap /mnt base linux linux-firmware` | `recipe bootstrap /mnt` |

**What `recipe bootstrap` installs (BASE_PACKAGES):**
```rust
const BASE_PACKAGES: &[&str] = &[
    "base",
    "linux",
    "linux-firmware",
    "systemd",
    "networkmanager",
    "bash",
    "coreutils",
    "util-linux",
    "recipe",  // Self-hosting: recipe installs itself
];
```

**Bootstrap Process:**
1. Verify all base recipes exist
2. Create filesystem hierarchy (`/bin`, `/etc`, `/usr`, etc.)
3. Install base packages with dependencies
4. Copy recipes to target for self-hosting
5. Save installed database to target

### Installation Flow (from manual-install.ts)

The documented LevitateOS installation follows Arch's workflow:

```
Boot ISO → Partition → Format → Mount → recipe bootstrap → Configure → Reboot
```

**Expected Commands in Live ISO:**

| Category | Commands | Purpose |
|----------|----------|---------|
| System | `ls`, `loadkeys`, `timedatectl` | Basic operations |
| Network | `nmcli`, `ping` | WiFi/connectivity |
| Disk | `lsblk`, `wipefs`, `parted` | Disk management |
| Filesystem | `mkfs.fat`, `mkfs.ext4`, `mount` | Formatting/mounting |
| Install | `recipe bootstrap /mnt` | Base system install |
| Config | `blkid`, `nano`, `chroot` | System configuration |
| Users | `passwd`, `useradd`, `chmod` | User management |
| Boot | `bootctl` | systemd-boot installation |
| Services | `systemctl`, `systemd-machine-id-setup` | Service management |
| Locale | `locale-gen`, `ln`, `hwclock` | Localization |

---

## The Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  LAYER 1: LEVITATEISO (ISO Builder)                        │
│  ─────────────────────────────────────────────────────────  │
│  • Extracts packages from Rocky ISO                        │
│  • Applies airootfs overlay (Arch-style)                   │
│  • Creates bootable live ISO                               │
│  • Output: levitateos.iso                                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ (User boots this ISO)
┌─────────────────────────────────────────────────────────────┐
│  LAYER 2: LIVE ENVIRONMENT                                 │
│  ─────────────────────────────────────────────────────────  │
│  • Boots to root shell (autologin)                         │
│  • Has all tools for installation                          │
│  • User runs: recipe bootstrap /mnt                        │
│  • User configures: fstab, bootloader, users               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ (User reboots into installed system)
┌─────────────────────────────────────────────────────────────┐
│  LAYER 3: INSTALLED SYSTEM                                 │
│  ─────────────────────────────────────────────────────────  │
│  • Minimal base system                                     │
│  • recipe package manager available                        │
│  • User installs additional packages:                      │
│      recipe install firefox                                │
│      recipe install ripgrep                                │
└─────────────────────────────────────────────────────────────┘
```

---

## Source Comparison Summary

| Component | Source | Rationale |
|-----------|--------|-----------|
| ISO build process | Arch (archiso) | Clean profile-based system |
| Package format in ISO | Rocky (RPM) | Pre-built, enterprise-grade |
| airootfs overlay | Arch (archiso) | Simple file-based customization |
| dracut + dmsquash-live | Rocky/Fedora | Live boot support |
| `recipe bootstrap` | Arch (`pacstrap`) | Familiar installation workflow |
| `recipe` S-expressions | LevitateOS original | Declarative package definitions |
| Installation guide | Arch wiki style | Proven documentation format |
| systemd-boot | Arch/systemd | Simple, modern bootloader |
| NetworkManager | Common | WiFi support in installer |

---

## TODO: Recipe Binary Integration

The `recipe` package manager binary needs to be included in the live ISO for `recipe bootstrap` to work. Options:

1. **Pre-build and include in airootfs:**
   ```bash
   # Build recipe binary
   cd ../recipe && cargo build --release

   # Copy to airootfs overlay
   cp target/release/recipe levitateiso/profile/airootfs/usr/bin/recipe
   ```

2. **Build during ISO creation:**
   Add a step in `levitateiso` to compile the `recipe` binary and include it.

3. **Ship recipes directory:**
   The live ISO also needs the `.recipe` files in `/usr/share/recipe/recipes/` for `recipe bootstrap` to find base package definitions.

Currently missing from the ISO:
- `/usr/bin/recipe` - The package manager binary
- `/usr/share/recipe/recipes/*.recipe` - Package definitions for base system
