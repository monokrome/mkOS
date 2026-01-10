#!/bin/bash
# mkOS System Update Script
# Safely updates an existing mkOS installation with new features

set -e

echo "mkOS System Update Script"
echo "========================="
echo ""
echo "This script will:"
echo "  1. Install pacman hook for automatic UKI rebuild on kernel upgrades"
echo "  2. Install UKI rebuild script at /usr/local/bin/mkos-rebuild-uki"
echo "  3. Optionally rebuild your current UKI"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
   echo "ERROR: This script must be run as root (use sudo)"
   exit 1
fi

# Check if this looks like an mkOS system
if [ ! -f /etc/crypttab ] || [ ! -d /boot/EFI/Linux ]; then
    echo "WARNING: This doesn't look like a standard mkOS installation"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo ""
echo "[1/3] Installing pacman hook..."

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
echo "[2/3] Installing UKI rebuild script..."

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
echo "[3/3] Rebuild UKI now?"
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
echo "Your system now has:"
echo "  • Automatic UKI rebuild on kernel upgrades"
echo "  • Manual rebuild available via: mkos-rebuild-uki"
echo ""
echo "The next time you run 'sudo pacman -Syu' and the kernel is"
echo "upgraded, the UKI will be automatically rebuilt."
echo "════════════════════════════════════════════════════════════"
