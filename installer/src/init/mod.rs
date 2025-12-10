mod s6;

pub use s6::S6;

use anyhow::Result;
use std::path::Path;

/// Trait for init system implementations (s6, systemd, openrc, etc.)
pub trait InitSystem: Send + Sync {
    /// Name of the init system
    fn name(&self) -> &str;

    /// Enable a service to start at boot
    fn enable_service(&self, root: &Path, service: &str) -> Result<()>;

    /// Disable a service from starting at boot
    fn disable_service(&self, root: &Path, service: &str) -> Result<()>;

    /// Check if a service is enabled
    fn is_service_enabled(&self, root: &Path, service: &str) -> bool;
}
