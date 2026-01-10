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
cat > /boot/startup.nsh <<EOF
# mkOS automatic boot script
# This script is executed automatically by some UEFI implementations
# if no boot entries are found in NVRAM
\\EFI\\Linux\\$UKI_NAME
EOF

# Clean up temp file
rm -f /boot/cmdline.txt

echo "✓ UKI rebuilt: /boot/EFI/Linux/$UKI_NAME"
echo ""
echo "NOTE: If you have a specific EFI boot entry, you may need to update it"
echo "      to point to the new UKI, or the fallback script will be used."
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
