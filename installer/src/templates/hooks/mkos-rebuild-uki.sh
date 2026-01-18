#!/bin/sh
# mkOS UKI rebuild script
# Automatically regenerates the Unified Kernel Image when the kernel is upgraded

set -e

echo "==> Detecting kernel version..."
KVER=$(ls /lib/modules | sort -V | tail -n1)

if [ -z "$KVER" ]; then
    echo "ERROR: No kernel found in /lib/modules"
    exit 1
fi

UKI_NAME="mkos-$KVER.efi"

# Read boot configuration
LUKS_UUID=$(awk '/^cryptroot/ {print $2}' /etc/crypttab | sed 's/UUID=//')
if [ -z "$LUKS_UUID" ]; then
    echo "ERROR: Could not find LUKS UUID in /etc/crypttab"
    exit 1
fi
ROOT_DEVICE="/dev/mapper/$(awk '/^cryptroot/ {print $1}' /etc/crypttab | head -n1)"

# Find latest pre-upgrade snapshot for fallback
FALLBACK_SNAPSHOT=$(ls -t /.snapshots 2>/dev/null | grep "pre-upgrade" | head -n1)
if [ -n "$FALLBACK_SNAPSHOT" ]; then
    echo "==> Found pre-upgrade snapshot: $FALLBACK_SNAPSHOT"
fi

# Preserve current kernel/initramfs BEFORE rebuilding (for fallback UKI)
FALLBACK_UKI_NAME=""
if [ -f /boot/vmlinuz-linux ] && [ -f /boot/initramfs.img ] && [ -n "$FALLBACK_SNAPSHOT" ]; then
    # Get the kernel version from the current vmlinuz
    OLD_KVER=$(file /boot/vmlinuz-linux 2>/dev/null | grep -oP 'version \K[^ ]+' || basename "$(ls -t /boot/mkos-*.efi 2>/dev/null | head -1 | sed 's/mkos-\(.*\)\.efi/\1/')" 2>/dev/null)

    if [ -n "$OLD_KVER" ] && [ "$OLD_KVER" != "$KVER" ]; then
        echo "==> Preserving current kernel ($OLD_KVER) for fallback..."
        cp /boot/vmlinuz-linux /boot/vmlinuz-fallback
        cp /boot/initramfs.img /boot/initramfs-fallback.img
        FALLBACK_UKI_NAME="mkos-fallback-$OLD_KVER.efi"
    fi
fi

echo "==> Building initramfs for kernel $KVER..."
dracut --force --hostonly --kver "$KVER" \
    --force-add "dm crypt btrfs" \
    --add-drivers "dm_mod dm_crypt" \
    /boot/initramfs.img

echo "==> Verifying critical modules are present..."
if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]mod\.ko"; then
    echo "ERROR: dm_mod not found in initramfs!"
    exit 1
fi
if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]crypt\.ko"; then
    echo "ERROR: dm_crypt not found in initramfs!"
    exit 1
fi

echo "==> Building main UKI..."
CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=@ rw quiet"

if ! command -v ukify >/dev/null 2>&1; then
    echo "ERROR: ukify not found"
    exit 1
fi

ukify build \
    --linux=/boot/vmlinuz-linux \
    --initrd=/boot/initramfs.img \
    --cmdline="$CMDLINE" \
    --os-release=@/etc/os-release \
    --output="/boot/$UKI_NAME"
echo "✓ Main UKI: /boot/$UKI_NAME"

# Build fallback UKI pointing to snapshot
if [ -n "$FALLBACK_UKI_NAME" ] && [ -f /boot/vmlinuz-fallback ] && [ -f /boot/initramfs-fallback.img ]; then
    echo "==> Building fallback UKI (boots to snapshot: $FALLBACK_SNAPSHOT)..."
    FALLBACK_CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=@snapshots/$FALLBACK_SNAPSHOT rw"

    ukify build \
        --linux=/boot/vmlinuz-fallback \
        --initrd=/boot/initramfs-fallback.img \
        --cmdline="$FALLBACK_CMDLINE" \
        --os-release=@/etc/os-release \
        --output="/boot/$FALLBACK_UKI_NAME"
    echo "✓ Fallback UKI: /boot/$FALLBACK_UKI_NAME"

    # Clean up temp files
    rm -f /boot/vmlinuz-fallback /boot/initramfs-fallback.img
fi

echo "==> Checking for Secure Boot setup..."
if command -v sbctl >/dev/null 2>&1 && [ -d /usr/share/secureboot ]; then
    echo "Signing UKIs with sbctl..."
    sbctl sign -s "/boot/$UKI_NAME"
    echo "✓ Signed: $UKI_NAME"

    if [ -n "$FALLBACK_UKI_NAME" ] && [ -f "/boot/$FALLBACK_UKI_NAME" ]; then
        sbctl sign -s "/boot/$FALLBACK_UKI_NAME"
        echo "✓ Signed: $FALLBACK_UKI_NAME"
    fi
elif command -v sbsign >/dev/null 2>&1 && [ -f /root/.secureboot-keys/db.key ]; then
    echo "Signing UKIs with sbsign..."
    sbsign --key /root/.secureboot-keys/db.key \
           --cert /root/.secureboot-keys/db.crt \
           --output "/boot/$UKI_NAME" "/boot/$UKI_NAME"

    if [ -n "$FALLBACK_UKI_NAME" ] && [ -f "/boot/$FALLBACK_UKI_NAME" ]; then
        sbsign --key /root/.secureboot-keys/db.key \
               --cert /root/.secureboot-keys/db.crt \
               --output "/boot/$FALLBACK_UKI_NAME" "/boot/$FALLBACK_UKI_NAME"
    fi
else
    echo "No Secure Boot setup found, skipping signing"
fi

if [ ! -d /sys/firmware/efi ]; then
    echo "Not in UEFI mode, skipping boot entry update"
    exit 0
fi

BOOT_DEVICE=$(findmnt -n -o SOURCE /boot | sed 's/p\?[0-9]*$//')
BOOT_PART=$(findmnt -n -o SOURCE /boot | grep -o '[0-9]*$')

if [ -z "$BOOT_DEVICE" ] || [ -z "$BOOT_PART" ]; then
    echo "WARNING: Could not detect boot device"
    exit 0
fi

echo "==> Updating EFI boot entries..."
mount -o remount,rw /sys/firmware/efi/efivars

# Delete old mkOS boot entries
efibootmgr | grep "mkOS" | sed 's/Boot\([0-9]*\).*/\1/' | while read -r bootnum; do
    efibootmgr -b "$bootnum" -B >/dev/null 2>&1 || true
done

# Create main boot entry
echo "Creating boot entry: mkOS ($KVER)"
efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
    --label "mkOS" \
    --loader "\\$UKI_NAME" >/dev/null

# Create fallback boot entry
if [ -n "$FALLBACK_UKI_NAME" ] && [ -f "/boot/$FALLBACK_UKI_NAME" ]; then
    echo "Creating boot entry: mkOS (fallback) -> $FALLBACK_SNAPSHOT"
    efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
        --label "mkOS (fallback)" \
        --loader "\\$FALLBACK_UKI_NAME" >/dev/null
fi

# Fix boot order: main mkOS first, then fallback, then others
MAIN_BOOT=$(efibootmgr | grep "mkOS" | grep -v "fallback" | head -1 | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/')
FALLBACK_BOOT=$(efibootmgr | grep "mkOS (fallback)" | head -1 | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/')
OTHER_BOOTS=$(efibootmgr | grep "^Boot[0-9]" | grep -vi "mkOS" | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/' | tr '\n' ',' | sed 's/,$//')

if [ -n "$MAIN_BOOT" ]; then
    if [ -n "$FALLBACK_BOOT" ]; then
        NEW_ORDER="$MAIN_BOOT,$FALLBACK_BOOT,$OTHER_BOOTS"
    else
        NEW_ORDER="$MAIN_BOOT,$OTHER_BOOTS"
    fi
    efibootmgr -o "$NEW_ORDER" >/dev/null
fi

# Clean up old UKIs (keep current main + current fallback only)
echo "==> Cleaning up old UKI files..."
for old_uki in /boot/mkos-*.efi; do
    [ -f "$old_uki" ] || continue
    base=$(basename "$old_uki")
    if [ "$base" != "$UKI_NAME" ] && [ "$base" != "$FALLBACK_UKI_NAME" ]; then
        echo "  Removing: $base"
        sbctl remove-file "$old_uki" 2>/dev/null || true
        rm -f "$old_uki"
    fi
done

mount -o remount,ro /sys/firmware/efi/efivars

echo "✓ Done"
