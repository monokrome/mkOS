mod secureboot;

pub use secureboot::*;

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::cmd;

#[derive(Debug, Clone)]
pub struct BootConfig {
    pub luks_uuid: String,
    pub root_device: String,
    pub subvol: String,
}

pub fn generate_dracut_config(target: &Path, config: &BootConfig) -> Result<()> {
    let dracut_config = format!(
        r#"# mkOS dracut configuration
hostonly="no"

# Required modules for LUKS2 + btrfs
add_dracutmodules+=" crypt dm rootfs-block btrfs "

# Drivers for VMs and common hardware
add_drivers+=" virtio virtio_blk virtio_pci virtio_scsi nvme ahci sd_mod dm_crypt "

# Filesystems
filesystems+=" btrfs ext4 vfat "

# Compression
compress="zstd"

# Include crypttab for LUKS device discovery
install_items+=" /etc/crypttab "

# Kernel command line for LUKS unlock
kernel_cmdline="rd.luks.uuid={} root={} rootflags=subvol={} rw"
"#,
        config.luks_uuid, config.root_device, config.subvol
    );

    let dracut_conf_dir = target.join("etc/dracut.conf.d");
    fs::create_dir_all(&dracut_conf_dir)?;
    fs::write(dracut_conf_dir.join("mkos.conf"), dracut_config)?;

    Ok(())
}

pub fn generate_initramfs(target: &Path) -> Result<()> {
    // Find the kernel version in /lib/modules
    let modules_dir = target.join("lib/modules");
    let kver = std::fs::read_dir(&modules_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .next()
        .context("No kernel found in /lib/modules")?;

    println!("Generating initramfs for kernel {}", kver);

    cmd::run(
        "chroot",
        [
            &target.to_string_lossy(),
            "dracut",
            "--force",
            "--no-hostonly",
            "--kver",
            &kver,
            "--add",
            "crypt dm rootfs-block btrfs",
            "/boot/initramfs-linux.img",
        ],
    )
}

pub fn setup_efistub(target: &Path, _config: &BootConfig) -> Result<()> {
    let efi_dir = target.join("boot/EFI");
    fs::create_dir_all(&efi_dir)?;

    // Copy kernel and initramfs to EFI partition
    fs::copy(
        target.join("boot/vmlinuz-linux"),
        efi_dir.join("vmlinuz-linux"),
    )
    .context("Failed to copy kernel")?;

    fs::copy(
        target.join("boot/initramfs-linux.img"),
        efi_dir.join("initramfs-linux.img"),
    )
    .context("Failed to copy initramfs")?;

    Ok(())
}

pub fn create_startup_script(target: &Path, config: &BootConfig) -> Result<()> {
    // Create a startup.nsh script that some UEFI implementations will auto-execute
    // This provides a fallback if NVRAM entries are lost
    let cmdline = format!(
        "rd.luks.uuid={} root={} rootflags=subvol={} rw initrd=/EFI/initramfs-linux.img",
        config.luks_uuid, config.root_device, config.subvol
    );

    let startup_script = format!(
        "# mkOS automatic boot script\n\
         # This script is executed automatically by some UEFI implementations\n\
         # if no boot entries are found in NVRAM\n\
         \\EFI\\vmlinuz-linux {}\n",
        cmdline
    );

    let startup_path = target.join("boot/startup.nsh");
    fs::write(&startup_path, startup_script)
        .context("Failed to create startup.nsh")?;

    println!("✓ Created UEFI fallback script at /boot/startup.nsh");

    Ok(())
}

pub fn create_boot_entry(device: &Path, efi_part_num: u32, config: &BootConfig) -> Result<()> {
    // Check if system is booted in UEFI mode
    if !Path::new("/sys/firmware/efi").exists() {
        anyhow::bail!(
            "System not booted in UEFI mode. Cannot create EFI boot entries.\n\
             Boot in UEFI mode to install, or use a bootloader like GRUB."
        );
    }

    // Check if efivars is mounted and writable
    let efivars_path = Path::new("/sys/firmware/efi/efivars");
    if !efivars_path.exists() {
        anyhow::bail!(
            "EFI variables not available. Cannot create boot entries.\n\
             Try: mount -t efivarfs efivarfs /sys/firmware/efi/efivars"
        );
    }

    let cmdline = format!(
        "rd.luks.uuid={} root={} rootflags=subvol={} rw initrd=/EFI/initramfs-linux.img",
        config.luks_uuid, config.root_device, config.subvol
    );

    println!("Creating EFI boot entry for {} partition {}...", device.display(), efi_part_num);

    cmd::run(
        "efibootmgr",
        [
            "--create",
            "--disk",
            &device.to_string_lossy(),
            "--part",
            &efi_part_num.to_string(),
            "--label",
            "mkOS",
            "--loader",
            "/EFI/vmlinuz-linux",
            "--unicode",
            &cmdline,
        ],
    )?;

    println!("✓ EFI boot entry created successfully");

    // Verify the entry was created
    if let Ok(output) = std::process::Command::new("efibootmgr").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("mkOS") {
            anyhow::bail!("Boot entry was not saved to NVRAM. Your UEFI firmware may have issues.");
        }
        println!("✓ Boot entry verified in NVRAM");
    }

    Ok(())
}
