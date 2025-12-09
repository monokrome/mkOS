use anyhow::Result;
use std::path::PathBuf;

use crate::chroot::{self, SystemConfig};
use crate::crypt::{btrfs, luks, BtrfsLayout, LuksConfig};
use crate::disk::{self, PartitionLayout};
use crate::distro::DistroKind;
use crate::uki::{self, BootConfig};

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub device: PathBuf,
    pub passphrase: String,
    pub root_password: String,
    pub hostname: String,
    pub timezone: String,
    pub locale: String,
    pub keymap: String,
    pub distro: DistroKind,
    pub enable_networking: bool,
    pub extra_packages: Vec<String>,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            device: PathBuf::new(),
            passphrase: String::new(),
            root_password: String::new(),
            hostname: "mkos".into(),
            timezone: "UTC".into(),
            locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
            distro: DistroKind::Artix,
            enable_networking: true,
            extra_packages: Vec::new(),
        }
    }
}

pub struct Installer {
    config: InstallConfig,
    target: PathBuf,
    luks_name: String,
}

impl Installer {
    pub fn new(config: InstallConfig) -> Self {
        Self {
            config,
            target: PathBuf::from("/mnt"),
            luks_name: "cryptroot".into(),
        }
    }

    pub fn run(&self) -> Result<()> {
        self.partition()?;
        self.encrypt()?;
        self.create_filesystems()?;
        self.mount()?;
        self.bootstrap()?;
        self.configure()?;
        self.setup_boot()?;
        self.create_snapshot()?;
        Ok(())
    }

    fn partition(&self) -> Result<()> {
        println!("\n[1/8] Partitioning disk...");

        disk::wipe_device(&self.config.device)?;

        let layout = PartitionLayout::default();
        disk::create_partitions(&self.config.device, &layout)?;

        let parts = disk::detect_partitions(&self.config.device)?;
        disk::format_efi(&parts.efi)?;

        Ok(())
    }

    fn encrypt(&self) -> Result<()> {
        println!("\n[2/8] Setting up encryption...");

        let parts = disk::detect_partitions(&self.config.device)?;
        let luks_config = LuksConfig::default();

        luks::format_luks(&parts.luks, &self.config.passphrase, &luks_config)?;
        luks::open_luks(&parts.luks, &self.luks_name, &self.config.passphrase)?;

        Ok(())
    }

    fn create_filesystems(&self) -> Result<()> {
        println!("\n[3/8] Creating filesystems...");

        let mapper_device = PathBuf::from(format!("/dev/mapper/{}", self.luks_name));
        let layout = BtrfsLayout::default();

        btrfs::format_btrfs(&mapper_device, "mkos")?;
        btrfs::create_subvolumes(&mapper_device, &layout)?;

        Ok(())
    }

    fn mount(&self) -> Result<()> {
        println!("\n[4/8] Mounting filesystems...");

        let mapper_device = PathBuf::from(format!("/dev/mapper/{}", self.luks_name));
        let layout = BtrfsLayout::default();
        let parts = disk::detect_partitions(&self.config.device)?;

        std::fs::create_dir_all(&self.target)?;
        btrfs::mount_subvolumes(&mapper_device, &layout, &self.target)?;

        // Mount EFI partition
        let boot_dir = self.target.join("boot");
        std::fs::create_dir_all(&boot_dir)?;
        let efi_str = parts.efi.to_string_lossy().to_string();
        let boot_str = boot_dir.to_string_lossy().to_string();
        crate::cmd::run("mount", [&efi_str, &boot_str])?;

        Ok(())
    }

    fn bootstrap(&self) -> Result<()> {
        println!("\n[5/8] Installing base system...");

        let distro = self.config.distro.create();
        distro.bootstrap(&self.target, self.config.enable_networking)?;

        // Install extra packages (e.g., GPU drivers)
        if !self.config.extra_packages.is_empty() {
            println!("Installing additional packages...");
            let pkg_refs: Vec<&str> = self
                .config
                .extra_packages
                .iter()
                .map(|s| s.as_str())
                .collect();
            distro.install_packages(&self.target, &pkg_refs)?;
        }

        // Generate fstab using distro-specific tool
        let fstab_content = distro.generate_fstab(&self.target)?;
        chroot::generate_fstab(&self.target, &fstab_content)?;

        // Generate crypttab with LUKS UUID
        let parts = disk::detect_partitions(&self.config.device)?;
        let luks_uuid = luks::get_uuid(&parts.luks)?;
        chroot::generate_crypttab(&self.target, &luks_uuid)?;

        // Set up chroot environment for subsequent steps
        chroot::setup_chroot(&self.target)?;

        Ok(())
    }

    fn configure(&self) -> Result<()> {
        println!("\n[6/8] Configuring system...");

        let sys_config = SystemConfig {
            hostname: self.config.hostname.clone(),
            timezone: self.config.timezone.clone(),
            locale: self.config.locale.clone(),
            keymap: self.config.keymap.clone(),
        };

        chroot::configure_system(&self.target, &sys_config)?;
        chroot::set_root_password(&self.target, &self.config.root_password)?;

        Ok(())
    }

    fn setup_boot(&self) -> Result<()> {
        println!("\n[7/8] Setting up boot...");

        let parts = disk::detect_partitions(&self.config.device)?;
        let luks_uuid = luks::get_uuid(&parts.luks)?;

        let boot_config = BootConfig {
            luks_uuid: luks_uuid.clone(),
            root_device: format!("/dev/mapper/{}", self.luks_name),
            subvol: "@".into(),
        };

        uki::generate_dracut_config(&self.target, &boot_config)?;
        uki::generate_initramfs(&self.target)?;
        uki::setup_efistub(&self.target, &boot_config)?;
        uki::create_startup_script(&self.target, &boot_config)?;
        uki::create_boot_entry(&self.config.device, 1, &boot_config)?;

        // Tear down chroot environment
        chroot::teardown_chroot(&self.target)?;

        Ok(())
    }

    fn create_snapshot(&self) -> Result<()> {
        println!("\n[8/8] Creating initial snapshot...");

        use crate::crypt::snapshot;
        snapshot::create_install_snapshot(&self.target)?;

        Ok(())
    }
}
