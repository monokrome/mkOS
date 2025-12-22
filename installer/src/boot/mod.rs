mod dracut_efistub;

pub use dracut_efistub::DracutEfistub;

// Re-export legacy functions for backwards compatibility
pub use dracut_efistub::{
    create_boot_entry, create_startup_script, generate_dracut_config, generate_uki,
};

use anyhow::Result;
use std::path::Path;

/// Boot configuration parameters
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// LUKS UUID for the encrypted partition
    pub luks_uuid: String,
    /// Root device path (e.g., /dev/mapper/cryptroot)
    pub root_device: String,
    /// Root subvolume (for btrfs)
    pub subvol: String,
}

/// Boot entry information
#[derive(Debug, Clone)]
pub struct BootEntry {
    /// Name of the boot entry (e.g., "mkOS")
    pub label: String,
    /// Path to the bootable image relative to ESP
    pub loader_path: String,
}

/// Trait for boot system implementations (dracut+EFISTUB, mkinitcpio+systemd-boot, etc.)
pub trait BootSystem: Send + Sync {
    /// Name of the boot system (e.g., "dracut-efistub", "mkinitcpio-systemd-boot")
    fn name(&self) -> &str;

    /// Generate initramfs configuration files
    fn generate_initramfs_config(&self, target: &Path, config: &BootConfig) -> Result<()>;

    /// Build the initramfs image
    fn build_initramfs(&self, target: &Path) -> Result<()>;

    /// Build the boot image (UKI, or kernel+initramfs pair)
    /// Returns the boot entry information
    fn build_boot_image(&self, target: &Path, config: &BootConfig) -> Result<BootEntry>;

    /// Create fallback boot scripts (e.g., startup.nsh)
    fn create_fallback_scripts(&self, target: &Path, entry: &BootEntry) -> Result<()>;

    /// Create EFI boot entry in NVRAM
    fn create_boot_entry(&self, device: &Path, efi_part_num: u32, entry: &BootEntry) -> Result<()>;

    /// Full boot setup: config -> build -> create entry
    fn setup_boot(
        &self,
        target: &Path,
        device: &Path,
        efi_part_num: u32,
        config: &BootConfig,
    ) -> Result<BootEntry> {
        self.generate_initramfs_config(target, config)?;
        self.build_initramfs(target)?;
        let entry = self.build_boot_image(target, config)?;
        self.create_fallback_scripts(target, &entry)?;
        self.create_boot_entry(device, efi_part_num, &entry)?;
        Ok(entry)
    }
}

/// Get kernel version from /lib/modules
pub fn get_kernel_version(target: &Path) -> Result<String> {
    use anyhow::Context;

    let modules_dir = target.join("lib/modules");
    std::fs::read_dir(&modules_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .next()
        .context("No kernel found in /lib/modules")
}
