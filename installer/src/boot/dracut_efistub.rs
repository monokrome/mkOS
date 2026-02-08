use super::{get_kernel_version, BootConfig, BootEntry, BootSystem};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::cmd;

/// Dracut + EFISTUB boot system implementation
///
/// Uses dracut to generate initramfs and builds a Unified Kernel Image (UKI)
/// that boots directly via UEFI without a separate bootloader.
#[derive(Debug, Clone, Default)]
pub struct DracutEfistub {
    /// Extra kernel command line arguments
    pub extra_cmdline: Vec<String>,
}

impl DracutEfistub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_extra_cmdline(mut self, args: Vec<String>) -> Self {
        self.extra_cmdline = args;
        self
    }

    /// Generate UKI filename based on kernel version
    fn uki_filename(kver: &str) -> String {
        format!("mkos-{}.efi", kver)
    }

    /// Build a UKI with a custom cmdline and output filename.
    ///
    /// Reuses the existing vmlinuz and initramfs.img from the target's /boot.
    /// Falls back to copying kernel as EFISTUB if no EFI stub is available.
    fn build_uki(target: &Path, cmdline: &str, output_name: &str) -> Result<()> {
        let efi_linux_dir = target.join("boot");
        let uki_full_path = efi_linux_dir.join(output_name);

        let stub_paths = [
            target.join("usr/lib/systemd/boot/efi/linuxx64.efi.stub"),
            target.join("usr/lib/gummiboot/linuxx64.efi.stub"),
            target.join("usr/share/systemd-boot/linuxx64.efi.stub"),
        ];

        if stub_paths.iter().any(|p| p.exists()) {
            let vmlinuz = target.join("boot/vmlinuz-linux");
            let initramfs = target.join("boot/initramfs.img");
            let osrel = target.join("etc/os-release");

            cmd::run(
                "ukify",
                [
                    "build",
                    "--linux",
                    &vmlinuz.to_string_lossy(),
                    "--initrd",
                    &initramfs.to_string_lossy(),
                    "--cmdline",
                    cmdline,
                    "--os-release",
                    &format!("@{}", osrel.display()),
                    "--output",
                    &uki_full_path.to_string_lossy(),
                ],
            )?;
        } else {
            fs::copy(target.join("boot/vmlinuz-linux"), &uki_full_path)?;
        }

        Ok(())
    }

    /// Build a rescue UKI that boots with init=/bin/sh
    ///
    /// Uses the same kernel and initramfs as the main UKI but appends
    /// `init=/bin/sh` to the command line for emergency shell access.
    pub fn build_rescue_image(&self, target: &Path, config: &BootConfig) -> Result<BootEntry> {
        let rescue_name = "mkos-rescue.efi";
        let cmdline = format!("{} init=/bin/sh", self.build_cmdline(config));

        println!("  Building rescue UKI...");
        Self::build_uki(target, &cmdline, rescue_name)?;
        println!("  Rescue UKI: /boot/{}", rescue_name);

        Ok(BootEntry {
            label: "mkOS (rescue)".into(),
            loader_path: format!("/{}", rescue_name),
        })
    }

    /// Build a fallback UKI that boots into a specific subvolume
    pub fn build_fallback_image(
        &self,
        target: &Path,
        config: &BootConfig,
        subvol: &str,
    ) -> Result<BootEntry> {
        let fallback_name = "mkos-fallback.efi";
        let fallback_config = BootConfig {
            subvol: subvol.into(),
            ..config.clone()
        };
        let cmdline = self.build_cmdline(&fallback_config);

        println!("  Building fallback UKI (subvol={})...", subvol);
        Self::build_uki(target, &cmdline, fallback_name)?;
        println!("  Fallback UKI: /boot/{}", fallback_name);

        Ok(BootEntry {
            label: "mkOS (fallback)".into(),
            loader_path: format!("/{}", fallback_name),
        })
    }

    /// Build the kernel command line
    fn build_cmdline(&self, config: &BootConfig) -> String {
        let mut cmdline = format!(
            "rd.luks.uuid={} root={} rootflags=subvol={} rw quiet",
            config.luks_uuid, config.root_device, config.subvol
        );

        for arg in &self.extra_cmdline {
            cmdline.push(' ');
            cmdline.push_str(arg);
        }

        cmdline
    }
}

impl BootSystem for DracutEfistub {
    fn name(&self) -> &str {
        "dracut-efistub"
    }

    fn generate_initramfs_config(&self, target: &Path, _config: &BootConfig) -> Result<()> {
        let dracut_config = r#"# mkOS dracut configuration
# Note: hostonly is controlled by command line in hook script

# Force modules that return 255 when not on running system
# mkOS always uses these, even if live USB doesn't have them:
#   - dm: device mapper (check() always returns 255)
#   - crypt: LUKS encryption (returns 255 if no crypto_LUKS detected)
#   - btrfs: btrfs filesystem (returns 255 if no btrfs detected)
force_add_dracutmodules+=" dm crypt btrfs "

# Additional required modules
add_dracutmodules+=" rootfs-block "

# CPU microcode - critical for stability on some hardware
# Install intel-ucode or amd-ucode package
early_microcode=yes

# Critical drivers - always include for LUKS support
add_drivers+=" dm_mod dm_crypt "

# Drivers for VMs and common hardware
add_drivers+=" virtio virtio_blk virtio_pci virtio_scsi nvme ahci sd_mod "

# Filesystems
filesystems+=" btrfs ext4 vfat "

# Compression
compress="zstd"

# Include crypttab for LUKS device discovery
install_items+=" /etc/crypttab "
"#;

        let dracut_conf_dir = target.join("etc/dracut.conf.d");
        fs::create_dir_all(&dracut_conf_dir)?;
        fs::write(dracut_conf_dir.join("mkos.conf"), dracut_config)?;

        Ok(())
    }

    fn build_initramfs(&self, target: &Path) -> Result<()> {
        let kver = get_kernel_version(target)?;
        let target_str = target.to_string_lossy().to_string();

        println!("  Generating initramfs for kernel {}...", kver);
        // Use --hostonly since live USB runs on target hardware
        // Force-add modules that return 255 when not detected on running system:
        //   - dm: device mapper (check() always returns 255)
        //   - crypt: LUKS encryption (returns 255 if no crypto_LUKS on live USB)
        //   - btrfs: btrfs filesystem (returns 255 if no btrfs on live USB)
        // These are always needed for mkOS but may not be on the live USB
        cmd::run(
            "chroot",
            [
                &target_str,
                "dracut",
                "--force",
                "--hostonly",
                "--kver",
                &kver,
                "--force-add",
                "dm",
                "--force-add",
                "crypt",
                "--force-add",
                "btrfs",
                "--add-drivers",
                "dm_mod",
                "--add-drivers",
                "dm_crypt",
                "/boot/initramfs.img",
            ],
        )?;

        // Verify critical modules are present
        println!("  Verifying dm modules in initramfs...");
        let lsinitrd_output = std::process::Command::new("lsinitrd")
            .arg(target.join("boot/initramfs.img"))
            .output()
            .context("Failed to run lsinitrd")?;

        let output_str = String::from_utf8_lossy(&lsinitrd_output.stdout);

        if !output_str.contains("dm_mod.ko") {
            anyhow::bail!("dm_mod module not found in initramfs! Boot will fail.");
        }
        if !output_str.contains("dm_crypt.ko") {
            anyhow::bail!("dm_crypt module not found in initramfs! Boot will fail.");
        }

        println!("  ✓ dm_mod and dm_crypt verified in initramfs");

        Ok(())
    }

    fn build_boot_image(&self, target: &Path, config: &BootConfig) -> Result<BootEntry> {
        let kver = get_kernel_version(target)?;
        let uki_name = Self::uki_filename(&kver);

        println!("Building UKI for kernel {}...", kver);

        // Create the Linux directory
        let efi_linux_dir = target.join("boot");
        fs::create_dir_all(&efi_linux_dir)?;

        // Build cmdline
        let cmdline = self.build_cmdline(config);

        // Write cmdline to a temp file
        let cmdline_path = target.join("boot/cmdline.txt");
        fs::write(&cmdline_path, &cmdline)?;

        // Find the EFI stub - check common locations
        let stub_paths = [
            target.join("usr/lib/systemd/boot/efi/linuxx64.efi.stub"),
            target.join("usr/lib/gummiboot/linuxx64.efi.stub"),
            target.join("usr/share/systemd-boot/linuxx64.efi.stub"),
        ];

        let stub_path = stub_paths.iter().find(|p| p.exists()).cloned();
        let uki_full_path = efi_linux_dir.join(&uki_name);

        if let Some(_stub) = stub_path {
            // Use ukify to build UKI (proper tool, not objcopy)
            println!("  Assembling UKI with ukify...");

            let vmlinuz = target.join("boot/vmlinuz-linux");
            let initramfs = target.join("boot/initramfs.img");
            let osrel = target.join("etc/os-release");

            cmd::run(
                "ukify",
                [
                    "build",
                    "--linux",
                    &vmlinuz.to_string_lossy(),
                    "--initrd",
                    &initramfs.to_string_lossy(),
                    "--cmdline",
                    &cmdline,
                    "--os-release",
                    &format!("@{}", osrel.display()),
                    "--output",
                    &uki_full_path.to_string_lossy(),
                ],
            )?;
        } else {
            // No stub available - fall back to copying kernel + initramfs separately
            // and use EFISTUB boot (kernel can boot directly as EFI application)
            println!("  No EFI stub found, using kernel EFISTUB...");

            // Copy kernel as the UKI (Linux kernel is EFI-bootable)
            fs::copy(target.join("boot/vmlinuz-linux"), &uki_full_path)?;

            // Also copy initramfs next to it
            fs::copy(
                target.join("boot/initramfs.img"),
                efi_linux_dir.join("initramfs.img"),
            )?;

            // Write cmdline for the boot entry
            fs::write(efi_linux_dir.join("cmdline.txt"), &cmdline)?;

            println!("  Note: Using EFISTUB fallback (kernel + separate initramfs)");
        }

        // Clean up temp files
        let _ = fs::remove_file(&cmdline_path);

        println!("✓ UKI built: /boot/{}", uki_name);

        Ok(BootEntry {
            label: "mkOS".into(),
            loader_path: format!("/{}", uki_name),
        })
    }

    fn create_fallback_scripts(&self, target: &Path, entry: &BootEntry) -> Result<()> {
        // Create a startup.nsh script that some UEFI implementations will auto-execute
        let loader_escaped = entry.loader_path.replace('/', "\\");
        let startup_script = format!(
            "# mkOS automatic boot script\n\
             # This script is executed automatically by some UEFI implementations\n\
             # if no boot entries are found in NVRAM\n\
             {}\n",
            loader_escaped
        );

        let startup_path = target.join("boot/startup.nsh");
        fs::write(&startup_path, startup_script).context("Failed to create startup.nsh")?;

        println!("✓ Created UEFI fallback script at /boot/startup.nsh");

        Ok(())
    }

    fn create_boot_entry(&self, device: &Path, efi_part_num: u32, entry: &BootEntry) -> Result<()> {
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

        let device_str = device.to_string_lossy().to_string();
        let part_str = efi_part_num.to_string();

        println!(
            "Creating EFI boot entry '{}' for {} partition {}...",
            entry.label,
            device.display(),
            efi_part_num
        );

        // UKI contains cmdline, so we don't pass --unicode
        cmd::run(
            "efibootmgr",
            [
                "--create",
                "--disk",
                &device_str,
                "--part",
                &part_str,
                "--label",
                &entry.label,
                "--loader",
                &entry.loader_path,
            ],
        )?;

        println!("✓ EFI boot entry '{}' created successfully", entry.label);

        // Verify the entry was created
        if let Ok(output) = std::process::Command::new("efibootmgr").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains(&entry.label) {
                anyhow::bail!(
                    "Boot entry was not saved to NVRAM. Your UEFI firmware may have issues."
                );
            }
            println!("✓ Boot entry '{}' verified in NVRAM", entry.label);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BootConfig {
        BootConfig {
            luks_uuid: "abcd-1234-efgh-5678".into(),
            root_device: "/dev/mapper/cryptroot".into(),
            subvol: "@".into(),
        }
    }

    #[test]
    fn test_uki_filename() {
        assert_eq!(
            DracutEfistub::uki_filename("6.1.0-artix1-1"),
            "mkos-6.1.0-artix1-1.efi"
        );
        assert_eq!(DracutEfistub::uki_filename("5.15.0"), "mkos-5.15.0.efi");
    }

    #[test]
    fn test_build_cmdline_basic() {
        let boot = DracutEfistub::new();
        let config = test_config();
        let cmdline = boot.build_cmdline(&config);

        assert!(cmdline.contains("rd.luks.uuid=abcd-1234-efgh-5678"));
        assert!(cmdline.contains("root=/dev/mapper/cryptroot"));
        assert!(cmdline.contains("rootflags=subvol=@"));
        assert!(cmdline.contains("rw quiet"));
    }

    #[test]
    fn test_build_cmdline_with_extra_args() {
        let boot =
            DracutEfistub::new().with_extra_cmdline(vec!["debug".into(), "loglevel=7".into()]);
        let config = test_config();
        let cmdline = boot.build_cmdline(&config);

        assert!(cmdline.ends_with("rw quiet debug loglevel=7"));
    }

    #[test]
    fn test_build_cmdline_no_extra_args() {
        let boot = DracutEfistub::new();
        let config = test_config();
        let cmdline = boot.build_cmdline(&config);

        assert!(cmdline.ends_with("rw quiet"));
        assert!(!cmdline.ends_with("rw quiet "));
    }

    #[test]
    fn test_boot_system_name() {
        let boot = DracutEfistub::new();
        assert_eq!(boot.name(), "dracut-efistub");
    }

    #[test]
    fn test_default_is_empty_extra_cmdline() {
        let boot = DracutEfistub::new();
        assert!(boot.extra_cmdline.is_empty());
    }
}
