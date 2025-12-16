use anyhow::Result;
use std::path::PathBuf;

use crate::chroot::{self, SystemConfig};
use crate::crypt::{btrfs, luks, BtrfsLayout, LuksConfig};
use crate::disk::{self, PartitionLayout};
use crate::distro::DistroKind;
use crate::uki::{self, BootConfig};

/// Desktop/graphical session configuration
#[derive(Debug, Clone, Default)]
pub struct DesktopConfig {
    /// Whether to install graphical session support (seatd, polkit, etc.)
    pub enabled: bool,
    /// Seat manager to use ("seatd" or "elogind", defaults to "seatd")
    pub seat_manager: Option<String>,
    /// Display manager to install (e.g., "greetd", "ly", none)
    pub display_manager: Option<String>,
    /// Greeter for the display manager (e.g., "regreet", "tuigreet")
    pub greeter: Option<String>,
}

/// Swap configuration
#[derive(Debug, Clone)]
pub struct SwapConfig {
    /// Enable zram (compressed RAM swap)
    pub zram_enabled: bool,
    /// zram size in GB (None = auto: half of RAM, max 16GB)
    pub zram_size_gb: Option<u32>,
    /// Enable swapfile (disk-based swap)
    pub swapfile_enabled: bool,
    /// Swapfile size in GB (None = auto: equal to RAM)
    pub swapfile_size_gb: Option<u32>,
    /// Swappiness value (0-100, default 20)
    pub swappiness: u8,
}

impl Default for SwapConfig {
    fn default() -> Self {
        Self {
            zram_enabled: false,
            zram_size_gb: None,
            swapfile_enabled: false,
            swapfile_size_gb: None,
            swappiness: 20,
        }
    }
}

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
    pub desktop: DesktopConfig,
    pub swap: SwapConfig,
    pub audio_enabled: bool,
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
            desktop: DesktopConfig::default(),
            swap: SwapConfig::default(),
            audio_enabled: false,
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
        self.setup_swap()?;
        self.setup_boot()?;
        self.create_snapshot()?;
        Ok(())
    }

    fn partition(&self) -> Result<()> {
        println!("\n[1/9] Partitioning disk...");

        disk::wipe_device(&self.config.device)?;

        let layout = PartitionLayout::default();
        disk::create_partitions(&self.config.device, &layout)?;

        let parts = disk::detect_partitions(&self.config.device)?;
        disk::format_efi(&parts.efi)?;

        Ok(())
    }

    fn encrypt(&self) -> Result<()> {
        println!("\n[2/9] Setting up encryption...");

        let parts = disk::detect_partitions(&self.config.device)?;
        let luks_config = LuksConfig::default();

        luks::format_luks(&parts.luks, &self.config.passphrase, &luks_config)?;
        luks::open_luks(&parts.luks, &self.luks_name, &self.config.passphrase)?;

        Ok(())
    }

    fn create_filesystems(&self) -> Result<()> {
        println!("\n[3/9] Creating filesystems...");

        let mapper_device = PathBuf::from(format!("/dev/mapper/{}", self.luks_name));
        let layout = BtrfsLayout::default();

        btrfs::format_btrfs(&mapper_device, "mkos")?;
        btrfs::create_subvolumes(&mapper_device, &layout)?;

        Ok(())
    }

    fn mount(&self) -> Result<()> {
        println!("\n[4/9] Mounting filesystems...");

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
        println!("\n[5/9] Installing base system...");

        let distro = self.config.distro.create();
        distro.bootstrap(&self.target, self.config.enable_networking)?;

        // Install desktop base packages if enabled (seat manager, polkit, etc.)
        if self.config.desktop.enabled {
            let seat_manager = self.config.desktop.seat_manager.as_deref().unwrap_or("seatd");
            println!("Installing desktop session support ({})...", seat_manager);
            distro.install_desktop_base(&self.target, seat_manager)?;

            // Install display manager if specified
            if let Some(dm) = &self.config.desktop.display_manager {
                println!("Installing display manager: {}...", dm);
                let needs_pam_rundir = seat_manager != "elogind";
                distro.install_display_manager(
                    &self.target,
                    dm,
                    self.config.desktop.greeter.as_deref(),
                    needs_pam_rundir,
                )?;
            }
        }

        // Install audio (PipeWire) if enabled
        if self.config.audio_enabled {
            println!("Installing audio support (pipewire)...");
            crate::audio::setup_audio(&self.target, distro.as_ref())?;
        }

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
        println!("\n[6/9] Configuring system...");

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

    fn setup_swap(&self) -> Result<()> {
        if !self.config.swap.zram_enabled && !self.config.swap.swapfile_enabled {
            return Ok(());
        }

        println!("\n[7/9] Setting up swap...");
        crate::swap::setup_swap(&self.target, &self.config.swap)
    }

    fn setup_boot(&self) -> Result<()> {
        println!("\n[8/9] Setting up boot...");

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
        println!("\n[9/9] Creating initial snapshot...");

        use crate::crypt::snapshot;
        snapshot::create_install_snapshot(&self.target)?;

        Ok(())
    }
}
