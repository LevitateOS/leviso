# CLAUDE.md - Leviso

## ⛔ STOP. READ. THEN ACT.

Every time you think you know where something goes - **stop. Read first.**

Every time you think something is worthless and should be deleted - **stop. Read it first.**

Every time you're about to write code - **stop. Read what already exists first.**

The five minutes you spend reading will save hours of cleanup.

**E2E installation tests belong in `/home/vince/Projects/LevitateOS/install-tests/`, NOT in `leviso/tests/`.** Read `leviso/tests/README.md` before creating any test file.

---

## Context

Leviso builds the LevitateOS ISO and rootfs tarball. LevitateOS is a **daily driver Linux distribution competing with Arch Linux** - minimal base that users build up via `recipe`.

The rootfs tarball (~70 MB) is intentionally small. Users install firmware, kernels, and applications via `recipe install`. This is NOT an embedded/container OS - it's a desktop/workstation OS.

## System Requirements (MODERN HARDWARE)

**LevitateOS targets modern computers. It will ship with a local LLM. Stop treating it like a toy.**

| Resource | Minimum | QEMU Default |
|----------|---------|--------------|
| RAM | 8 GB | 4 GB |
| Storage | 64 GB SSD | 32 GB |
| CPU | x86-64-v3 (2015+) | Skylake |

**NEVER** use minimal resource values (512MB RAM, etc). This is not an embedded OS.

---

## Commands

```bash
cargo run -- test           # Quick debug (terminal, serial)
cargo run -- test -c "cmd"  # Run command after boot, exit
cargo run -- run            # Full test (QEMU GUI, UEFI only)
```

**Never pipe `cargo run -- test` to tail/head** - breaks output buffering.

## Architecture

```
leviso/
├── downloads/           # Rocky ISO, extracted rootfs (gitignored)
├── output/              # initramfs, ISO outputs (gitignored)
├── profile/init         # Init script (PID 1)
└── src/                 # Rust source
```

## Adding Binaries

Edit `src/initramfs.rs` arrays: `coreutils`, `sbin_utils`. Build copies binary + library deps from Rocky rootfs.

## Testing

### The Golden Rule

**Tests must verify what USERS experience, not what DEVELOPERS find convenient to test.**

A test that runs `systemctl status` does NOT prove users can install LevitateOS.
Only a test that ACTUALLY INSTALLS LevitateOS proves users can install it.

### Install Test (`tests/install_test.rs`)

**ONE test. ONE QEMU instance. ONE installation. ONE reboot. ONE verification.**

The test does EXACTLY what a user does:
```
1. Boot ISO in QEMU
2. Partition the virtual disk (sfdisk)
3. Format partitions (mkfs.fat, mkfs.ext4)
4. Mount partitions
5. Extract rootfs tarball
6. Install bootloader (bootctl)
7. Configure fstab
8. Reboot into installed system (NOT the ISO)
9. Verify: system boots, user can login, basic commands work
```

If this test passes, users CAN install. If it fails, they CANNOT.

```bash
cargo test --test install_test -- --ignored
```

### Quick Tests (Development Only)

```bash
cargo run -- test              # Boot to shell (interactive)
cargo run -- test -c "free -h" # Run command, exit
cargo run -- run               # Full QEMU GUI with disk
```

---

## Anti-Cheat Principles

Based on [Anthropic's Reward Hacking Research](https://www.anthropic.com/research/emergent-misalignment-reward-hacking):

### 1. Test OUTCOMES, Not PROXIES

| WRONG (Proxy) | RIGHT (Outcome) |
|---------------|-----------------|
| "lsblk runs" | "disk was partitioned correctly" |
| "systemctl works" | "system booted from installed disk" |
| "78 tests pass" | "user completed installation" |

### 2. One Test Per User Journey

A user installs ONCE. The test installs ONCE.

**NEVER** create separate tests that each boot QEMU:
- ❌ `test_boot_reaches_shell` (boots QEMU)
- ❌ `test_systemctl_works` (boots QEMU again)
- ❌ `test_disk_visible` (boots QEMU again)
- ...15 more QEMU instances

**ALWAYS** test the complete flow in ONE session:
- ✅ `test_full_installation` (boots ONCE, does everything, verifies)

### 3. Verification Must Survive Reboot

The installed system must boot WITHOUT the ISO. This proves:
- Bootloader was installed correctly
- Root filesystem is complete
- Init system works
- It's not just running from the live ISO

### 4. External Source of Truth

The test verifies against REALITY, not internal expectations:
- Did the disk actually get partitioned? (check with lsblk)
- Did files actually get extracted? (check with ls)
- Did the system actually boot? (check boot messages)

### 5. Cheats That Are Now IMPOSSIBLE

| Cheat | Why It's Blocked |
|-------|------------------|
| Run 15 parallel QEMUs | Only ONE test, uses file lock |
| Fake "installation complete" | Must actually reboot and boot from disk |
| Skip partitioning | Reboot fails without proper disk |
| Skip bootloader | System won't boot without it |
| Accept partial success | Single pass/fail for entire journey |
| Increase timeout to hide issues | Timeout is generous but fixed |

### 6. If The Test Passes

You can tell users with CONFIDENCE:
> "LevitateOS installs on bare metal. We tested the complete installation flow."

### 7. If The Test Fails

**DO NOT:**
- Split it into smaller tests that "pass individually"
- Add workarounds to make the test pass
- Mark parts as "optional"
- Increase timeouts indefinitely

**DO:**
- Fix the actual installation process
- The test reflects user experience - fix the experience

---

## Critical Rules

1. **Answer questions first** - Don't start coding when user asks "why?"
2. **Rocky 10 is non-negotiable** - Never suggest downgrading
3. **Fix tool usage, not requirements** - QEMU needs `-cpu host`, don't change distro
4. **No host dependencies** - Download everything, never use `/usr/share/...`
5. **Rocky = userspace only** - Kernel must be independent (not a Rocky rebrand)
6. **No false positives** - Missing binary = build fails, not "optional"
7. **Tests simulate users** - ONE boot, ONE install. Not 15 parallel VMs.
8. **Test outcomes, not proxies** - "lsblk works" ≠ "installation works"
