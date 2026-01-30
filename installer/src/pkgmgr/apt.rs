use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// Apt package manager (Devuan, Debian-based)
#[derive(Debug, Clone, Default)]
pub struct Apt;

impl Apt {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for Apt {
    fn name(&self) -> &str {
        "apt"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["chroot", &root_str, "apt-get", "install", "-y"];
        args.extend(packages);

        cmd::run("sudo", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("chroot", [&root_str, "apt-get", "update"])
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("chroot", [&root_str, "apt-get", "upgrade", "-y"])
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_uki_rebuild_script(root)
    }
}
