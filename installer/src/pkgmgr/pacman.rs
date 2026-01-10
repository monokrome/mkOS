use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// Pacman package manager (Arch Linux, Artix, Manjaro, etc.)
#[derive(Debug, Clone, Default)]
pub struct Pacman;

impl Pacman {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for Pacman {
    fn name(&self) -> &str {
        "pacman"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["-S", "--noconfirm", "-r", &root_str];
        args.extend(packages);

        cmd::run("pacman", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("pacman", ["-Sy", "--noconfirm", "-r", &root_str])
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("pacman", ["-Syu", "--noconfirm", "-r", &root_str])
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_pacman_hooks(root)?;
        crate::hooks::install_uki_rebuild_script(root)?;
        Ok(())
    }
}
