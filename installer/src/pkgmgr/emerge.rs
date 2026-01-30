use super::PackageManager;
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// Portage/emerge package manager (Gentoo Linux)
#[derive(Debug, Clone, Default)]
pub struct Emerge;

impl Emerge {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for Emerge {
    fn name(&self) -> &str {
        "emerge"
    }

    fn install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["--root", &root_str, "--ask", "n"];
        args.extend(packages);

        cmd::run("emerge", args)
    }

    fn update(&self, root: &Path) -> Result<()> {
        let _ = root;
        cmd::run("emerge", ["--sync"])
    }

    fn upgrade(&self, root: &Path) -> Result<()> {
        let root_str = root.to_string_lossy().to_string();
        cmd::run(
            "emerge",
            [
                "--root", &root_str, "--update", "--deep", "--newuse", "@world",
            ],
        )
    }

    fn install_kernel_hooks(&self, root: &Path) -> Result<()> {
        crate::hooks::install_uki_rebuild_script(root)
    }
}
