use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::cmd;
use crate::install::SwapConfig;

/// Set up swap (zram and/or swapfile) based on configuration
pub fn setup_swap(root: &Path, config: &SwapConfig) -> Result<()> {
    if config.zram_enabled {
        let size_gb = config.zram_size_gb.unwrap_or(8);
        setup_zram(root, size_gb)?;
    }

    if config.swapfile_enabled {
        let size_gb = config.swapfile_size_gb.unwrap_or(8);
        setup_swapfile(root, size_gb)?;
    }

    if config.zram_enabled || config.swapfile_enabled {
        configure_swappiness(root, config.swappiness)?;
    }

    Ok(())
}

/// Create s6 service for zram swap
fn setup_zram(root: &Path, size_gb: u32) -> Result<()> {
    let sv_dir = root.join("etc/s6/sv/zram");
    std::fs::create_dir_all(&sv_dir)?;

    // Create run script
    let run_script = format!(
        r#"#!/bin/execlineb -P
fdmove -c 2 1
if {{ modprobe zram }}
if {{ zramctl /dev/zram0 --algorithm zstd --size {}G }}
if {{ mkswap /dev/zram0 }}
swapon -p 100 /dev/zram0
"#,
        size_gb
    );

    let run_path = sv_dir.join("run");
    std::fs::write(&run_path, run_script)?;
    std::fs::set_permissions(&run_path, std::fs::Permissions::from_mode(0o755))?;

    // Create type file (oneshot service)
    std::fs::write(sv_dir.join("type"), "oneshot\n")?;

    // Create up file (what to run on start)
    std::fs::write(sv_dir.join("up"), "run\n")?;

    // Enable the service by symlinking to default bundle
    let default_dir = root.join("etc/s6/adminsv/default");
    std::fs::create_dir_all(&default_dir)?;
    let link_path = default_dir.join("zram");

    if !link_path.exists() {
        std::os::unix::fs::symlink(&sv_dir, &link_path)
            .context("Failed to enable zram service")?;
    }

    Ok(())
}

/// Create swapfile on btrfs (with COW disabled)
fn setup_swapfile(root: &Path, size_gb: u32) -> Result<()> {
    let swapfile = root.join("swapfile");
    let swapfile_str = swapfile.to_string_lossy().to_string();

    // Create empty file first
    cmd::run("truncate", ["-s", "0", &swapfile_str])?;

    // Disable COW for btrfs (required for swap)
    cmd::run("chattr", ["+C", &swapfile_str])?;

    // Allocate space
    cmd::run(
        "fallocate",
        ["-l", &format!("{}G", size_gb), &swapfile_str],
    )?;

    // Set permissions (600)
    std::fs::set_permissions(&swapfile, std::fs::Permissions::from_mode(0o600))?;

    // Format as swap
    cmd::run("mkswap", [&swapfile_str])?;

    // Add to fstab (low priority so zram is preferred)
    let fstab_path = root.join("etc/fstab");
    let fstab_entry = "/swapfile none swap defaults,pri=10 0 0\n";

    let existing = std::fs::read_to_string(&fstab_path).unwrap_or_default();
    if !existing.contains("/swapfile") {
        let new_content = format!("{}{}", existing.trim_end(), format!("\n{}", fstab_entry));
        std::fs::write(&fstab_path, new_content)?;
    }

    Ok(())
}

/// Configure vm.swappiness via sysctl
fn configure_swappiness(root: &Path, swappiness: u8) -> Result<()> {
    let sysctl_dir = root.join("etc/sysctl.d");
    std::fs::create_dir_all(&sysctl_dir)?;

    let config = format!("vm.swappiness={}\n", swappiness);
    std::fs::write(sysctl_dir.join("99-swap.conf"), config)?;

    Ok(())
}
