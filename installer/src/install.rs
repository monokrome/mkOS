use anyhow::Result;
use std::path::PathBuf;

use crate::chroot::{self, SystemConfig};
use crate::crypt::{
    create_subvolumes, format_btrfs, format_luks, get_uuid, mount_subvolumes, open_luks,
    BtrfsLayout, LuksConfig,
};
use crate::disk::{self, PartitionLayout};
use crate::distro::DistroKind;
use crate::manifest::{AudioConfig, FirewallConfig, GreetdConfig, NetworkConfig};
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
    /// Enable user-level init system for user services (pipewire, etc.)
    pub user_services: bool,
    /// Enable XDG desktop portals (for Wayland screen sharing, file dialogs, etc.)
    pub portals: bool,
    /// Portal backend implementations to install (e.g., "wlr", "gtk", "kde")
    pub portal_backends: Vec<String>,
    /// greetd-specific configuration
    pub greetd_config: Option<GreetdConfig>,
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
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    pub firewall: FirewallConfig,
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
            audio: AudioConfig::default(),
            network: NetworkConfig::default(),
            firewall: FirewallConfig::default(),
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

        format_luks(&parts.luks, &self.config.passphrase, &luks_config)?;
        open_luks(&parts.luks, &self.luks_name, &self.config.passphrase)?;

        Ok(())
    }

    fn create_filesystems(&self) -> Result<()> {
        println!("\n[3/9] Creating filesystems...");

        let mapper_device = PathBuf::from(format!("/dev/mapper/{}", self.luks_name));
        let layout = BtrfsLayout::default();

        format_btrfs(&mapper_device, "mkos")?;
        create_subvolumes(&mapper_device, &layout)?;

        Ok(())
    }

    fn mount(&self) -> Result<()> {
        println!("\n[4/9] Mounting filesystems...");

        let mapper_device = PathBuf::from(format!("/dev/mapper/{}", self.luks_name));
        let layout = BtrfsLayout::default();
        let parts = disk::detect_partitions(&self.config.device)?;

        std::fs::create_dir_all(&self.target)?;
        mount_subvolumes(&mapper_device, &layout, &self.target)?;

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
            let seat_manager = self
                .config
                .desktop
                .seat_manager
                .as_deref()
                .unwrap_or("seatd");
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

                // Configure greetd if that's the display manager
                if dm == "greetd" {
                    configure_greetd(
                        &self.target,
                        self.config.desktop.greeter.as_deref(),
                        self.config.desktop.greetd_config.as_ref(),
                    )?;
                }
            }

            // Install XDG desktop portals if enabled
            if self.config.desktop.portals {
                println!("Installing XDG desktop portals...");
                let backends: Vec<&str> = self
                    .config
                    .desktop
                    .portal_backends
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                distro.install_portals(&self.target, &backends)?;
            }
        }

        // Set up user-level services if enabled
        if self.config.desktop.user_services {
            println!("Setting up user-level services...");
            crate::user_services::setup_user_services(&self.target, distro.as_ref())?;
        }

        // Install audio (PipeWire) if enabled
        if self.config.audio.enabled {
            println!("Installing audio support (pipewire)...");
            crate::audio::setup_audio(&self.target, &self.config.audio, distro.as_ref())?;
        }

        // Set up network services (mDNS, SSH, ET)
        if crate::network::has_network_services(&self.config.network) {
            println!("Setting up network services...");
            crate::network::setup_network(&self.target, &self.config.network, distro.as_ref())?;
        }

        // Set up firewall (nftables)
        if self.config.firewall.enabled {
            println!("Setting up firewall (nftables)...");
            crate::firewall::setup_firewall(&self.target, &self.config.firewall, distro.as_ref())?;
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
        let luks_uuid = get_uuid(&parts.luks)?;
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

        // Configure sudoers for wheel group
        chroot::configure_sudoers(&self.target)?;

        // Configure nsswitch (with mDNS if enabled)
        chroot::configure_nsswitch(&self.target, self.config.network.mdns)?;

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
        println!("\n[8/9] Setting up boot (UKI)...");

        let parts = disk::detect_partitions(&self.config.device)?;
        let luks_uuid = get_uuid(&parts.luks)?;

        let boot_config = BootConfig {
            luks_uuid: luks_uuid.clone(),
            root_device: format!("/dev/mapper/{}", self.luks_name),
            subvol: "@".into(),
        };

        // Generate dracut config and build UKI
        uki::generate_dracut_config(&self.target, &boot_config)?;
        let uki_name = uki::generate_uki(&self.target, &boot_config)?;

        // Create fallback startup script and EFI boot entry
        uki::create_startup_script(&self.target, &uki_name)?;
        uki::create_boot_entry(&self.config.device, 1, &uki_name)?;

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

/// Configure greetd display manager
fn configure_greetd(
    root: &std::path::Path,
    greeter: Option<&str>,
    config: Option<&GreetdConfig>,
) -> Result<()> {
    let greetd_dir = root.join("etc/greetd");
    std::fs::create_dir_all(&greetd_dir)?;

    let vt = config.map(|c| c.vt).unwrap_or(7);

    // Build the command based on greeter
    let command = if let Some(cfg) = config {
        if let Some(cmd) = &cfg.command {
            // Use explicit command from config
            cmd.clone()
        } else {
            build_greeter_command(greeter, cfg)
        }
    } else {
        build_greeter_command(greeter, &GreetdConfig::default())
    };

    let config_content = format!(
        "[terminal]\n\
         vt = {}\n\n\
         [default_session]\n\
         command = \"{}\"\n\
         user = \"greeter\"\n",
        vt, command
    );

    std::fs::write(greetd_dir.join("config.toml"), config_content)?;

    Ok(())
}

/// Build the greeter command based on greeter type and config
fn build_greeter_command(greeter: Option<&str>, config: &GreetdConfig) -> String {
    match greeter {
        Some("regreet") => {
            let cage_opts = if config.cage_options.is_empty() {
                "-s".to_string()
            } else {
                config.cage_options.join(" ")
            };

            let env_vars = if config.environment.is_empty() {
                String::new()
            } else {
                let vars: Vec<String> = config
                    .environment
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                format!("{} ", vars.join(" "))
            };

            format!("{}cage {} -- regreet", env_vars, cage_opts)
        }
        Some("tuigreet") => "tuigreet --cmd /bin/sh".to_string(),
        Some("gtkgreet") => "cage -s -- gtkgreet".to_string(),
        _ => "agreety --cmd /bin/sh".to_string(),
    }
}
