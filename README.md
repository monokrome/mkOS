# mkOS

Make your Linux, your way.

A declarative installer and configuration tool that sets up secure, reproducible Linux systems with full disk encryption and btrfs snapshots out of the box.

## Features

- **Automated Installation**: Partitions, encrypts, and installs your choice of Linux base system with a single command
- **LUKS2 Encryption**: Full disk encryption with Argon2id key derivation configured automatically
- **Btrfs Layout**: Proper subvolume structure for root, home, snapshots, and swap isolation
- **EFISTUB Boot**: Direct kernel boot from UEFI - no bootloader complexity or attack surface
- **Declarative Config**: Define your system in a manifest file for reproducible installations
- **Snapshot Support**: Automatic pre-upgrade snapshots with proper swap isolation via @swap subvolume
- **Multiple Bases**: Currently supports Artix Linux (more distributions coming soon)

## Quick Start

### Fresh Installation

Boot into a live environment and run:

```bash
curl -sL https://mkos.cc/install | sh
```

Or use a manifest file:

```bash
mkos-install manifest.yaml
```

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

- `examples/desktop.yaml` - Desktop workstation with Wayland support
- `examples/minimal.json` - Minimal server installation

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
