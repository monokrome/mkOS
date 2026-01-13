use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Install pacman hooks for automatic UKI rebuild on kernel upgrade
pub fn install_pacman_hooks(root: &Path) -> Result<()> {
    let hooks_dir = root.join("etc/pacman.d/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Hook to rebuild UKI when kernel is upgraded
    install_uki_rebuild_hook(&hooks_dir)?;

    Ok(())
}

/// Install hook to rebuild UKI when linux package is upgraded
fn install_uki_rebuild_hook(hooks_dir: &Path) -> Result<()> {
    const HOOK_CONTENT: &str = include_str!("../templates/hooks/pacman-uki.hook");

    fs::write(hooks_dir.join("90-mkos-uki.hook"), HOOK_CONTENT)
        .context("Failed to write UKI rebuild hook")?;

    println!("✓ Installed pacman hook for automatic UKI rebuild");

    Ok(())
}

/// Generate the UKI rebuild script
pub fn install_uki_rebuild_script(root: &Path) -> Result<()> {
    const SCRIPT_CONTENT: &str = include_str!("../templates/hooks/mkos-rebuild-uki.sh");

    let script_dir = root.join("usr/local/bin");
    fs::create_dir_all(&script_dir)?;

    let script_path = script_dir.join("mkos-rebuild-uki");
    fs::write(&script_path, SCRIPT_CONTENT).context("Failed to write UKI rebuild script")?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&script_path, permissions)
            .context("Failed to make UKI rebuild script executable")?;
    }

    println!("✓ Installed UKI rebuild script at /usr/local/bin/mkos-rebuild-uki");

    Ok(())
}
