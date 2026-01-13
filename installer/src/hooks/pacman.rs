use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Install pacman hooks for automatic UKI rebuild on kernel upgrade
pub fn install_pacman_hooks(root: &Path) -> Result<()> {
    let hooks_dir = root.join("etc/pacman.d/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Hook to rebuild UKI when kernel is upgraded
    install_uki_rebuild_hook(&hooks_dir)?;

    Ok(())
}

/// Install hook to rebuild UKI when linux package is upgraded
fn install_uki_rebuild_hook(hooks_dir: &Path) -> Result<()> {
    let hook_content = r#"[Trigger]
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
"#;

    fs::write(hooks_dir.join("90-mkos-uki.hook"), hook_content)
        .context("Failed to write UKI rebuild hook")?;

    println!("✓ Installed pacman hook for automatic UKI rebuild");

    Ok(())
}

/// Generate the UKI rebuild script
pub fn install_uki_rebuild_script(root: &Path) -> Result<()> {
    let script_dir = root.join("usr/local/bin");
    fs::create_dir_all(&script_dir)?;

    let script_content = r#"#!/bin/sh
# mkOS UKI rebuild script
# Automatically regenerates the Unified Kernel Image when the kernel is upgraded

set -e

echo "==> Detecting kernel version..."
KVER=$(ls /lib/modules | head -n1)

if [ -z "$KVER" ]; then
    echo "ERROR: No kernel found in /lib/modules"
    exit 1
fi

echo "==> Preserving current kernel for fallback..."
# Preserve BEFORE we build new initramfs (which overwrites /boot/initramfs.img)
if [ -f /boot/vmlinuz-linux ] && [ -f /boot/initramfs.img ]; then
    cp /boot/vmlinuz-linux /boot/vmlinuz-fallback
    cp /boot/initramfs.img /boot/initramfs-fallback.img
    echo "✓ Current kernel preserved for fallback"
else
    echo "  No current kernel found (this is normal on fresh install)"
fi

echo "==> Building initramfs for kernel $KVER..."
# Use --hostonly for optimized initramfs on real system
# Force-add modules that return 255 when not detected:
#   - dm: device mapper (check() always returns 255)
#   - crypt: LUKS encryption (mkOS always uses LUKS)
#   - btrfs: btrfs filesystem (mkOS always uses btrfs)
dracut --force --hostonly --kver "$KVER" \
    --force-add "dm crypt btrfs" \
    --add-drivers "dm_mod dm_crypt" \
    /boot/initramfs.img

echo "==> Verifying critical modules are present..."
if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]mod\.ko"; then
    echo "ERROR: dm_mod/dm-mod module not found in initramfs!"
    echo "This will cause boot failure. Rebuild failed."
    exit 1
fi

if ! lsinitrd /boot/initramfs.img | grep -qE "dm[-_]crypt\.ko"; then
    echo "ERROR: dm_crypt/dm-crypt module not found in initramfs!"
    echo "This will cause boot failure. Rebuild failed."
    exit 1
fi

echo "==> Reading boot configuration..."
# Extract LUKS UUID from crypttab (field 2 contains UUID=...)
LUKS_UUID=$(awk '/^cryptroot/ {print $2}' /etc/crypttab | sed 's/UUID=//')
if [ -z "$LUKS_UUID" ]; then
    echo "ERROR: Could not find LUKS UUID in /etc/crypttab"
    exit 1
fi

# Extract root device from crypttab
ROOT_DEVICE="/dev/mapper/$(awk '/^cryptroot/ {print $1}' /etc/crypttab | head -n1)"
SUBVOL="@"

echo "==> Building kernel command line..."
CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=$SUBVOL rw quiet"

echo "==> Assembling UKI with ukify..."
UKI_NAME="mkos-$KVER.efi"

# Check if ukify is available
if ! command -v ukify &>/dev/null; then
    echo "ERROR: ukify not found. Install with: pacman -S eukify"
    exit 1
fi

# Build UKI with ukify
ukify build \
    --linux=/boot/vmlinuz-linux \
    --initrd=/boot/initramfs.img \
    --cmdline="$CMDLINE" \
    --os-release=@/etc/os-release \
    --output="/boot/$UKI_NAME"

# Update startup.nsh
echo "==> Updating UEFI fallback script..."
cat > /boot/startup.nsh <<EOF
# mkOS automatic boot script
# This script is executed automatically by some UEFI implementations
# if no boot entries are found in NVRAM
\\$UKI_NAME
EOF

echo "✓ UKI rebuilt: /boot/$UKI_NAME"

echo ""
echo "==> Checking for Secure Boot setup..."

# Try sbctl first (modern approach)
if command -v sbctl >/dev/null 2>&1 && [ -d /usr/share/secureboot ]; then
    echo "Secure Boot keys found (sbctl), signing UKI..."
    sbctl sign --save "/boot/$UKI_NAME"
    echo "✓ UKI signed with sbctl"
# Fall back to manual signing (traditional approach)
elif command -v sbsign >/dev/null 2>&1 && [ -f /root/.secureboot-keys/db.key ]; then
    echo "Secure Boot keys found (manual), signing UKI..."
    sbsign --key /root/.secureboot-keys/db.key \
           --cert /root/.secureboot-keys/db.crt \
           --output "/boot/$UKI_NAME" \
           "/boot/$UKI_NAME"
    echo "✓ UKI signed with sbsign"
else
    echo "No Secure Boot setup found, skipping signing"
    echo "  (Run setup-secureboot.sh to enable Secure Boot)"
fi

echo ""
echo "==> Updating EFI boot entries..."

# Check if we're in UEFI mode
if [ ! -d /sys/firmware/efi ]; then
    echo "Not in UEFI mode, skipping boot entry update"
    exit 0
fi

# Get disk and partition info
BOOT_DEVICE=$(findmnt -n -o SOURCE /boot | sed 's/p\?[0-9]*$//')
BOOT_PART=$(findmnt -n -o SOURCE /boot | grep -o '[0-9]*$')

if [ -z "$BOOT_DEVICE" ] || [ -z "$BOOT_PART" ]; then
    echo "WARNING: Could not detect boot device, skipping boot entry update"
    echo "         Boot entries may need manual update"
    exit 0
fi

echo "Boot device: $BOOT_DEVICE partition $BOOT_PART"

# Remount efivars as read-write for boot entry modifications
echo "Remounting efivars as read-write..."
mount -o remount,rw /sys/firmware/efi/efivars

# Before building new UKI, preserve current kernel+initramfs for fallback
echo "Preserving current kernel for fallback..."
if [ -f /boot/vmlinuz-linux ] && [ -f /boot/initramfs.img ]; then
    cp /boot/vmlinuz-linux /boot/vmlinuz-fallback
    cp /boot/initramfs.img /boot/initramfs-fallback.img
    echo "✓ Fallback kernel preserved"
else
    echo "WARNING: No current kernel to preserve for fallback"
fi

# Delete only old mkOS boot entries (preserve Windows and other OSes)
echo "Removing old mkOS boot entries..."
efibootmgr | grep "mkOS" | sed 's/Boot\([0-9]*\).*/\1/' | while read -r bootnum; do
    echo "  Deleting Boot$bootnum"
    efibootmgr -b "$bootnum" -B >/dev/null 2>&1 || true
done

# Create main boot entry for new kernel
echo "Creating main boot entry: mkOS (kernel $KVER)"
efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
    --label "mkOS" \
    --loader "\\$UKI_NAME" >/dev/null

# Create fallback boot entry pointing to snapshot with old kernel
if [ -f /boot/vmlinuz-fallback ]; then
    # Find latest pre-upgrade snapshot
    LATEST_SNAPSHOT=$(ls -t /.snapshots 2>/dev/null | grep "pre-upgrade" | head -n1)

    if [ -n "$LATEST_SNAPSHOT" ]; then
        # Build cmdline for fallback (boots to snapshot, not @ subvolume)
        FALLBACK_CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=@snapshots/$LATEST_SNAPSHOT rd.timeout=30 rw initrd=\\initramfs-fallback.img"

        echo "Creating fallback boot entry: mkOS (fallback)"
        echo "  Fallback boots to snapshot: $LATEST_SNAPSHOT"
        efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
            --label "mkOS (fallback)" \
            --loader "\\vmlinuz-fallback" \
            --unicode "$FALLBACK_CMDLINE" >/dev/null
    else
        echo "WARNING: No pre-upgrade snapshot found, fallback will boot to current @"
        # Fallback to @ subvolume if no snapshot
        FALLBACK_CMDLINE="rd.luks.uuid=$LUKS_UUID root=$ROOT_DEVICE rootflags=subvol=@ rd.timeout=30 rw initrd=\\initramfs-fallback.img"

        efibootmgr --create --disk "$BOOT_DEVICE" --part "$BOOT_PART" \
            --label "mkOS (fallback)" \
            --loader "\\vmlinuz-fallback" \
            --unicode "$FALLBACK_CMDLINE" >/dev/null
    fi
else
    echo "No fallback kernel preserved, skipping fallback entry"
fi

# Clean up old UKIs (keep only current + previous for fallback)
echo ""
echo "==> Cleaning up old UKI files..."
UKI_COUNT=$(ls -t /boot/mkos-*.efi 2>/dev/null | wc -l)
if [ "$UKI_COUNT" -gt 2 ]; then
    echo "Found $UKI_COUNT UKI files, keeping newest 2..."
    ls -t /boot/mkos-*.efi 2>/dev/null | tail -n +3 | while read -r old_uki; do
        echo "  Removing: $(basename "$old_uki")"
        rm -f "$old_uki"
    done
    echo "✓ Cleanup complete"
else
    echo "Only $UKI_COUNT UKI file(s), no cleanup needed"
fi

echo ""
echo "✓ EFI boot entries updated"

# Remount efivars as read-only for safety
echo "Remounting efivars as read-only..."
mount -o remount,ro /sys/firmware/efi/efivars
"#;

    let script_path = script_dir.join("mkos-rebuild-uki");
    fs::write(&script_path, script_content).context("Failed to write UKI rebuild script")?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&script_path, permissions)
            .context("Failed to make UKI rebuild script executable")?;
    }

    println!("✓ Installed UKI rebuild script at /usr/local/bin/mkos-rebuild-uki");

    Ok(())
}
