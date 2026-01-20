# LevitateOS Base Recipes

These recipes define the base system packages for LevitateOS.

## Usage

During installation from the live ISO:

```bash
# Set RPM source to mounted Rocky packages
export RPM_PATH=/run/media/rocky/BaseOS/Packages

# Install base system
recipe install base --deps
```

## Architecture

- `base.rhai` - Meta-recipe that depends on all base packages
- Individual recipes use `rpm_install()` to extract from Rocky RPMs
- Each recipe tracks installed files for clean removal

## Recipe Categories

- **Core**: glibc, bash, coreutils, filesystem
- **System**: systemd, util-linux, shadow-utils
- **Network**: iproute, wget, curl
- **Text**: grep, sed, gawk, findutils
- **Compression**: tar, gzip, xz, bzip2
- **Hardware**: kmod, pciutils
