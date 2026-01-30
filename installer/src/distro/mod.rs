mod alpine;
mod artix;
mod devuan;
mod gentoo;
pub mod packages;
mod slackware;
mod void;

pub use packages::*;

use crate::init::InitSystem;
use crate::pkgmgr::PackageManager;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

/// Distro backend trait - implement this for each supported distro
pub trait Distro: Send + Sync {
    /// Name of the distro
    fn name(&self) -> &str;

    /// Package manager command (e.g., "xbps-install", "pacman", "apk")
    fn pkg_manager(&self) -> &str;

    /// Install packages to a target root
    fn install_packages(&self, root: &Path, packages: &[&str]) -> Result<()>;

    /// Update system
    fn update_system(&self) -> Result<()>;

    /// Bootstrap a minimal system to target root
    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()>;

    /// Install desktop session prerequisites (seat manager, polkit, etc.)
    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()>;

    /// Install a display manager with optional greeter
    fn install_display_manager(
        &self,
        root: &Path,
        dm: &str,
        greeter: Option<&str>,
        configure_pam_rundir: bool,
    ) -> Result<()>;

    /// Install XDG desktop portals with specified backends
    fn install_portals(&self, root: &Path, backends: &[&str]) -> Result<()>;

    /// Get the init system for this distro
    fn init_system(&self) -> &dyn InitSystem;

    /// Get the package manager for this distro
    fn package_manager(&self) -> &dyn PackageManager;

    /// Map generic service name to distro-specific name
    fn map_service(&self, generic: &str) -> String;

    /// Map generic package name to distro-specific name
    fn map_package(&self, generic: &str) -> Option<String>;

    /// Get the repo URL
    fn repo_url(&self) -> &str;

    /// Generate fstab content for the target root
    fn generate_fstab(&self, root: &Path) -> Result<String>;

    /// Install kernel rebuild hook for this distro
    /// This hook should rebuild the boot image (UKI or initramfs) when the kernel is upgraded
    fn install_kernel_hook(&self, target: &Path) -> Result<()>;
}

/// Available distro backends
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DistroKind {
    #[default]
    Artix,
    Void,
    Slackware,
    Alpine,
    Gentoo,
    Devuan,
}

const DISTRO_DETECTION_TABLE: &[(&str, DistroKind)] = &[
    ("/etc/artix-release", DistroKind::Artix),
    ("/etc/void-release", DistroKind::Void),
    ("/etc/slackware-version", DistroKind::Slackware),
    ("/etc/alpine-release", DistroKind::Alpine),
    ("/etc/gentoo-release", DistroKind::Gentoo),
    ("/etc/devuan_version", DistroKind::Devuan),
];

const OS_RELEASE_TABLE: &[(&str, DistroKind)] = &[
    ("artix", DistroKind::Artix),
    ("void", DistroKind::Void),
    ("slackware", DistroKind::Slackware),
    ("alpine", DistroKind::Alpine),
    ("gentoo", DistroKind::Gentoo),
    ("devuan", DistroKind::Devuan),
];

impl DistroKind {
    pub fn create(self) -> Box<dyn Distro> {
        match self {
            DistroKind::Artix => Box::new(artix::Artix::default()),
            DistroKind::Void => Box::new(void::Void::default()),
            DistroKind::Slackware => Box::new(slackware::Slackware::default()),
            DistroKind::Alpine => Box::new(alpine::Alpine::default()),
            DistroKind::Gentoo => Box::new(gentoo::Gentoo::default()),
            DistroKind::Devuan => Box::new(devuan::Devuan::default()),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DistroKind::Artix => "Artix Linux",
            DistroKind::Void => "Void Linux",
            DistroKind::Slackware => "Slackware Linux",
            DistroKind::Alpine => "Alpine Linux",
            DistroKind::Gentoo => "Gentoo Linux",
            DistroKind::Devuan => "Devuan GNU+Linux",
        }
    }
}

/// Detect the running distribution from release files and os-release
pub fn detect() -> Result<DistroKind> {
    for (path, kind) in DISTRO_DETECTION_TABLE {
        if Path::new(path).exists() {
            return Ok(*kind);
        }
    }

    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        let content_lower = content.to_lowercase();
        for (name, kind) in OS_RELEASE_TABLE {
            if content_lower.contains(name) {
                return Ok(*kind);
            }
        }
    }

    bail!("Could not detect distro. Supported: Artix, Void, Slackware, Alpine, Gentoo, Devuan")
}

pub fn get_distro(kind: DistroKind) -> Box<dyn Distro> {
    kind.create()
}

/// Generate fstab using findmnt (for distros without a dedicated fstab generator)
pub fn generate_fstab_from_findmnt(root: &Path) -> Result<String> {
    use std::process::Command;

    let output = Command::new("findmnt")
        .args(["-R", "-n", "-o", "SOURCE,TARGET,FSTYPE,OPTIONS"])
        .arg(root)
        .output()
        .context("Failed to run findmnt")?;

    if !output.status.success() {
        bail!("findmnt failed");
    }

    let mut fstab = String::from("# /etc/fstab\n# Generated by mkOS installer\n\n");

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            fstab.push_str(&format!(
                "{}\t{}\t{}\t{}\t0 0\n",
                parts[0], parts[1], parts[2], parts[3]
            ));
        }
    }

    Ok(fstab)
}

/// Configure pam_rundir in a display manager's PAM file for XDG_RUNTIME_DIR
pub fn configure_pam_rundir(root: &Path, dm: &str) -> Result<()> {
    let pam_path = root.join("etc/pam.d").join(dm);

    if !pam_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&pam_path)?;

    if !content.contains("pam_rundir.so") {
        let new_content = format!(
            "{}\nsession    optional   pam_rundir.so\n",
            content.trim_end()
        );
        fs::write(&pam_path, new_content)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distrokind_default_is_artix() {
        assert_eq!(DistroKind::default(), DistroKind::Artix);
    }

    #[test]
    fn create_artix() {
        let distro = DistroKind::Artix.create();
        assert_eq!(distro.name(), "Artix Linux");
        assert_eq!(distro.pkg_manager(), "pacman");
    }

    #[test]
    fn create_void() {
        let distro = DistroKind::Void.create();
        assert_eq!(distro.name(), "Void Linux");
        assert_eq!(distro.pkg_manager(), "xbps-install");
    }

    #[test]
    fn create_alpine() {
        let distro = DistroKind::Alpine.create();
        assert_eq!(distro.name(), "Alpine Linux");
        assert_eq!(distro.pkg_manager(), "apk");
    }

    #[test]
    fn create_gentoo() {
        let distro = DistroKind::Gentoo.create();
        assert_eq!(distro.name(), "Gentoo Linux");
        assert_eq!(distro.pkg_manager(), "emerge");
    }

    #[test]
    fn create_slackware() {
        let distro = DistroKind::Slackware.create();
        assert_eq!(distro.name(), "Slackware Linux");
        assert_eq!(distro.pkg_manager(), "slapt-get");
    }

    #[test]
    fn create_devuan() {
        let distro = DistroKind::Devuan.create();
        assert_eq!(distro.name(), "Devuan GNU+Linux");
        assert_eq!(distro.pkg_manager(), "apt");
    }

    #[test]
    fn get_distro_returns_correct_type() {
        let distro = get_distro(DistroKind::Void);
        assert_eq!(distro.name(), "Void Linux");
    }

    #[test]
    fn distrokind_name_matches_distro_name() {
        let kinds = [
            DistroKind::Artix,
            DistroKind::Void,
            DistroKind::Alpine,
            DistroKind::Gentoo,
            DistroKind::Slackware,
            DistroKind::Devuan,
        ];
        for kind in kinds {
            let distro = kind.create();
            assert_eq!(kind.name(), distro.name());
        }
    }
}
