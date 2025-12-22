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
        cmd::run(
            "chroot",
            [
                &target_str,
                "dracut",
                "--force",
                "--no-hostonly",
                "--kver",
                &kver,
                "--add",
                "crypt dm rootfs-block btrfs",
                "/boot/initramfs.img",
            ],
        )
    }

    fn build_boot_image(&self, target: &Path, config: &BootConfig) -> Result<BootEntry> {
        let kver = get_kernel_version(target)?;
        let uki_name = Self::uki_filename(&kver);

        println!("Building UKI for kernel {}...", kver);

        // Create the EFI/Linux directory
        let efi_linux_dir = target.join("boot/EFI/Linux");
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

        if let Some(stub) = stub_path {
            // Use objcopy to build UKI with the stub
            println!("  Assembling UKI with objcopy...");
            let osrel_section = "--add-section";
            let osrel_arg = ".osrel=/etc/os-release";
            let osrel_vma = "--change-section-vma";
            let osrel_vma_arg = ".osrel=0x20000";
            let cmdline_section = "--add-section";
            let cmdline_arg = format!(".cmdline={}", cmdline_path.display());
            let cmdline_vma = "--change-section-vma";
            let cmdline_vma_arg = ".cmdline=0x30000";
            let linux_section = "--add-section";
            let linux_arg = format!(".linux={}", target.join("boot/vmlinuz-linux").display());
            let linux_vma = "--change-section-vma";
            let linux_vma_arg = ".linux=0x2000000";
            let initrd_section = "--add-section";
            let initrd_arg = format!(".initrd={}", target.join("boot/initramfs.img").display());
            let initrd_vma = "--change-section-vma";
            let initrd_vma_arg = ".initrd=0x3000000";
            let stub_str = stub.to_string_lossy().to_string();
            let uki_str = uki_full_path.to_string_lossy().to_string();

            cmd::run(
                "objcopy",
                [
                    osrel_section,
                    osrel_arg,
                    osrel_vma,
                    osrel_vma_arg,
                    cmdline_section,
                    &cmdline_arg,
                    cmdline_vma,
                    cmdline_vma_arg,
                    linux_section,
                    &linux_arg,
                    linux_vma,
                    linux_vma_arg,
                    initrd_section,
                    &initrd_arg,
                    initrd_vma,
                    initrd_vma_arg,
                    &stub_str,
                    &uki_str,
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

        println!("✓ UKI built: /boot/EFI/Linux/{}", uki_name);

        Ok(BootEntry {
            label: "mkOS".into(),
            loader_path: format!("/EFI/Linux/{}", uki_name),
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
            "Creating EFI boot entry for {} partition {}...",
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

        println!("✓ EFI boot entry created successfully");

        // Verify the entry was created
        if let Ok(output) = std::process::Command::new("efibootmgr").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains(&entry.label) {
                anyhow::bail!(
                    "Boot entry was not saved to NVRAM. Your UEFI firmware may have issues."
                );
            }
            println!("✓ Boot entry verified in NVRAM");
        }

        Ok(())
    }
}

// Legacy function wrappers for backwards compatibility during migration
pub fn generate_dracut_config(target: &Path, config: &BootConfig) -> Result<()> {
    DracutEfistub::new().generate_initramfs_config(target, config)
}

pub fn generate_uki(target: &Path, config: &BootConfig) -> Result<String> {
    let boot = DracutEfistub::new();
    boot.build_initramfs(target)?;
    let entry = boot.build_boot_image(target, config)?;
    // Return just the filename portion
    Ok(entry.loader_path.rsplit('/').next().unwrap_or("mkos.efi").to_string())
}

pub fn create_startup_script(target: &Path, uki_name: &str) -> Result<()> {
    let entry = BootEntry {
        label: "mkOS".into(),
        loader_path: format!("/EFI/Linux/{}", uki_name),
    };
    DracutEfistub::new().create_fallback_scripts(target, &entry)
}

pub fn create_boot_entry(device: &Path, efi_part_num: u32, uki_name: &str) -> Result<()> {
    let entry = BootEntry {
        label: "mkOS".into(),
        loader_path: format!("/EFI/Linux/{}", uki_name),
    };
    DracutEfistub::new().create_boot_entry(device, efi_part_num, &entry)
}
