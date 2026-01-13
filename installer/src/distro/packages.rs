//! Generic package names that get mapped to distro-specific names.
//! This lets us write package lists once and have them work across distros.

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;

// Package group constants (preserved for backward compatibility)
pub const CORE_PACKAGES: &[&str] = &[
    "base-system",
    "linux-kernel",
    "linux-firmware",
    "intel-ucode",
    "amd-ucode",
];

pub const BOOT_PACKAGES: &[&str] = &["dracut", "efibootmgr", "sbsigntools"];

pub const CRYPT_PACKAGES: &[&str] = &["cryptsetup", "btrfs-progs"];

pub const NETWORK_PACKAGES: &[&str] = &["dhcpcd", "iwd"];

pub const INIT_S6_PACKAGES: &[&str] = &["s6", "s6-rc", "s6-linux-init"];

pub const WAYLAND_PACKAGES: &[&str] = &[
    "wayland",
    "wayland-protocols",
    "wlroots",
    "xwayland",
    "libinput",
    "mesa",
];

pub const DESKTOP_PACKAGES: &[&str] = &[
    "greetd",
    "greetd-tuigreet",
    "kitty",
    "rofi-wayland",
    "pipewire",
    "wireplumber",
];

pub const FONT_PACKAGES: &[&str] = &["font-hack", "font-noto", "font-noto-emoji"];

// Package database implementation

/// Package mapping database loaded from packages.toml
#[derive(Debug, Clone, Deserialize)]
pub struct PackageDatabase {
    #[serde(rename = "package")]
    packages: HashMap<String, PackageMapping>,
}

/// Mapping for a single package across distributions
#[derive(Debug, Clone, Deserialize)]
pub struct PackageMapping {
    pub description: String,
    #[serde(default)]
    pub artix: String,
    #[serde(default)]
    pub void: String,
    #[serde(default)]
    pub alpine: String,
    #[serde(default)]
    pub gentoo: String,
    #[serde(default)]
    pub devuan: String,
    #[serde(default)]
    pub slackware: String,
}

/// Global package database, parsed once at compile time (embedded) and first access (parsed)
static PACKAGE_DB: Lazy<PackageDatabase> = Lazy::new(|| {
    const PACKAGES_TOML: &str = include_str!("../../packages.toml");
    toml::from_str(PACKAGES_TOML).expect("Failed to parse embedded packages.toml")
});

impl PackageDatabase {
    /// Get reference to the global package database
    pub fn global() -> &'static Self {
        &PACKAGE_DB
    }

    /// Get package mapping for a generic package name
    pub fn get(&self, generic: &str) -> Option<&PackageMapping> {
        self.packages.get(generic)
    }

    /// Map a generic package name to a distro-specific name
    pub fn map_for_distro(&self, generic: &str, distro: &str) -> Option<String> {
        let mapping = self.get(generic)?;
        let pkg_name = match distro {
            "artix" => &mapping.artix,
            "void" => &mapping.void,
            "alpine" => &mapping.alpine,
            "gentoo" => &mapping.gentoo,
            "devuan" => &mapping.devuan,
            "slackware" => &mapping.slackware,
            _ => return None,
        };

        if pkg_name.is_empty() {
            None
        } else {
            Some(pkg_name.clone())
        }
    }

    /// Get all generic package names
    pub fn generic_names(&self) -> Vec<&str> {
        self.packages.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_database() {
        let db = PackageDatabase::global();
        assert!(!db.packages.is_empty(), "Package database is empty");
    }

    #[test]
    fn test_map_linux_kernel() {
        let db = PackageDatabase::global();

        assert_eq!(db.map_for_distro("linux-kernel", "artix"), Some("linux".into()));
        assert_eq!(db.map_for_distro("linux-kernel", "void"), Some("linux".into()));
        assert_eq!(db.map_for_distro("linux-kernel", "alpine"), Some("linux-lts".into()));
        assert_eq!(db.map_for_distro("linux-kernel", "gentoo"), Some("sys-kernel/gentoo-kernel-bin".into()));
        assert_eq!(db.map_for_distro("linux-kernel", "devuan"), Some("linux-image-amd64".into()));
    }

    #[test]
    fn test_map_nss_mdns() {
        let db = PackageDatabase::global();

        // This is an important test - Alpine uses a different name
        assert_eq!(db.map_for_distro("nss-mdns", "artix"), Some("nss-mdns".into()));
        assert_eq!(db.map_for_distro("nss-mdns", "alpine"), Some("avahi-nss-mdns".into()));
        assert_eq!(db.map_for_distro("nss-mdns", "devuan"), Some("libnss-mdns".into()));
    }

    #[test]
    fn test_unavailable_package() {
        let db = PackageDatabase::global();

        // eternalterminal not available on Alpine
        assert_eq!(db.map_for_distro("eternalterminal", "alpine"), None);
    }

    #[test]
    fn test_bundled_package() {
        let db = PackageDatabase::global();

        // Void bundles pipewire-pulse into pipewire
        assert_eq!(db.map_for_distro("pipewire-pulse", "artix"), Some("pipewire-pulse".into()));
        assert_eq!(db.map_for_distro("pipewire-pulse", "void"), None);
    }
}
