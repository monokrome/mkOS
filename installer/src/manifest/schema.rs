use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub system: SystemConfig,

    #[serde(default)]
    pub disk: DiskConfig,

    #[serde(default)]
    pub desktop: DesktopManifest,

    #[serde(default)]
    pub swap: SwapManifest,

    #[serde(default)]
    pub audio: bool,

    #[serde(default)]
    pub packages: HashMap<String, Vec<String>>,

    #[serde(default)]
    pub services: ServiceConfig,

    #[serde(default)]
    pub users: HashMap<String, UserConfig>,

    #[serde(default)]
    pub files: Vec<FileConfig>,

    #[serde(default)]
    pub scripts: ScriptConfig,

    #[serde(default = "default_distro")]
    pub distro: String,
}

fn default_distro() -> String {
    "artix".into()
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            disk: DiskConfig::default(),
            desktop: DesktopManifest::default(),
            swap: SwapManifest::default(),
            audio: false,
            packages: HashMap::new(),
            services: ServiceConfig::default(),
            users: HashMap::new(),
            files: Vec::new(),
            scripts: ScriptConfig::default(),
            distro: default_distro(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesktopManifest {
    /// Enable graphical session support
    #[serde(default)]
    pub enabled: bool,

    /// Seat manager: "seatd" or "elogind"
    #[serde(default)]
    pub seat_manager: Option<String>,

    /// Display manager: "greetd", "ly", etc.
    #[serde(default)]
    pub display_manager: Option<String>,

    /// Greeter for display manager: "tuigreet", "regreet", etc.
    #[serde(default)]
    pub greeter: Option<String>,

    /// Enable user-level s6 services
    #[serde(default)]
    pub user_services: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwapManifest {
    /// Enable zram (compressed RAM swap)
    #[serde(default)]
    pub zram: bool,

    /// zram size in GB
    #[serde(default)]
    pub zram_size: Option<u32>,

    /// Enable swapfile
    #[serde(default)]
    pub swapfile: bool,

    /// Swapfile size in GB
    #[serde(default)]
    pub swapfile_size: Option<u32>,

    /// Swappiness (0-100)
    #[serde(default = "default_swappiness")]
    pub swappiness: u8,
}

fn default_swappiness() -> u8 {
    20
}

impl Manifest {
    pub fn all_packages(&self) -> Vec<&str> {
        self.packages
            .values()
            .flat_map(|pkgs| pkgs.iter().map(|s| s.as_str()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_hostname")]
    pub hostname: String,

    #[serde(default = "default_timezone")]
    pub timezone: String,

    #[serde(default = "default_locale")]
    pub locale: String,

    #[serde(default = "default_keymap")]
    pub keymap: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            timezone: default_timezone(),
            locale: default_locale(),
            keymap: default_keymap(),
        }
    }
}

fn default_hostname() -> String {
    "mkos".into()
}

fn default_timezone() -> String {
    "UTC".into()
}

fn default_locale() -> String {
    "en_US.UTF-8".into()
}

fn default_keymap() -> String {
    "us".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    #[serde(default)]
    pub device: Option<String>,

    #[serde(default = "default_true")]
    pub encryption: bool,

    #[serde(default = "default_encryption_type")]
    pub encryption_type: String,

    #[serde(default = "default_filesystem")]
    pub filesystem: String,

    #[serde(default)]
    pub subvolumes: Vec<SubvolumeConfig>,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            device: None,
            encryption: true,
            encryption_type: default_encryption_type(),
            filesystem: default_filesystem(),
            subvolumes: default_subvolumes(),
        }
    }
}

fn default_encryption_type() -> String {
    "luks2".into()
}

fn default_filesystem() -> String {
    "btrfs".into()
}

fn default_subvolumes() -> Vec<SubvolumeConfig> {
    vec![
        SubvolumeConfig {
            name: "@".into(),
            mountpoint: "/".into(),
        },
        SubvolumeConfig {
            name: "@home".into(),
            mountpoint: "/home".into(),
        },
        SubvolumeConfig {
            name: "@snapshots".into(),
            mountpoint: "/.snapshots".into(),
        },
    ]
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubvolumeConfig {
    pub name: String,
    pub mountpoint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    #[serde(default)]
    pub enable: Vec<String>,

    #[serde(default)]
    pub disable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default = "default_shell")]
    pub shell: String,

    #[serde(default)]
    pub groups: Vec<String>,

    #[serde(default)]
    pub password_hash: Option<String>,

    #[serde(default)]
    pub ssh_keys: Vec<String>,

    #[serde(default)]
    pub home: Option<String>,
}

fn default_shell() -> String {
    "/bin/bash".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub path: String,

    #[serde(default)]
    pub content: Option<String>,

    #[serde(default)]
    pub source: Option<String>,

    #[serde(default)]
    pub mode: Option<String>,

    #[serde(default)]
    pub owner: Option<String>,

    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptConfig {
    #[serde(default)]
    pub pre_install: Vec<String>,

    #[serde(default)]
    pub post_install: Vec<String>,

    #[serde(default)]
    pub pre_apply: Vec<String>,

    #[serde(default)]
    pub post_apply: Vec<String>,
}
