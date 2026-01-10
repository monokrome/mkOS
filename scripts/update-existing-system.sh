#!/bin/bash
# mkOS System Update Script
# Safely updates an existing mkOS installation with new features

set -e

echo "mkOS System Update Script"
echo "========================="
echo ""
echo "This script will:"
echo "  1. Build and install mkos tools to /usr/local/bin"
echo "  2. Install pacman hook for automatic UKI rebuild on kernel upgrades"
echo "  3. Install UKI rebuild script at /usr/local/bin/mkos-rebuild-uki"
echo "  4. Migrate swapfile to @swap subvolume (if needed)"
echo "  5. Optionally rebuild your current UKI"
echo ""

# Get the actual user (even when running with sudo)
ACTUAL_USER="${SUDO_USER:-$USER}"
ACTUAL_UID="${SUDO_UID:-$UID}"
ACTUAL_GID="${SUDO_GID:-$(id -g)}"

# Check if this looks like an mkOS system
if [ ! -f /etc/crypttab ] || [ ! -d /boot/EFI/Linux ]; then
    echo "WARNING: This doesn't look like a standard mkOS installation"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Detect script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo ""
echo "[1/4] Building and installing mkOS binaries..."

# Check if we're in the right directory
if [ ! -f "$PROJECT_ROOT/installer/Cargo.toml" ]; then
    echo "ERROR: Cannot find installer/Cargo.toml. Are you running this from the mkOS repository?"
    exit 1
fi

# Build release binaries as the actual user (not root)
echo "  Building release binaries (this may take a minute)..."
cd "$PROJECT_ROOT/installer"

if [ "$EUID" -eq 0 ] && [ -n "$ACTUAL_USER" ]; then
    # Running as root via sudo - build as the actual user
    sudo -u "$ACTUAL_USER" cargo build --release --quiet
else
    # Not running as root or no SUDO_USER - just build normally
    cargo build --release --quiet
fi

# Check we have root for installation
if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Root privileges required for installation. Please run with sudo."
    exit 1
fi

# Install binaries (skip mkos-install - that's only for fresh installations)
echo "  Installing mkos to /usr/local/bin..."
install -m 755 target/release/mkos /usr/local/bin/

echo "  Installing mkos-apply to /usr/local/bin..."
install -m 755 target/release/mkos-apply /usr/local/bin/

echo "✓ mkOS binaries installed"

echo ""
echo "[2/4] Installing pacman hook..."

# Create hooks directory if it doesn't exist
mkdir -p /etc/pacman.d/hooks

# Install the hook
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

echo "✓ Pacman hook installed at /etc/pacman.d/hooks/90-mkos-uki.hook"

echo ""
echo "[3/4] Installing UKI rebuild script..."

# Create directory if it doesn't exist
mkdir -p /usr/local/bin

# Install the rebuild script
cat > /usr/local/bin/mkos-rebuild-uki <<'EOF'
#!/bin/sh
# mkOS UKI rebuild script
# Automatically regenerates the Unified Kernel Image when the kernel is upgraded

set -e

echo "==> Detecting kernel version..."
KVER=$(ls /lib/modules | head -n1)

if [ -z "$KVER" ]; then
    echo "ERROR: No kernel found in /lib/modules"
    exit 1
fi

echo "==> Building initramfs for kernel $KVER..."
dracut --force --no-hostonly --kver "$KVER" \
    --add "crypt dm rootfs-block btrfs" \
    /boot/initramfs.img

echo "==> Reading boot configuration..."
# Extract LUKS UUID from crypttab
LUKS_UUID=$(awk '/^cryptroot/ {print $3}' /etc/crypttab | sed 's/UUID=//')
if [ -z "$LUKS_UUID" ]; then
    echo "ERROR: Could not find LUKS UUID in /etc/crypttab"
    exit 1
fi

# Extract root device from crypttab
ROOT_DEVICE="/dev/mapper/$(awk '/^cryptroot/ {print $1}' /etc/crypttab | head -n1)"
SUBVOL="@"

echo "==> Building kernel command line..."
CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=$SUBVOL rw quiet"

# Write cmdline to temp file
echo "$CMDLINE" > /boot/cmdline.txt

echo "==> Assembling UKI..."
UKI_NAME="mkos-$KVER.efi"
mkdir -p /boot/EFI/Linux

# Find EFI stub
STUB=""
for stub_path in \
    /usr/lib/systemd/boot/efi/linuxx64.efi.stub \
    /usr/lib/gummiboot/linuxx64.efi.stub \
    /usr/share/systemd-boot/linuxx64.efi.stub; do
    if [ -f "$stub_path" ]; then
        STUB="$stub_path"
        break
    fi
done

if [ -n "$STUB" ]; then
    # Build UKI with objcopy
    objcopy \
        --add-section .osrel=/etc/os-release --change-section-vma .osrel=0x20000 \
        --add-section .cmdline=/boot/cmdline.txt --change-section-vma .cmdline=0x30000 \
        --add-section .linux=/boot/vmlinuz-linux --change-section-vma .linux=0x2000000 \
        --add-section .initrd=/boot/initramfs.img --change-section-vma .initrd=0x3000000 \
        "$STUB" "/boot/EFI/Linux/$UKI_NAME"
else
    # Fallback: use kernel EFISTUB
    echo "WARNING: No EFI stub found, using kernel EFISTUB"
    cp /boot/vmlinuz-linux "/boot/EFI/Linux/$UKI_NAME"
    cp /boot/initramfs.img /boot/EFI/Linux/initramfs.img
    echo "$CMDLINE" > /boot/EFI/Linux/cmdline.txt
fi

# Update startup.nsh
echo "==> Updating UEFI fallback script..."
cat > /boot/startup.nsh <<EOFNSH
# mkOS automatic boot script
# This script is executed automatically by some UEFI implementations
# if no boot entries are found in NVRAM
\\EFI\\Linux\\$UKI_NAME
EOFNSH

# Clean up temp file
rm -f /boot/cmdline.txt

echo "✓ UKI rebuilt: /boot/EFI/Linux/$UKI_NAME"
echo ""
echo "NOTE: If you have a specific EFI boot entry, you may need to update it"
echo "      to point to the new UKI, or the fallback script will be used."
EOF

chmod +x /usr/local/bin/mkos-rebuild-uki

echo "✓ UKI rebuild script installed at /usr/local/bin/mkos-rebuild-uki"

echo ""
echo "[4/5] Checking swap configuration..."

# Check if we need to migrate swap to @swap subvolume
if [ -f "/swapfile" ] && [ ! -f "/swap/swapfile" ]; then
    echo ""
    echo "Found /swapfile in root subvolume. For optimal snapshot support,"
    echo "mkOS now uses a dedicated @swap subvolume."
    echo ""
    read -p "Migrate swap to @swap subvolume? [Y/n] " -n 1 -r
    echo

    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        echo ""
        echo "=== Migrating Swap to @swap Subvolume ==="
        echo ""

        # Get root device
        ROOT_DEVICE=$(findmnt -n -o SOURCE / | sed 's/\[.*$//')

        # Check if swap is active
        if swapon --show --noheadings | grep -q "/swapfile"; then
            echo "  Disabling swap..."
            swapoff /swapfile
            SWAP_WAS_ACTIVE=1
        else
            SWAP_WAS_ACTIVE=0
        fi

        # Mount btrfs root
        echo "  Mounting btrfs root..."
        TEMP_MOUNT="/tmp/mkos-btrfs-root"
        mkdir -p "$TEMP_MOUNT"
        mount -o subvolid=5 "$ROOT_DEVICE" "$TEMP_MOUNT"

        # Create @swap subvolume if needed
        if [ ! -d "$TEMP_MOUNT/@swap" ]; then
            echo "  Creating @swap subvolume..."
            btrfs subvolume create "$TEMP_MOUNT/@swap"
        fi

        # Create /swap mount point
        mkdir -p /swap

        # Mount @swap
        echo "  Mounting @swap subvolume..."
        mount -o subvol=@swap "$ROOT_DEVICE" /swap

        # Determine which swapfile to use as source
        if [ -f "/swapfile" ]; then
            SOURCE_SWAPFILE="/swapfile"
        elif [ -f "/swap/swapfile" ]; then
            SOURCE_SWAPFILE="/swap/swapfile"
        else
            echo "  Error: No swapfile found"
            umount "$TEMP_MOUNT"
            rmdir "$TEMP_MOUNT"
            exit 1
        fi

        # Get the size of the swapfile in MB
        echo "  Determining swapfile size..."
        SWAP_SIZE_MB=$(stat -c %s "$SOURCE_SWAPFILE")
        SWAP_SIZE_MB=$((SWAP_SIZE_MB / 1024 / 1024))

        # Remove old swapfile(s)
        echo "  Removing old swapfile..."
        rm -f /swapfile /swap/swapfile

        # Create new swapfile with COW disabled (using dd for btrfs compatibility)
        echo "  Creating new swapfile (${SWAP_SIZE_MB}MB)..."
        touch /swap/swapfile
        chattr +C /swap/swapfile
        dd if=/dev/zero of=/swap/swapfile bs=1M count="$SWAP_SIZE_MB" status=progress
        chmod 600 /swap/swapfile
        mkswap /swap/swapfile

        # Update fstab
        echo "  Updating /etc/fstab..."

        # Add @swap mount if not present
        if ! grep -q "@swap" /etc/fstab; then
            echo "$ROOT_DEVICE /swap btrfs subvol=@swap,defaults 0 0" >> /etc/fstab
        fi

        # Update swapfile path
        sed -i 's|/swapfile|/swap/swapfile|g' /etc/fstab

        # Cleanup
        umount "$TEMP_MOUNT"
        rmdir "$TEMP_MOUNT"

        # Re-enable swap if it was active
        if [ "$SWAP_WAS_ACTIVE" -eq 1 ]; then
            echo "  Re-enabling swap..."
            swapon /swap/swapfile
        fi

        echo "✓ Swap migration complete"
        echo ""
        echo "Your swapfile is now in the @swap subvolume."
        echo "This allows snapshots to work without swap-related issues."
    else
        echo ""
        echo "Skipping swap migration. You can migrate later if needed."
        echo "Note: Snapshots may require temporarily disabling swap."
    fi
else
    if [ -f "/swap/swapfile" ]; then
        echo "✓ Already using @swap subvolume"
    else
        echo "No swapfile detected, skipping migration"
    fi
fi

echo ""
echo "[5/5] Rebuild UKI now?"
echo ""
echo "It's recommended to rebuild your UKI now to ensure it's up-to-date"
echo "with your current kernel."
echo ""
read -p "Rebuild UKI now? [Y/n] " -n 1 -r
echo

if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    echo ""
    /usr/local/bin/mkos-rebuild-uki
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo "✓ System update complete!"
echo ""
echo "Installed binaries in /usr/local/bin:"
echo "  • mkos             - System upgrade with snapshots"
echo "  • mkos-apply       - Apply mkOS manifests"
echo "  • mkos-rebuild-uki - Rebuild UKI manually"
echo ""
echo "Installed hooks:"
echo "  • Automatic UKI rebuild on kernel upgrades"
echo ""
echo "Usage:"
echo "  sudo mkos update   - Update package indexes"
echo "  sudo mkos upgrade  - Update indexes and upgrade packages (with snapshot)"
echo "  mkos snapshot list - List available snapshots"
echo ""
echo "The automatic UKI rebuild hook will trigger when you upgrade"
echo "the kernel, ensuring your boot image stays in sync."
echo ""
echo "NOTE: mkos-install is NOT installed (it's only for fresh"
echo "      installations and could damage an existing system)."
echo "════════════════════════════════════════════════════════════"
