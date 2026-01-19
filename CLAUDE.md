# CLAUDE MISTAKES - DO NOT REPEAT

## Things that annoyed the user during leviso development

### 1. Implementing instead of answering questions
When the user asked "why do we need grub-mkrescue? is it required?" I immediately started implementing an alternative (xorriso) instead of ANSWERING THE DAMN QUESTION FIRST.

**Rule: When the user asks WHY or asks a question, ANSWER IT. Don't start coding.**

### 2. Swinging between rebrands
User expressed concern about making a Rocky rebrand. I immediately swung to "let's copy archiso's approach" - which would make it an Arch rebrand. The user wants NEITHER.

**Rule: Understand what the user actually wants before proposing solutions.**

### 3. Using host system dependencies
I was about to use `/usr/share/syslinux/` from the user's Fedora system to build the ISO. This violates the fundamental principle that a build system should be SELF-CONTAINED and not depend on what's installed on the host.

**Rule: Leviso must download ALL its dependencies. Never assume host has anything beyond basic tools (cpio, gzip, etc).**

### 4. Changing user requirements instead of investigating
Used Rocky 10.0 URL, got a 404. Instead of investigating (maybe it's 10.1?), I immediately suggested "let's use Rocky 9 instead" - changing what the user asked for. User had to tell me to look harder, and Rocky 10.1 was right there.

**Rule: When something doesn't work, INVESTIGATE. Don't immediately change what the user asked for.**

### 5. Poor context retention
User had to repeat themselves multiple times. I kept forgetting what we discussed and making the same conceptual mistakes.

**Rule: Pay attention to the conversation. Don't make the user repeat themselves.**

### 6. Requiring user to say STOP multiple times
User had to interrupt me with "STOP" because I was running ahead implementing things without discussion.

**Rule: Discuss first, implement second. Especially for architectural decisions.**

### 7. Suggesting alternatives to what user explicitly asked for
User asked for an ISO. TWICE I suggested "let's just test with QEMU direct kernel boot, no ISO needed." That's not what they asked for. Same pattern as Rocky 10 → Rocky 9.

**Rule: If the user asks for X, deliver X. Don't suggest "how about Y instead" unless there's a genuine blocker.**

### 8. Suggesting to change distro because QEMU emulation was wrong
Rocky 10 requires x86-64-v3. QEMU's default CPU doesn't support it. The FIX is to run QEMU with `-cpu Skylake-Client` or `-cpu host`. Instead of fixing the QEMU command, I suggested "use Rocky 9 kernel" - AGAIN changing what the user asked for instead of fixing the actual problem.

**Rule: When a tool doesn't work, FIX THE TOOL USAGE. Don't change the user's requirements.**

### 9. ROCKY 10 IS NON-NEGOTIABLE
If I EVER suggest downgrading to Rocky 9 or any other version because of an issue, I am fundamentally failing. Rocky 10 is the requirement. Period. Find another way to fix the problem.

**Rule: NEVER suggest changing Rocky 10. Fix the actual problem.**

### 10. Using Rocky's kernel makes it a Rocky rebrand
We used `vmlinuz` directly from the Rocky ISO. That makes leviso a Rocky rebrand, NOT LevitateOS. Rocky should ONLY be a source for userspace binaries (bash, coreutils, libs). The KERNEL must be our own - either vanilla from kernel.org or built from source.

**Rule: Rocky = source for userspace binaries ONLY. Kernel must be independent.**

---

## Testing Commands

**Claude uses `test`, User uses `run`.**

```bash
# Claude: Quick debug in terminal (direct kernel boot, serial console)
cargo run -- test

# Claude: Run a command after boot and exit
cargo run -- test -c "timedatectl"

# User: Real test in QEMU GUI (full ISO, closest to bare metal)
cargo run -- run

# User: Same but force BIOS instead of UEFI
cargo run -- run --bios
```

- `test` = fast iteration, no ISO rebuild needed, output in terminal
- `test -c "cmd"` = run command after boot, then exit immediately
- `run` = real experience, requires `cargo run -- iso` first, opens GUI window

### 11. DO NOT PIPE `cargo run -- test` TO tail OR head

**This will break output buffering and cause the test to hang forever.**

```bash
# WRONG - will hang:
cargo run -- test -c "echo hello" | tail -20
cargo run -- test 2>&1 | head -50

# RIGHT - run directly:
cargo run -- test -c "echo hello"
```

The QEMU serial console output is line-buffered and piping breaks the readline loop. Just run the command directly.

---

## Architecture

### What we're building

We build an **initramfs** - a compressed cpio archive that becomes the live environment. The ISO is just a wrapper containing:
- Kernel (vmlinuz)
- Initramfs (initramfs.cpio.gz)
- Bootloader (isolinux for BIOS, GRUB EFI for UEFI)

### Directory structure

```
leviso/
├── downloads/           # Downloaded dependencies (gitignored)
│   ├── rocky.iso        # Rocky 10 Minimal ISO
│   ├── iso-contents/    # Extracted ISO (kernel, EFI files)
│   ├── rootfs/          # Extracted squashfs (userspace binaries)
│   └── syslinux-6.03/   # Syslinux for BIOS boot
├── output/              # Build outputs (gitignored)
│   ├── initramfs-root/  # Unpacked initramfs contents
│   ├── initramfs.cpio.gz
│   ├── iso-root/        # ISO contents before packaging
│   └── leviso.iso       # Final bootable ISO
├── profile/
│   └── init             # Init script (runs as PID 1)
└── src/                 # Rust source
```

### Adding binaries to the initramfs

Edit `src/initramfs.rs`:
- `coreutils` array: binaries from /usr/bin or /bin
- `sbin_utils` array: binaries from /usr/sbin or /sbin

The build automatically copies each binary and its library dependencies.

---

## Where Binaries Come From

**ALL userspace binaries come from the Rocky 10 rootfs.**

The flow:
1. `leviso download` - Downloads Rocky 10 Minimal ISO
2. `leviso extract` - Extracts squashfs to `downloads/rootfs/`
3. `leviso initramfs` - Copies binaries FROM `downloads/rootfs/` INTO the initramfs

When you add a binary (like `gzip`), the code:
1. Looks in `downloads/rootfs/usr/bin/`, `downloads/rootfs/bin/`, etc.
2. Copies the binary to the initramfs
3. Runs `ldd` to find shared library dependencies
4. Copies those libraries from the rootfs too

**DO NOT:**
- Copy binaries from the host system (your Fedora/Arch/whatever)
- Download binaries from random URLs
- Build binaries from source (yet - Phase 10 goal)

**DO:**
- Add the binary name to `coreutils` or `sbin_utils` array
- Let the build system find it in Rocky rootfs
- If it's not in Rocky, that's a problem to solve differently

This keeps the build reproducible and self-contained.
