# mkOS Multi-Distro Support Status

## Overview

mkOS installer now supports 6 non-systemd Linux distributions. All distros have complete backend implementations with upgrade support and kernel hooks.

## Distribution Status

### ✅ Ready to Install

#### **Void Linux** - RECOMMENDED FOR TESTING
- **Status**: Fully functional, ready to test
- **Init System**: runit (native)
- **Package Manager**: xbps
- **Bootstrap**: Uses xbps-install with chroot
- **Notes**: Most straightforward to install after Artix

**Quick Start:**
```bash
# Copy example manifest
cp examples/void-minimal.yml my-void.yml

# Edit device and settings
vim my-void.yml

# Run installer
sudo mkos-install my-void.yml
```

#### **Artix Linux** - REFERENCE IMPLEMENTATION
- **Status**: Fully tested, production-ready
- **Init System**: s6 (default), runit, or OpenRC
- **Package Manager**: pacman
- **Bootstrap**: Uses pacstrap
- **Notes**: This is what mkOS was originally built for

---

### ⚠️ Needs Testing

#### **Gentoo Linux** - NOW FULLY AUTOMATED!
- **Status**: ✅ Automatic stage3 download and extraction
- **Init System**: OpenRC
- **Package Manager**: emerge (Portage)
- **Bootstrap**: **FULLY AUTOMATED** (downloads latest stage3)
- **Notes**: Prompts for variant selection (openrc, hardened, musl)

**Quick Install:**
```bash
# 1. Boot Gentoo minimal ISO (or any live environment with curl and tar)
# 2. Build installer
git clone https://github.com/monokrome/mkOS.git && cd mkOS/installer
cargo build --release

# 3. Just run it - fully automated!
sudo target/release/mkos-install

# The installer will:
# - Auto-detect architecture (amd64, arm64, etc.)
# - Prompt for stage3 variant (openrc, hardened, musl)
# - Download latest stage3 from Gentoo mirrors (~250-400MB)
# - Extract stage3 with correct permissions
# - Install kernel and configure system
# - All done!
```

#### **Alpine Linux**
- **Status**: Backend complete, untested
- **Init System**: OpenRC
- **Package Manager**: apk
- **Bootstrap**: Uses apk add with --root
- **Notes**: Lightweight, musl-based (not glibc)

#### **Slackware Linux**
- **Status**: Backend complete, untested
- **Init System**: SysVinit (BSD-style)
- **Package Manager**: slackpkg or slapt-get
- **Bootstrap**: Uses installpkg/slackpkg
- **Notes**: Conservative, stable, some packages need SlackBuilds

#### **Devuan GNU+Linux**
- **Status**: Backend complete, untested
- **Init System**: SysVinit (Debian-style)
- **Package Manager**: apt
- **Bootstrap**: Uses debootstrap (similar to Debian)
- **Notes**: systemd-free Debian derivative

---

## What Works (All Distros)

✅ **Core Features:**
- LUKS2 full-disk encryption (Argon2id)
- btrfs with subvolume layout (@, @home, @snapshots, @var, @tmp)
- EFISTUB direct boot (no GRUB)
- UKI (Unified Kernel Image) generation
- Kernel rebuild hooks (automatic UKI regeneration on kernel updates)
- `mkos upgrade` - Safe system upgrades with snapshots
- `mkos rollback` - Restore from snapshot if upgrade fails
- `mkos apply` - Apply manifest to running system

✅ **Package Management:**
- Cross-distro package mapping database
- Distro-specific package installation
- Dependency resolution via native package managers

✅ **Init Systems:**
- System services: s6, runit, OpenRC, SysVinit
- User services: s6, runit (for distros without native support)

---

## What's NOT Tested Yet

⚠️ **Needs Real Hardware Testing:**
- Void Linux fresh install (backend ready, needs validation)
- Gentoo Linux install (requires stage3 workflow testing)
- Alpine, Slackware, Devuan installs

⚠️ **Known Limitations:**
- Gentoo bootstrap doesn't auto-download stage3 (manual step required)
- Slackware kernel hooks need manual invocation (no automatic package hooks)
- Some display managers not in all distro repos (documented in package mappings)

---

## Testing Recommendations

### For Your Laptop Install:

**Option 1: Void Linux (Recommended)**
- Closest to Artix in setup complexity
- runit init system (simple, reliable)
- xbps package manager (fast, clean)
- Should "just work" with the installer

**Option 2: Gentoo Linux (Advanced)**
- More involved setup (stage3 extraction)
- Powerful but time-consuming
- Good for learning system internals
- Requires more manual intervention

### Before Installing:

1. **Backup everything** - This is still experimental for non-Artix distros
2. **Test in VM first** - Validate on virtual hardware before bare metal
3. **Have recovery media ready** - Keep a live USB handy
4. **Read the example manifests** - They document all available options

### During Install:

1. Boot a live environment (Void live ISO, Gentoo minimal ISO, etc.)
2. Clone mkOS repo and build installer:
   ```bash
   git clone https://github.com/monokrome/mkOS.git
   cd mkOS/installer
   cargo build --release
   ```
3. Copy the binary somewhere in PATH:
   ```bash
   sudo cp target/release/mkos-install /usr/local/bin/
   ```
4. Create and customize your manifest
5. Run the installer

---

## Reporting Issues

If you test Void or Gentoo and encounter issues:

1. **Expected Issues:**
   - Gentoo stage3 bootstrap needs manual intervention
   - Some packages might not be in repos (check package mappings)

2. **Report Bugs:**
   - Package installation failures
   - Init system configuration errors
   - Boot failures
   - Kernel hook problems

3. **Include in Bug Reports:**
   - Which distro you're installing
   - Your manifest file
   - Full error output
   - Output of `lsblk` and `findmnt`

---

## Next Steps

**Immediate (for your install):**
- [ ] Decide: Void or Gentoo?
- [ ] Test in VM first
- [ ] Customize manifest
- [ ] Run installer
- [ ] Report results

**Future Work:**
- [ ] Auto-download Gentoo stage3 tarballs
- [ ] Improve Slackware kernel hook automation
- [ ] Add TUI for interactive distro selection
- [ ] Create pre-built install ISOs per distro
- [ ] Document desktop environment setups per distro

---

## Interactive Mode (No Manifest)

The installer now **auto-detects** which distribution it's running on! Simply run:

```bash
sudo mkos-install
```

The installer will:
1. Detect the current distro from the live environment
2. Prompt for device selection
3. Prompt for hostname, timezone, passwords, etc.
4. Install that distribution with mkOS configuration

**Supported detection:**
- ✅ Artix (checks `/etc/artix-release`)
- ✅ Void (checks `/etc/void-release`)
- ✅ Gentoo (checks `/etc/gentoo-release`)
- ✅ Alpine (checks `/etc/alpine-release`)
- ✅ Slackware (checks `/etc/slackware-version`)
- ✅ Devuan (checks `/etc/devuan_version`)

**Fallback:** If detection fails, you'll be prompted to select from the list of 6 distros.

## Example Workflows

### Void Linux Install (Interactive - EASIEST):
```bash
# 1. Boot Void live ISO
# 2. Build installer
git clone https://github.com/monokrome/mkOS.git && cd mkOS/installer
cargo build --release
sudo cp target/release/mkos-install /usr/local/bin/

# 3. Just run it - auto-detects Void!
sudo mkos-install

# Answer prompts and you're done!
```

### Void Linux Install (With Manifest):
```bash
# 1. Boot Void live ISO
# 2. Build installer
git clone https://github.com/monokrome/mkOS.git && cd mkOS/installer
cargo build --release
sudo cp target/release/mkos-install /usr/local/bin/

# 3. Customize manifest
cp examples/void-minimal.yml my-install.yml
vim my-install.yml  # Set device, hostname, etc.

# 4. Install
sudo mkos-install my-install.yml

# 5. Reboot
sudo reboot
```

### Gentoo Linux Install (Interactive - FULLY AUTOMATED):
```bash
# 1. Boot Gentoo minimal ISO (or any live environment with curl and tar)
# 2. Build installer
git clone https://github.com/monokrome/mkOS.git && cd mkOS/installer
cargo build --release

# 3. Just run it - fully automated!
sudo target/release/mkos-install

# The installer will:
# - Auto-detect architecture (amd64, arm64, etc.)
# - Prompt for stage3 variant (openrc, hardened, musl)
# - Download latest stage3 from Gentoo mirrors (~250-400MB)
# - Extract stage3 with correct permissions
# - Install kernel and configure system
# - All done!
```

Good luck with the install! 🚀
