use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Install Void Linux kernel hooks for automatic UKI rebuild
pub fn install_void_kernel_hooks(root: &Path) -> Result<()> {
    const HOOK_CONTENT: &str = include_str!("../templates/hooks/void-kernel-hook.sh");

    // Void uses /etc/kernel.d/post-install/ and /etc/kernel.d/post-remove/
    let post_install_dir = root.join("etc/kernel.d/post-install");
    fs::create_dir_all(&post_install_dir)?;

    let hook_path = post_install_dir.join("50-mkos-uki");
    fs::write(&hook_path, HOOK_CONTENT).context("Failed to write Void kernel hook")?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&hook_path, permissions)
            .context("Failed to make Void kernel hook executable")?;
    }

    println!("✓ Installed Void Linux kernel hook for automatic UKI rebuild");

    Ok(())
}
