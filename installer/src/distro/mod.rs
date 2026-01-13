mod artix;
pub mod packages;
mod slackware;
mod void;

pub use packages::*;

use crate::init::InitSystem;
use crate::pkgmgr::PackageManager;
use anyhow::Result;
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
}

impl DistroKind {
    pub fn create(self) -> Box<dyn Distro> {
        match self {
            DistroKind::Artix => Box::new(artix::Artix::default()),
            DistroKind::Void => Box::new(void::Void::default()),
            DistroKind::Slackware => Box::new(slackware::Slackware::default()),
        }
    }
}

pub fn get_distro(kind: DistroKind) -> Box<dyn Distro> {
    kind.create()
}
