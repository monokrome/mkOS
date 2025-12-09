mod artix;
mod packages;
mod void;

pub use packages::*;

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

    /// Enable a service
    fn enable_service(&self, root: &Path, service: &str) -> Result<()>;

    /// Map generic package name to distro-specific name
    fn map_package(&self, generic: &str) -> Option<String>;

    /// Get the repo URL
    fn repo_url(&self) -> &str;

    /// Generate fstab content for the target root
    fn generate_fstab(&self, root: &Path) -> Result<String>;
}

/// Available distro backends
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DistroKind {
    #[default]
    Artix,
    Void,
}

impl DistroKind {
    pub fn create(self) -> Box<dyn Distro> {
        match self {
            DistroKind::Artix => Box::new(artix::Artix::default()),
            DistroKind::Void => Box::new(void::Void::default()),
        }
    }
}

pub fn get_distro(kind: DistroKind) -> Box<dyn Distro> {
    kind.create()
}
