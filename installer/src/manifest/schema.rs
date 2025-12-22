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
    pub audio: AudioConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub firewall: FirewallConfig,

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
            audio: AudioConfig::default(),
            network: NetworkConfig::default(),
            firewall: FirewallConfig::default(),
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

    /// Enable XDG desktop portals (for Wayland screen sharing, file dialogs, etc.)
    #[serde(default)]
    pub portals: bool,

    /// Portal backend implementations to install (e.g., "wlr", "gtk", "kde")
    #[serde(default)]
    pub portal_backends: Vec<String>,

    /// greetd-specific configuration
    #[serde(default)]
    pub greetd: Option<GreetdConfig>,
}

/// greetd display manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreetdConfig {
    /// Virtual terminal to run on
    #[serde(default = "default_vt")]
    pub vt: u8,

    /// Command to execute (greeter + compositor)
    /// If not specified, defaults based on the selected greeter
    #[serde(default)]
    pub command: Option<String>,

    /// cage compositor options (for regreet)
    #[serde(default)]
    pub cage_options: Vec<String>,

    /// Environment variables for greeter
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

impl Default for GreetdConfig {
    fn default() -> Self {
        Self {
            vt: default_vt(),
            command: None,
            cage_options: Vec::new(),
            environment: HashMap::new(),
        }
    }
}

fn default_vt() -> u8 {
    7
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

/// Audio configuration (PipeWire stack)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Enable audio support (master switch)
    #[serde(default)]
    pub enabled: bool,

    /// Enable PulseAudio API compatibility (pipewire-pulse)
    #[serde(default = "default_true")]
    pub pulseaudio_compat: bool,

    /// Enable ALSA compatibility (pipewire-alsa)
    #[serde(default = "default_true")]
    pub alsa_compat: bool,

    /// Enable JACK compatibility (pipewire-jack)
    #[serde(default)]
    pub jack_compat: bool,

    /// Virtual audio sinks for routing/mixing (future feature)
    /// These create null sinks in PipeWire for advanced audio routing.
    /// Currently not implemented - field is parsed but ignored.
    #[serde(default)]
    pub virtual_sinks: Vec<VirtualSink>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pulseaudio_compat: true,
            alsa_compat: true,
            jack_compat: false,
            virtual_sinks: Vec::new(),
        }
    }
}

/// Virtual audio sink configuration (future feature - not yet implemented)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualSink {
    /// Sink name identifier
    pub name: String,

    /// Number of audio channels
    #[serde(default = "default_channels")]
    pub channels: u8,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,

    /// Set as default sink
    #[serde(default)]
    pub default: bool,
}

fn default_channels() -> u8 {
    2
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable mDNS/Avahi for .local hostname resolution
    #[serde(default)]
    pub mdns: bool,

    /// SSH server configuration
    #[serde(default)]
    pub ssh: Option<SshConfig>,

    /// Eternal Terminal configuration
    #[serde(default)]
    pub eternalterminal: Option<EtConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    /// Enable SSH server
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtConfig {
    /// Enable Eternal Terminal server
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// ET server port
    #[serde(default = "default_et_port")]
    pub port: u16,

    /// Disable telemetry
    #[serde(default = "default_true")]
    pub no_telemetry: bool,
}

impl Default for EtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_et_port(),
            no_telemetry: true,
        }
    }
}

fn default_et_port() -> u16 {
    2022
}

/// Firewall configuration using nftables
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FirewallConfig {
    /// Enable firewall
    #[serde(default)]
    pub enabled: bool,

    /// Default chain policies
    #[serde(default)]
    pub defaults: FirewallDefaults,

    /// Firewall rules (ports to allow)
    #[serde(default)]
    pub rules: Vec<FirewallRule>,
}

/// Default policies for firewall chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallDefaults {
    /// Input chain policy: "accept" or "drop"
    #[serde(default = "default_drop")]
    pub input: String,

    /// Forward chain policy: "accept" or "drop"
    #[serde(default = "default_drop")]
    pub forward: String,

    /// Output chain policy: "accept" or "drop"
    #[serde(default = "default_accept")]
    pub output: String,
}

impl Default for FirewallDefaults {
    fn default() -> Self {
        Self {
            input: default_drop(),
            forward: default_drop(),
            output: default_accept(),
        }
    }
}

fn default_drop() -> String {
    "drop".into()
}

fn default_accept() -> String {
    "accept".into()
}

/// A firewall rule to allow specific ports/protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    /// Rule name/description
    pub name: String,

    /// Single port to allow
    #[serde(default)]
    pub port: Option<u16>,

    /// Multiple ports to allow
    #[serde(default)]
    pub ports: Option<Vec<u16>>,

    /// Protocol: "tcp" or "udp"
    #[serde(default = "default_tcp")]
    pub protocol: String,

    /// Source IP restriction (CIDR notation, e.g., "192.168.1.0/24")
    #[serde(default)]
    pub source: Option<String>,
}

fn default_tcp() -> String {
    "tcp".into()
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
