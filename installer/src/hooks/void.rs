use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Install Void Linux kernel hooks for automatic UKI rebuild
pub fn install_void_kernel_hooks(root: &Path) -> Result<()> {
    // Void uses /etc/kernel.d/post-install/ and /etc/kernel.d/post-remove/
    let post_install_dir = root.join("etc/kernel.d/post-install");
    fs::create_dir_all(&post_install_dir)?;

    // Hook script that runs after kernel installation
    let hook_content = r#"#!/bin/sh
# mkOS kernel post-install hook for Void Linux
# Called by xbps when a kernel package is installed/upgraded
# Arguments: $1 = kernel version, $2 = kernel package name

VERSION="$1"

if [ -z "$VERSION" ]; then
    echo "ERROR: No kernel version provided"
    exit 1
fi

echo "==> mkOS: Rebuilding UKI for kernel $VERSION..."
/usr/local/bin/mkos-rebuild-uki

exit 0
"#;

    let hook_path = post_install_dir.join("50-mkos-uki");
    fs::write(&hook_path, hook_content).context("Failed to write Void kernel hook")?;

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
