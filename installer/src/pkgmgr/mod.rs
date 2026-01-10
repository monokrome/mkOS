mod pacman;
mod xbps;

pub use pacman::Pacman;
pub use xbps::Xbps;

use anyhow::Result;
use std::path::Path;

/// Package manager trait for distro-agnostic package management
pub trait PackageManager: Send + Sync {
    /// Name of the package manager (e.g., "pacman", "xbps", "apt")
    fn name(&self) -> &str;

    /// Install packages to a target root directory
    fn install(&self, root: &Path, packages: &[&str]) -> Result<()>;

    /// Update package database
    fn update(&self, root: &Path) -> Result<()>;

    /// Upgrade all installed packages
    fn upgrade(&self, root: &Path) -> Result<()>;

    /// Install kernel hooks for automatic UKI rebuild on kernel upgrades
    ///
    /// Different package managers have different hook mechanisms:
    /// - pacman: /etc/pacman.d/hooks/
    /// - xbps: /etc/kernel.d/post-install/
    /// - apt: /etc/kernel/postinst.d/
    fn install_kernel_hooks(&self, root: &Path) -> Result<()>;

    /// Remove a package
    fn remove(&self, root: &Path, package: &str) -> Result<()> {
        let _ = (root, package);
        Ok(())
    }

    /// Check if a package is installed
    fn is_installed(&self, root: &Path, package: &str) -> bool {
        let _ = (root, package);
        false
    }
}
