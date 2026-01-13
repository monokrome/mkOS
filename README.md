# mkOS

Make your Linux, your way.

A declarative installer and configuration tool that sets up secure, reproducible Linux systems with full disk encryption and btrfs snapshots out of the box.

## Features

- **Automated Installation**: Partitions, encrypts, and installs your choice of Linux distribution with a single command
- **Multi-Distribution Support**: Supports 6 non-systemd Linux distributions:
  - **Artix Linux** (s6/runit/OpenRC) - Production ready
  - **Void Linux** (runit) - Fully functional
  - **Gentoo Linux** (OpenRC) - Automatic stage3 download
  - **Alpine Linux** (OpenRC) - Backend complete
  - **Slackware Linux** (SysVinit) - Backend complete
  - **Devuan GNU+Linux** (SysVinit) - Backend complete
- **Auto-Detection**: Automatically detects which distribution you're running from the live environment
- **Interactive Mode**: No manifest file required - just run the installer and answer prompts
- **LUKS2 Encryption**: Full disk encryption with Argon2id key derivation configured automatically
- **Btrfs Layout**: Proper subvolume structure for root, home, snapshots, and swap isolation
- **EFISTUB Boot**: Direct kernel boot from UEFI - no bootloader complexity or attack surface
- **Declarative Config**: Define your system in a manifest file for reproducible installations
- **Snapshot Support**: Automatic pre-upgrade snapshots with proper swap isolation via @swap subvolume

## Quick Start

### Fresh Installation

Boot into any supported live environment (Artix, Void, Gentoo, Alpine, Slackware, or Devuan) and run:

```bash
curl -sL https://mkos.cc/install | sh
```

The installer will:
- Auto-detect which distribution you're installing
- Prompt for device selection and configuration interactively
- Automatically handle distribution-specific setup (e.g., Gentoo stage3 download)

Alternatively, use a manifest file for declarative, reproducible installations:

```bash
mkos-install manifest.yaml
```

See `examples/` for sample manifests for each supported distribution.

### Managing Existing Systems

mkOS provides a unified command-line tool for system management:

```bash
# Update package indexes
mkos update

# Upgrade packages with automatic snapshot
mkos upgrade

# Apply configuration manifest
mkos apply manifest.yaml

# Manage snapshots
mkos snapshot list
mkos snapshot delete <name>
```

### Updating mkOS Tools

For systems already installed with mkOS, update to the latest tools:

```bash
cd /path/to/mkos
git pull
sudo ./scripts/update-existing-system.sh
```

This will:
- Install the latest mkOS binaries
- Set up automatic UKI rebuild on kernel upgrades
- Migrate swap to @swap subvolume (if needed)

## What You Get

```
+------------------+
|   EFI (512MB)    |  FAT32 - Kernel + Initramfs
+------------------+
|                  |
|   LUKS2 Volume   |  AES-XTS-512 + Argon2id
|                  |
|  +------------+  |
|  |   btrfs    |  |
|  |            |  |
|  | @          |  |  Root subvolume
|  | @home      |  |  User data
|  | @snapshots |  |  Automatic backups
|  | @swap      |  |  Swap isolation
|  +------------+  |
+------------------+
```

A properly partitioned, encrypted system with snapshot support, configured to boot directly via EFISTUB.

## Manifest Examples

See the `examples/` directory for sample manifests:

- `examples/desktop.yaml` - Desktop workstation with Wayland support (Artix)
- `examples/minimal.json` - Minimal server installation (Artix)
- `examples/void-minimal.yml` - Void Linux minimal installation
- `examples/gentoo-minimal.yml` - Gentoo Linux with automatic stage3 download

Each manifest shows distribution-specific configuration options and available features.

For detailed information about distribution support status, see [`installer/DISTRO_STATUS.md`](installer/DISTRO_STATUS.md).

## Commands Reference

### Installation

- `mkos-install [manifest.yaml]` - Fresh system installation from manifest

### System Management

- `mkos update` - Update package indexes only
- `mkos upgrade` - Update indexes and upgrade packages (creates snapshot first)
- `mkos apply <manifest>` - Apply configuration manifest to system (creates snapshot first)

### Snapshots

- `mkos snapshot list` - List all available snapshots
- `mkos snapshot delete <name>` - Delete a specific snapshot

### Utilities

- `mkos-rebuild-uki` - Manually rebuild the Unified Kernel Image
- `mkos-apply <manifest>` - Legacy command (use `mkos apply` instead)

## Architecture

### Swap Isolation

Recent versions of mkOS use a dedicated `@swap` subvolume to isolate swap space. This allows btrfs snapshots to work without needing to disable swap. The migration to this architecture happens automatically when you run the update script.

### Automatic UKI Rebuild

mkOS installs package manager hooks that automatically rebuild the Unified Kernel Image when the kernel is upgraded. This prevents boot failures after kernel updates.

## Use Cases

- **Personal Workstations**: Set up your development machine with encryption and snapshots in minutes
- **Server Deployments**: Reproducible server installations from manifest files
- **Custom Distributions**: Use mkOS as the foundation for building your own Linux distribution

## License

MIT License - see [LICENSE](LICENSE) for details

## Links

- [GitHub Repository](https://github.com/monokrome/mkOS)
- [Documentation](https://mkos.cc)
