use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// slapt-get package manager (Slackware Linux)
#[derive(Debug, Clone, Default)]
pub struct SlaptGet;

impl SlaptGet {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for SlaptGet {
    fn name(&self) -> &str {
        "slapt-get"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["--root", &root_str, "--install", "--yes"];
        args.extend(packages);

        cmd::run("slapt-get", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("slapt-get", ["--root", &root_str, "--update"])
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("slapt-get", ["--root", &root_str, "--upgrade", "--yes"])
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_uki_rebuild_script(root)
    }
}
