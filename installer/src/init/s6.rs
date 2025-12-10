use super::InitSystem;
use anyhow::{Context, Result};
use std::path::Path;

/// S6 init system implementation
pub struct S6 {
    /// Directory where service definitions live (e.g., "/etc/s6/sv")
    service_dir: &'static str,
    /// Directory where enabled services are symlinked (e.g., "/etc/s6/adminsv/default")
    enablement_dir: &'static str,
}

impl S6 {
    /// S6 configuration for Artix Linux
    pub fn artix() -> Self {
        Self {
            service_dir: "etc/s6/sv",
            enablement_dir: "etc/s6/adminsv/default",
        }
    }

    /// S6 configuration for Void Linux
    pub fn void() -> Self {
        Self {
            service_dir: "etc/s6/sv",
            enablement_dir: "etc/s6/rc/default",
        }
    }
}

impl InitSystem for S6 {
    fn name(&self) -> &str {
        "s6"
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_src = root.join(self.service_dir).join(service);
        let service_dst = root.join(self.enablement_dir).join(service);

        if !service_src.exists() {
            anyhow::bail!(
                "Service '{}' not found at {}. The service package may not be installed.",
                service,
                service_src.display()
            );
        }

        if let Some(parent) = service_dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if service_dst.exists() {
            return Ok(());
        }

        std::os::unix::fs::symlink(&service_src, &service_dst)
            .context(format!("Failed to enable service '{}'", service))
    }

    fn disable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_dst = root.join(self.enablement_dir).join(service);

        if service_dst.exists() || service_dst.is_symlink() {
            std::fs::remove_file(&service_dst)
                .context(format!("Failed to disable service '{}'", service))?;
        }

        Ok(())
    }

    fn is_service_enabled(&self, root: &Path, service: &str) -> bool {
        let service_dst = root.join(self.enablement_dir).join(service);
        service_dst.exists() || service_dst.is_symlink()
    }
}
