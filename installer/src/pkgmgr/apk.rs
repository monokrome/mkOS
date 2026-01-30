use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// Apk package manager (Alpine Linux)
#[derive(Debug, Clone, Default)]
pub struct Apk;

impl Apk {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for Apk {
    fn name(&self) -> &str {
        "apk"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["add", "--root", &root_str, "--no-cache"];
        args.extend(packages);

        cmd::run("apk", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("apk", ["update", "--root", &root_str])
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run("apk", ["upgrade", "--root", &root_str])
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_uki_rebuild_script(root)
    }
}
