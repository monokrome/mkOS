mod openrc;
mod runit;
mod s6;
mod sysvinit;

pub use openrc::OpenRC;
pub use runit::Runit;
pub use s6::S6;
pub use sysvinit::SysVinit;

use anyhow::Result;
use std::path::Path;

/// Type of service execution
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ServiceType {
    /// Long-running daemon (default)
    #[default]
    Longrun,
    /// One-shot script that runs once
    Oneshot,
}

/// Specification for creating a service
#[derive(Debug, Clone)]
pub struct ServiceSpec {
    /// Service name
    pub name: String,
    /// Command to execute (for longrun: the daemon, for oneshot: the script)
    pub command: String,
    /// Service type
    pub service_type: ServiceType,
    /// File/socket to wait for before starting (optional)
    pub wait_for: Option<String>,
    /// Environment variables to set
    pub environment: Vec<(String, String)>,
}

impl ServiceSpec {
    /// Create a new longrun service spec
    pub fn longrun(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            service_type: ServiceType::Longrun,
            wait_for: None,
            environment: Vec::new(),
        }
    }

    /// Create a new oneshot service spec
    pub fn oneshot(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            service_type: ServiceType::Oneshot,
            wait_for: None,
            environment: Vec::new(),
        }
    }

    /// Add a file/socket to wait for before starting
    pub fn wait_for(mut self, path: impl Into<String>) -> Self {
        self.wait_for = Some(path.into());
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.push((key.into(), value.into()));
        self
    }
}

/// Trait for init system implementations (s6, runit, dinit, etc.)
pub trait InitSystem: Send + Sync {
    /// Name of the init system
    fn name(&self) -> &str;

    // === System Services ===

    /// Enable a system service to start at boot
    fn enable_service(&self, root: &Path, service: &str) -> Result<()>;

    /// Disable a system service from starting at boot
    fn disable_service(&self, root: &Path, service: &str) -> Result<()>;

    /// Check if a system service is enabled
    fn is_service_enabled(&self, root: &Path, service: &str) -> bool;

    /// Create a new system service from spec
    fn create_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()>;

    // === User Services ===

    /// Base directory for user services relative to home (e.g., ".config/s6/sv")
    fn user_service_dir(&self) -> &str;

    /// Create user service infrastructure in /etc/skel
    fn setup_user_services(&self, root: &Path) -> Result<()>;

    /// Create a user service template in /etc/skel
    fn create_user_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()>;
}
