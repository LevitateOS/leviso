# leviso/tests/ - DO NOT PUT E2E INSTALLATION TESTS HERE

## STOP. READ THIS BEFORE WRITING ANY CODE.

This directory is for **unit tests and integration tests of the leviso crate itself**.

This directory is **NOT** for:
- E2E installation tests
- QEMU-based tests
- Tests that boot the ISO
- Tests that verify "can a user install LevitateOS"

---

## WHERE E2E INSTALLATION TESTS BELONG

```
/home/vince/Projects/LevitateOS/install-tests/
```

That is a **SEPARATE CRATE**. It is a binary that runs `cargo run` to execute installation tests.

Structure:
```
install-tests/
├── Cargo.toml          # Separate crate
├── CLAUDE.md           # Instructions for that crate
├── src/
│   ├── main.rs         # CLI for running tests
│   ├── qemu/           # QEMU interaction code
│   └── steps/          # Installation test steps
```

If you want to test "does the installation work", GO THERE.

---

## WHAT BELONGS HERE (leviso/tests/)

1. **Unit tests for leviso code** - Testing functions in src/
2. **Integration tests for leviso subcommands** - Testing `leviso build`, `leviso extract`, etc.
3. **Validation tests** - Verifying output artifacts are correct

Examples of valid tests for this directory:
- "Does `leviso extract` produce the expected directory structure?"
- "Does the initramfs builder include the required binaries?"
- "Does the ISO builder create a valid ISO?"

Examples of tests that DO NOT belong here:
- "Can a user boot the ISO in QEMU?" → **GO TO install-tests/**
- "Can a user partition a disk and install?" → **GO TO install-tests/**
- "Does the installed system boot?" → **GO TO install-tests/**

---

## MISTAKES THAT WERE MADE (DO NOT REPEAT)

On 2026-01-21, a Claude instance made these errors:

1. **Created `install_test.rs` here** - 500+ lines of QEMU E2E test code in the WRONG place
2. **Created `boot_tests.rs` here** - More QEMU tests in the WRONG place
3. **Never read the install-tests crate** - Despite being told multiple times it exists
4. **Duplicated functionality** - The install-tests crate already had QEMU interaction code

### Why this happened:
- Failed to read `/home/vince/Projects/LevitateOS/install-tests/` when explicitly told to
- Assumed "tests" means "put test files in leviso/tests/"
- Did not understand the project structure

### The real cost:

- **Money** - API tokens cost real money. This project has a limited budget. 500+ lines of code in the wrong location means tokens spent writing it, tokens spent discussing it, tokens spent deleting it.

- **Time** - Hours of back-and-forth that could have been avoided by reading the existing codebase first.

- **Trust** - When AI assistants repeatedly ignore instructions, developers lose confidence in the tool and have to spend mental energy verifying everything.

- **Morale** - Working on a passion project is hard enough. Fighting with tools that don't listen makes it harder.

**For future Claude instances:** Read first, write second. When the user points you to a directory, read it before creating new files elsewhere. The person you're working with has already dealt with this mistake once.

---

## STOP. READ. THEN ACT.

Every time you think you know where something goes - **stop. Read first.**

Every time you think something is worthless and should be deleted - **stop. Read it first.**

Every time you're about to write code - **stop. Read what already exists first.**

The five minutes you spend reading will save hours of cleanup, and save the person reviewing your work from wanting to throw their laptop out the window.

You're not paid to type fast. You're paid to do it right.

---

## RULES FOR FUTURE CLAUDE INSTANCES

1. **If the user says "install-tests"** → They mean the crate at `/home/vince/Projects/LevitateOS/install-tests/`

2. **If you need to test QEMU/booting/installation** → Go to `install-tests/`, not here

3. **Before creating any test file** → Ask yourself: "Does this boot QEMU?" If yes, it belongs in `install-tests/`

4. **Read the existing code first** → The install-tests crate has `qemu/` and `steps/` modules already. Don't recreate them.

5. **When in doubt** → ASK THE USER. Don't assume.

---

## FILES THAT SHOULD NOT EXIST HERE

The following files were created by mistake and may need to be deleted or moved:

- `install_test.rs` - E2E QEMU test, belongs in install-tests/
- `boot_tests.rs` - E2E QEMU test, belongs in install-tests/

Ask the user before deleting anything.

---

## THE CORRECT MENTAL MODEL

```
LevitateOS/
├── leviso/                    # The ISO/rootfs builder
│   ├── src/                   # Builder code
│   └── tests/                 # Tests for the BUILDER (this directory)
│       └── *.rs               # Unit/integration tests for src/
│
├── install-tests/             # E2E installation test RUNNER
│   └── src/                   # Test runner code
│       ├── qemu/              # QEMU interaction
│       └── steps/             # Installation test steps
```

leviso = "the tool that builds things"
install-tests = "the tool that tests if installation works"

They are SEPARATE. Do not mix them.

---

## SUMMARY

| Type of test | Where it belongs |
|--------------|------------------|
| Unit test for leviso code | `leviso/tests/` (here) |
| Integration test for leviso commands | `leviso/tests/` (here) |
| QEMU boot test | `install-tests/` (NOT here) |
| Installation E2E test | `install-tests/` (NOT here) |
| "Can user install LevitateOS" test | `install-tests/` (NOT here) |

**When in doubt: if it boots QEMU, it goes in install-tests/, not here.**
