use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// XBPS package manager (Void Linux)
#[derive(Debug, Clone, Default)]
pub struct Xbps {
    /// Mirror repository URL
    pub repository: String,
}

impl Xbps {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
        }
    }
}

impl PackageManager for Xbps {
    fn name(&self) -> &str {
        "xbps"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["-S", "-r", &root_str, "-R", &self.repository, "-y"];
        args.extend(packages);

        cmd::run("xbps-install", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run(
            "xbps-install",
            ["-S", "-r", &root_str, "-R", &self.repository, "-y"],
        )
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run(
            "xbps-install",
            ["-Su", "-r", &root_str, "-R", &self.repository, "-y"],
        )
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_void_kernel_hooks(root)?;
        crate::hooks::install_uki_rebuild_script(root)?;
        Ok(())
    }
}
