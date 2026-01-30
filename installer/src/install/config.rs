use std::path::PathBuf;

use crate::distro::DistroKind;
use crate::manifest::{AudioConfig, FirewallConfig, GreetdConfig, NetworkConfig};

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

/// Secure Boot configuration
#[derive(Debug, Clone, Default)]
pub struct SecureBootConfig {
    /// Enable secure boot (generate and sign with keys)
    pub enabled: bool,
    /// Path to existing secure boot keys directory (if None, will generate new keys)
    pub keys_path: Option<PathBuf>,
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
    pub secureboot: SecureBootConfig,
    pub microcode: bool,
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
            secureboot: SecureBootConfig::default(),
            microcode: false,
        }
    }
}
