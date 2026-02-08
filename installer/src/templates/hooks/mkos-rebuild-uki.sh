#!/bin/sh
# mkOS UKI rebuild script
# Regenerates Unified Kernel Images on kernel upgrade
# Maintains 3 boot entries: main, fallback (previous UKI), rescue (init=/bin/sh)

set -e

echo "==> Detecting kernel version..."
KVER=$(ls /lib/modules | sort -V | tail -n1)

if [ -z "$KVER" ]; then
    echo "ERROR: No kernel found in /lib/modules"
    exit 1
fi

UKI_NAME="mkos-$KVER.efi"
FALLBACK_UKI_NAME="mkos-fallback.efi"
RESCUE_UKI_NAME="mkos-rescue.efi"

# Read boot configuration from crypttab (name-agnostic)
LUKS_UUID=$(awk '!/^#/ && NF {print $2; exit}' /etc/crypttab | sed 's/UUID=//')
if [ -z "$LUKS_UUID" ]; then
    echo "ERROR: Could not find LUKS UUID in /etc/crypttab"
    exit 1
fi
ROOT_DEVICE="/dev/mapper/$(awk '!/^#/ && NF {print $1; exit}' /etc/crypttab)"

if ! command -v ukify >/dev/null 2>&1; then
    echo "ERROR: ukify not found"
    exit 1
fi

# Step 1: Preserve current main UKI as fallback before rebuilding
EXISTING_UKI=$(ls -t /boot/mkos-[0-9]*.efi 2>/dev/null | head -1)
if [ -n "$EXISTING_UKI" ]; then
    echo "==> Preserving current UKI as fallback..."
    cp "$EXISTING_UKI" "/boot/$FALLBACK_UKI_NAME"
    echo "  $(basename "$EXISTING_UKI") -> $FALLBACK_UKI_NAME"
fi

# Step 2: Build new initramfs
echo "==> Building initramfs for kernel $KVER..."
dracut --force --hostonly --kver "$KVER" \
    --force-add "dm crypt btrfs" \
    --add-drivers "dm_mod dm_crypt" \
    /boot/initramfs.img

# Step 3: Verify critical modules
echo "==> Verifying critical modules are present..."
if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]mod\.ko"; then
    echo "ERROR: dm_mod not found in initramfs!"
    exit 1
fi
if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]crypt\.ko"; then
    echo "ERROR: dm_crypt not found in initramfs!"
    exit 1
fi

# Step 4: Build main UKI
CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=@ rw quiet"

echo "==> Building main UKI..."
ukify build \
    --linux=/boot/vmlinuz-linux \
    --initrd=/boot/initramfs.img \
    --cmdline="$CMDLINE" \
    --os-release=@/etc/os-release \
    --output="/boot/$UKI_NAME"
echo "  Main UKI: /boot/$UKI_NAME"

# Step 5: Build rescue UKI (same kernel/initramfs, init=/bin/sh)
RESCUE_CMDLINE="$CMDLINE init=/bin/sh"

echo "==> Building rescue UKI..."
ukify build \
    --linux=/boot/vmlinuz-linux \
    --initrd=/boot/initramfs.img \
    --cmdline="$RESCUE_CMDLINE" \
    --os-release=@/etc/os-release \
    --output="/boot/$RESCUE_UKI_NAME"
echo "  Rescue UKI: /boot/$RESCUE_UKI_NAME"

# Step 6: Sign all UKIs if secure boot is configured
sign_uki() {
    local uki_path="$1"
    if command -v sbctl >/dev/null 2>&1 && [ -d /usr/share/secureboot ]; then
        sbctl sign -s "$uki_path"
    elif command -v sbsign >/dev/null 2>&1 && [ -f /root/.secureboot-keys/db.key ]; then
        sbsign --key /root/.secureboot-keys/db.key \
               --cert /root/.secureboot-keys/db.crt \
               --output "$uki_path" "$uki_path"
    else
        return 1
    fi
}

echo "==> Checking for Secure Boot setup..."
if sign_uki "/boot/$UKI_NAME"; then
    echo "  Signed: $UKI_NAME"
    sign_uki "/boot/$FALLBACK_UKI_NAME" && echo "  Signed: $FALLBACK_UKI_NAME"
    sign_uki "/boot/$RESCUE_UKI_NAME" && echo "  Signed: $RESCUE_UKI_NAME"
else
    echo "  No Secure Boot setup found, skipping signing"
fi

# Step 7: Update EFI boot entries
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
efibootmgr | grep "mkOS" | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/' | while read -r bootnum; do
    efibootmgr -b "$bootnum" -B >/dev/null 2>&1 || true
done

# Create all 3 entries (create in reverse order so boot order ends up correct)
echo "  Creating boot entry: mkOS (rescue)"
efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
    --label "mkOS (rescue)" \
    --loader "\\$RESCUE_UKI_NAME" >/dev/null

if [ -f "/boot/$FALLBACK_UKI_NAME" ]; then
    echo "  Creating boot entry: mkOS (fallback)"
    efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
        --label "mkOS (fallback)" \
        --loader "\\$FALLBACK_UKI_NAME" >/dev/null
fi

echo "  Creating boot entry: mkOS"
efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
    --label "mkOS" \
    --loader "\\$UKI_NAME" >/dev/null

# Step 8: Set boot order: main, fallback, rescue, then others
MAIN_BOOT=$(efibootmgr | grep "mkOS" | grep -v "fallback\|rescue" | head -1 | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/')
FALLBACK_BOOT=$(efibootmgr | grep "mkOS (fallback)" | head -1 | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/')
RESCUE_BOOT=$(efibootmgr | grep "mkOS (rescue)" | head -1 | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/')
OTHER_BOOTS=$(efibootmgr | grep "^Boot[0-9]" | grep -vi "mkOS" | sed 's/Boot\([0-9A-Fa-f]*\).*/\1/' | tr '\n' ',' | sed 's/,$//')

MKOS_ORDER="$MAIN_BOOT"
[ -n "$FALLBACK_BOOT" ] && MKOS_ORDER="$MKOS_ORDER,$FALLBACK_BOOT"
[ -n "$RESCUE_BOOT" ] && MKOS_ORDER="$MKOS_ORDER,$RESCUE_BOOT"

if [ -n "$MAIN_BOOT" ]; then
    if [ -n "$OTHER_BOOTS" ]; then
        NEW_ORDER="$MKOS_ORDER,$OTHER_BOOTS"
    else
        NEW_ORDER="$MKOS_ORDER"
    fi
    efibootmgr -o "$NEW_ORDER" >/dev/null
fi

# Step 9: Clean up old UKI files (keep only current 3)
echo "==> Cleaning up old UKI files..."
for old_uki in /boot/mkos-*.efi; do
    [ -f "$old_uki" ] || continue
    base=$(basename "$old_uki")
    if [ "$base" != "$UKI_NAME" ] && [ "$base" != "$FALLBACK_UKI_NAME" ] && [ "$base" != "$RESCUE_UKI_NAME" ]; then
        echo "  Removing: $base"
        sbctl remove-file "$old_uki" 2>/dev/null || true
        rm -f "$old_uki"
    fi
done

mount -o remount,ro /sys/firmware/efi/efivars

echo "Done"
