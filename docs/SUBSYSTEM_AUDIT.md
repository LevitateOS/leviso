# Leviso Subsystem Audit

**Date:** 2026-01-23
**Purpose:** Identify scattered patterns that should be centralized for code quality

---

## Summary

| Subsystem | Occurrences | Files | Priority | Status |
|-----------|-------------|-------|----------|--------|
| Download/Verification | - | 4 | HIGH | DONE ✓ |
| Command Execution | 39 calls | 15 | HIGH | DONE ✓ |
| Console Output | 268 println! | 29 | HIGH | TODO |
| File Operations | 79+ calls | 23 | MEDIUM | TODO |
| Path Construction | 355 .join() | 29 | MEDIUM | TODO |
| Tokio Runtime | 3 | 3 | LOW | TODO |
| Environment Variables | 41 | 10 | LOW | Partial |

---

## 1. Command Execution (Priority: HIGH)

**Problem:** 39 `Command::new()` calls across 15 files with inconsistent error handling.

### Files Affected
- `src/iso.rs` - 9 calls
- `src/build/kernel.rs` - 5 calls
- `src/extract.rs` - 4 calls
- `src/preflight.rs` - 4 calls
- `src/deps/download.rs` - 3 calls
- `src/qemu.rs` - 2 calls
- `src/initramfs/mod.rs` - 2 calls
- `src/squashfs/system.rs` - 2 calls
- Plus 7 more files with 1 call each

### Inconsistencies

**Pattern A: No stderr capture (bad)**
```rust
// iso.rs
let status = Command::new("xorriso").args([...]).status()?;
if !status.success() {
    bail!("xorriso failed");  // No idea WHY it failed
}
```

**Pattern B: Captures stderr (good)**
```rust
// kernel.rs
let output = Command::new("make").args([...]).output().context("...")?;
if !output.status.success() {
    bail!("make failed:\n{}", String::from_utf8_lossy(&output.stderr));
}
```

**Pattern C: Mixed/inconsistent**
```rust
// extract.rs
let chmod_status = Command::new("chmod")
    .args([...])
    .status()
    .context("Failed to chmod")?;
```

### Migration Progress

| File | Commands | Status |
|------|----------|--------|
| `src/squashfs/pack.rs` | 1 | DONE ✓ |
| `src/squashfs/mod.rs` | 1 | DONE ✓ |
| `src/iso.rs` | 9 | DONE ✓ |
| `src/build/kernel.rs` | 5 | DONE ✓ |
| `src/extract.rs` | 4 | DONE ✓ |
| `src/preflight.rs` | 4 | DONE ✓ |
| `src/common/binary.rs` | 1 | DONE ✓ |
| `src/main.rs` | 2 | DONE ✓ |
| `src/deps/download.rs` | 3 | SKIP (async, uses tokio Command) |
| `src/qemu.rs` | 1 | DONE ✓ |
| `src/initramfs/mod.rs` | 2 | DONE ✓ |
| `src/squashfs/system.rs` | 2 | DONE ✓ |
| `src/build/modules.rs` | 1 | DONE ✓ |
| `src/build/libdeps.rs` | 1 | DONE ✓ |
| `src/deps/tools.rs` | 1 | DONE ✓ |

**All files migrated.** The `src/process.rs` module is now used consistently across the codebase.

### Implemented Solution

Created `src/process.rs` module with builder pattern:

```rust
pub struct Cmd {
    program: String,
    args: Vec<String>,
    current_dir: Option<PathBuf>,
    allow_fail: bool,
    error_prefix: Option<String>,
}

impl Cmd {
    pub fn new(program) -> Self;
    pub fn arg(self, arg) -> Self;
    pub fn args(self, args) -> Self;
    pub fn arg_path(self, path: &Path) -> Self;
    pub fn dir(self, dir: &Path) -> Self;
    pub fn allow_fail(self) -> Self;
    pub fn error_msg(self, msg) -> Self;

    /// Run command, capture output, fail with stderr on error
    pub fn run(self) -> Result<CommandResult>;

    /// Run command, stream output to console (for long builds)
    pub fn run_interactive(self) -> Result<ExitStatus>;
}

/// Convenience functions
pub fn run(program, args) -> Result<CommandResult>;
pub fn run_in(program, args, dir) -> Result<CommandResult>;
pub fn shell(command: &str) -> Result<CommandResult>;
pub fn shell_in(command: &str, dir: &Path) -> Result<CommandResult>;
pub fn which(program: &str) -> Option<String>;
pub fn exists(program: &str) -> bool;
```

The module includes 14 unit tests covering error handling, command building, and shell execution.

---

## 2. Console Output (Priority: HIGH)

**Problem:** 268 `println!` calls across 29 files with 6+ different formatting styles.

### Formatting Styles Found

```rust
// Style 1: Simple action
println!("Copying systemd units...");

// Style 2: Indented detail
println!("  Copied {}/{} unit files", copied, total);

// Style 3: Section header
println!("=== Full LevitateOS Build ===");

// Style 4: Status tag
println!("[SKIP] Kernel already built");

// Style 5: Progress (carriage return)
print!("\r    {}", progress.display());

// Style 6: Path display
println!("Found bash at: {}", bash_path.display());
```

### Problems
- No log levels (info, warn, error, debug)
- No filtering capability (verbose vs quiet mode)
- Direct stdout coupling (hard to test)
- Inconsistent formatting

### Proposed Solution

Create `src/ui.rs` module:

```rust
pub enum Level { Debug, Info, Warn, Error }

pub fn section(title: &str);           // "=== Title ==="
pub fn info(msg: &str);                // "  message"
pub fn detail(msg: &str);              // "    detail"
pub fn success(msg: &str);             // "  [OK] message"
pub fn skip(msg: &str);                // "  [SKIP] message"
pub fn warn(msg: &str);                // "  [WARN] message"
pub fn error(msg: &str);               // "  [ERROR] message"
pub fn progress(current: u64, total: u64, msg: &str);
```

---

## 3. File Operations (Priority: MEDIUM)

**Problem:** 79+ `fs::create_dir_all` calls across 23 files with inconsistent error context.

### Files Most Affected
- `src/build/systemd.rs` - 11 occurrences
- `src/build/etc.rs` - 6 occurrences
- `src/build/kernel.rs` - 4 occurrences
- `src/deps/download.rs` - 5 occurrences

### Inconsistencies

```rust
// Pattern 1: No context (bad)
fs::create_dir_all(path)?;

// Pattern 2: With context (good)
fs::create_dir_all(path)
    .with_context(|| format!("Failed to create {}", path.display()))?;

// Pattern 3: Check first
if !path.exists() {
    fs::create_dir_all(path)?;
}
```

### Other Scattered Operations
- `fs::remove_dir_all` - 14 occurrences
- `fs::remove_file` - scattered
- `fs::write` - scattered
- `fs::copy` - scattered
- `fs::set_permissions` - 36 occurrences

### Proposed Solution

Create `src/fs_util.rs` module:

```rust
/// Create directory with proper error context
pub fn create_dir(path: &Path) -> Result<()>;

/// Remove directory tree with verification
pub fn remove_tree(path: &Path) -> Result<()>;

/// Copy file with progress for large files
pub fn copy_file(src: &Path, dest: &Path) -> Result<()>;

/// Set executable permissions (0o755)
pub fn make_executable(path: &Path) -> Result<()>;

/// Create symlink with proper error handling
pub fn symlink(src: &Path, dest: &Path) -> Result<()>;
```

**Note:** `common/binary.rs` already has some of these (`make_executable`, `create_symlink_if_missing`). Consider consolidating.

---

## 4. Path Construction (Priority: MEDIUM)

**Problem:** 355 `.join()` calls across 29 files. Some files have path structs, most don't.

### Good Pattern (iso.rs)
```rust
struct IsoPaths {
    iso_contents: PathBuf,
    output_dir: PathBuf,
    efi_boot: PathBuf,
    // ...
}
```

### Bad Pattern (main.rs, scattered)
```rust
let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");
let squashfs_path = base_dir.join("output/filesystem.squashfs");
let initramfs_path = base_dir.join("output/initramfs-tiny.cpio.gz");
let iso_path = base_dir.join("output/levitateos.iso");
```

### Proposed Solution

Extend the `IsoPaths` pattern to a unified `ProjectPaths`:

```rust
pub struct ProjectPaths {
    pub base: PathBuf,
    pub output: OutputPaths,
    pub downloads: PathBuf,
    pub cache: PathBuf,
}

pub struct OutputPaths {
    pub staging: PathBuf,
    pub vmlinuz: PathBuf,
    pub initramfs: PathBuf,
    pub squashfs: PathBuf,
    pub iso: PathBuf,
}

impl ProjectPaths {
    pub fn new(base_dir: &Path) -> Self { ... }
}
```

---

## 5. Tokio Runtime (Priority: LOW)

**Problem:** 3 files each create their own tokio runtime.

### Files
- `src/deps/rocky.rs`
- `src/deps/tools.rs`
- `src/deps/linux.rs`

### Current Pattern
```rust
let rt = tokio::runtime::Runtime::new()?;
rt.block_on(async { ... })
```

### Proposed Solution

Two options:

**Option A:** Single runtime in main, pass down
```rust
// main.rs
#[tokio::main]
async fn main() { ... }
```

**Option B:** Shared runtime helper
```rust
// src/runtime.rs
pub fn block_on<F: Future>(f: F) -> F::Output {
    static RT: OnceLock<Runtime> = OnceLock::new();
    let rt = RT.get_or_init(|| Runtime::new().unwrap());
    rt.block_on(f)
}
```

---

## 6. Environment Variables (Priority: LOW)

**Problem:** 41 `env::var` calls across 10 files. Some centralized, some scattered.

### Good Pattern (deps/rocky.rs)
```rust
impl RockyConfig {
    pub fn from_env() -> Self {
        Self {
            version: env::var("ROCKY_VERSION")
                .unwrap_or_else(|_| defaults::VERSION.to_string()),
            // ...
        }
    }
}
```

### Bad Pattern (scattered)
```rust
// build/etc.rs
let url = env::var("BUSYBOX_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

// iso.rs
let label = env::var("ISO_LABEL").unwrap_or_else(|_| "LEVITATEOS".to_string());
```

### Proposed Solution

Extend `src/config.rs` to centralize all env vars:

```rust
pub struct Config {
    pub rocky: RockyConfig,
    pub linux: LinuxConfig,
    pub iso: IsoConfig,
    pub build: BuildConfig,
}

impl Config {
    pub fn from_env() -> Self { ... }
}
```

---

## Already Well-Done

These subsystems are already properly centralized:

1. **Download module** (`src/deps/download.rs`)
   - HTTP with retry, resume, progress
   - BitTorrent support
   - Git clone
   - Tarball extraction
   - SHA256 verification

2. **Binary/library handling** (`src/common/binary.rs`)
   - ELF parsing for dependencies
   - Library copying with symlinks
   - Executable permissions

---

## Implementation Order

1. **Command Execution** - Highest impact, affects error messages users see
2. **Console Output** - Enables verbose/quiet modes, testability
3. **File Operations** - Safety and consistency
4. **Path Construction** - Maintainability
5. **Tokio Runtime** - Minor cleanup
6. **Environment Variables** - Minor cleanup

---

## Notes

- Each refactoring should be done in a separate PR/commit
- Add tests for new modules
- Update existing code incrementally (don't rewrite everything at once)
- The `common/` directory already has good patterns to follow
