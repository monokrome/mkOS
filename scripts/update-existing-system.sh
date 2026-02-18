#!/bin/bash
# mkOS System Update Script
# Safely updates an existing mkOS installation with new features

set -e

echo "mkOS System Update Script"
echo ""
echo "This script will:"
echo "  1. Build and install mkos tools to /usr/local/bin"
echo "  2. Update dracut configuration"
echo "  3. Install UKI rebuild script and pacman hook"
echo "  4. Migrate swap to @swap subvolume (if needed)"
echo "  5. Optionally rebuild your current UKI"
echo ""

# Get the actual user (even when running with sudo)
ACTUAL_USER="${SUDO_USER:-$USER}"

# Detect script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Check if we're in the right directory
if [ ! -f "$PROJECT_ROOT/installer/Cargo.toml" ]; then
    echo "ERROR: Cannot find installer/Cargo.toml. Are you running this from the mkOS repository?"
    exit 1
fi

# Check if this looks like an mkOS system
if [ ! -f /etc/crypttab ]; then
    echo "WARNING: This doesn't look like a standard mkOS installation (no /etc/crypttab)"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo ""
echo "[1/5] Building and installing mkOS binaries..."

# Build release binaries as the actual user (not root)
echo "  Building release binaries (this may take a minute)..."
cd "$PROJECT_ROOT/installer"

if [ "$EUID" -eq 0 ] && [ -n "$ACTUAL_USER" ]; then
    sudo -u "$ACTUAL_USER" cargo build --release --quiet
else
    cargo build --release --quiet
fi

# Check we have root for installation
if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Root privileges required for installation. Please run with sudo."
    exit 1
fi

echo "  Installing mkos to /usr/local/bin..."
install -m 755 target/release/mkos /usr/local/bin/

echo "  Installing mkos-apply to /usr/local/bin..."
install -m 755 target/release/mkos-apply /usr/local/bin/

echo "  Installing mkos-rescue to /usr/local/bin..."
install -m 755 target/release/mkos-rescue /usr/local/bin/

echo ""
echo "[2/5] Updating dracut configuration..."

# Write the current dracut config (replaces any stale config)
mkdir -p /etc/dracut.conf.d
cat > /etc/dracut.conf.d/mkos.conf <<'DRACUT_EOF'
# mkOS dracut configuration

# Omit all systemd dracut modules - mkOS targets non-systemd distributions
omit_dracutmodules+=" systemd systemd-initrd systemd-udevd dracut-systemd "
omit_dracutmodules+=" systemd-ac-power systemd-ask-password systemd-battery-check "
omit_dracutmodules+=" systemd-bsod systemd-coredump systemd-creds systemd-cryptsetup "
omit_dracutmodules+=" systemd-hostnamed systemd-integritysetup systemd-journald "
omit_dracutmodules+=" systemd-ldconfig systemd-modules-load systemd-network-management "
omit_dracutmodules+=" systemd-networkd systemd-pcrphase systemd-portabled systemd-pstore "
omit_dracutmodules+=" systemd-repart systemd-resolved systemd-sysctl systemd-sysext "
omit_dracutmodules+=" systemd-timedated systemd-timesyncd systemd-tmpfiles "
omit_dracutmodules+=" systemd-veritysetup systemd-emergency systemd-sysusers "

# Force modules that return 255 when not on running system
force_add_dracutmodules+=" dm crypt btrfs "

# Additional required modules
add_dracutmodules+=" rootfs-block "

# CPU microcode
early_microcode=yes

# Critical drivers for LUKS support
add_drivers+=" dm_mod dm_crypt "

# Drivers for VMs and common hardware
add_drivers+=" virtio virtio_blk virtio_pci virtio_scsi nvme ahci sd_mod "

# Filesystems
filesystems+=" btrfs ext4 vfat "

# Compression
compress="zstd"

# Include crypttab for LUKS device discovery
install_items+=" /etc/crypttab "
DRACUT_EOF

echo "  Updated /etc/dracut.conf.d/mkos.conf"

echo ""
echo "[3/5] Installing UKI rebuild script and pacman hook..."

# Install the rebuild script from the repo template
REBUILD_TEMPLATE="$PROJECT_ROOT/installer/src/templates/hooks/mkos-rebuild-uki.sh"
if [ ! -f "$REBUILD_TEMPLATE" ]; then
    echo "ERROR: Cannot find rebuild template at $REBUILD_TEMPLATE"
    exit 1
fi

install -m 755 "$REBUILD_TEMPLATE" /usr/local/bin/mkos-rebuild-uki
echo "  Installed /usr/local/bin/mkos-rebuild-uki"

# Install the pacman hook
mkdir -p /etc/pacman.d/hooks
cat > /etc/pacman.d/hooks/90-mkos-uki.hook <<'EOF'
[Trigger]
Operation = Install
Operation = Upgrade
Type = Package
Target = linux
Target = linux-lts
Target = linux-hardened
Target = linux-zen

[Action]
Description = Rebuilding Unified Kernel Image...
When = PostTransaction
Exec = /usr/local/bin/mkos-rebuild-uki
EOF

echo "  Installed /etc/pacman.d/hooks/90-mkos-uki.hook"

# Migrate old UKI path if needed (/boot/EFI/Linux -> /boot)
if [ -d "/boot/EFI/Linux" ]; then
    OLD_UKIS=$(ls /boot/EFI/Linux/mkos-*.efi 2>/dev/null || true)
    if [ -n "$OLD_UKIS" ]; then
        echo ""
        echo "  Found old UKIs in /boot/EFI/Linux/ - these will be replaced"
        echo "  when you rebuild below."
    fi
fi

echo ""
echo "[4/5] Checking swap configuration..."

# Check if we need to migrate swap to @swap subvolume
if [ -f "/swapfile" ] && [ ! -f "/swap/swapfile" ]; then
    echo ""
    echo "  Found /swapfile in root subvolume. For optimal snapshot support,"
    echo "  mkOS now uses a dedicated @swap subvolume."
    echo ""
    read -p "  Migrate swap to @swap subvolume? [Y/n] " -n 1 -r
    echo

    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        ROOT_DEVICE=$(findmnt -n -o SOURCE / | sed 's/\[.*$//')

        if swapon --show --noheadings | grep -q "/swapfile"; then
            echo "  Disabling swap..."
            swapoff /swapfile
            SWAP_WAS_ACTIVE=1
        else
            SWAP_WAS_ACTIVE=0
        fi

        echo "  Mounting btrfs root..."
        TEMP_MOUNT="/tmp/mkos-btrfs-root"
        mkdir -p "$TEMP_MOUNT"
        mount -o subvolid=5 "$ROOT_DEVICE" "$TEMP_MOUNT"

        if [ ! -d "$TEMP_MOUNT/@swap" ]; then
            echo "  Creating @swap subvolume..."
            btrfs subvolume create "$TEMP_MOUNT/@swap"
        fi

        mkdir -p /swap
        mount -o subvol=@swap "$ROOT_DEVICE" /swap

        SWAP_SIZE_MB=$(stat -c %s "/swapfile")
        SWAP_SIZE_MB=$((SWAP_SIZE_MB / 1024 / 1024))

        echo "  Removing old swapfile..."
        rm -f /swapfile

        echo "  Creating new swapfile (${SWAP_SIZE_MB}MB)..."
        touch /swap/swapfile
        chattr +C /swap/swapfile
        dd if=/dev/zero of=/swap/swapfile bs=1M count="$SWAP_SIZE_MB" status=progress 2>&1 | tail -n 1

        chmod 600 /swap/swapfile
        mkswap /swap/swapfile > /dev/null

        echo "  Updating /etc/fstab..."
        if ! grep -q "@swap" /etc/fstab; then
            echo "$ROOT_DEVICE /swap btrfs subvol=@swap,defaults 0 0" >> /etc/fstab
        fi
        sed -i 's|/swapfile|/swap/swapfile|g' /etc/fstab

        umount "$TEMP_MOUNT"
        rmdir "$TEMP_MOUNT"

        if [ "$SWAP_WAS_ACTIVE" -eq 1 ]; then
            echo "  Re-enabling swap..."
            swapon /swap/swapfile
        fi

        echo "  Swap migration complete"
    fi
elif [ -f "/swap/swapfile" ]; then
    echo "  Already using @swap subvolume"
else
    echo "  No swapfile detected, skipping"
fi

echo ""
echo "[5/5] Rebuild UKI?"
echo ""
echo "  Recommended to apply the updated dracut config and rebuild all"
echo "  boot images (main, fallback, rescue) without systemd modules."
echo ""
read -p "  Rebuild UKI now? [Y/n] " -n 1 -r
echo

if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    echo ""
    /usr/local/bin/mkos-rebuild-uki
fi

echo ""
echo "Update complete."
echo ""
echo "Installed binaries:"
echo "  mkos             - System management (update, upgrade, apply, snapshot)"
echo "  mkos-apply       - Apply manifests (legacy, use 'mkos apply')"
echo "  mkos-rescue      - Chroot into installed system from live environment"
echo "  mkos-rebuild-uki - Rebuild UKI manually"
echo ""
echo "Installed hooks:"
echo "  Automatic UKI rebuild on kernel upgrade (/etc/pacman.d/hooks/90-mkos-uki.hook)"
