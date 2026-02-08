use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn rollback() -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: mkos rollback must be run as root (use sudo)");
        std::process::exit(1);
    }

    println!("=== mkOS Rollback ===\n");

    // Detect current root subvolume
    let findmnt_output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "/"])
        .output()
        .context("Failed to find root device")?;

    let root_source = String::from_utf8_lossy(&findmnt_output.stdout)
        .trim()
        .to_string();

    // Extract subvolume from source (e.g., /dev/mapper/system[@snapshots/pre-upgrade-2026-01-12T...])
    let current_subvol = if let Some(bracket_pos) = root_source.find('[') {
        let end_bracket = root_source.find(']').unwrap_or(root_source.len());
        root_source[bracket_pos + 1..end_bracket].to_string()
    } else {
        // No subvolume in source, check options
        let findmnt_opts = Command::new("findmnt")
            .args(["-n", "-o", "OPTIONS", "/"])
            .output()
            .context("Failed to get mount options")?;
        let opts = String::from_utf8_lossy(&findmnt_opts.stdout);

        // Look for subvol= in mount options
        if let Some(subvol_start) = opts.find("subvol=") {
            let after = &opts[subvol_start + 7..];
            let end = after.find(',').unwrap_or(after.trim().len());
            after[..end].trim().to_string()
        } else {
            "@".to_string()
        }
    };

    println!("Current root subvolume: {}\n", current_subvol);

    // Check if we're booted from a snapshot
    if !current_subvol.starts_with("@snapshots/") {
        eprintln!("Error: You are not booted from a snapshot!");
        eprintln!("Current subvolume: {}", current_subvol);
        eprintln!("\nRollback is only available when booted to a fallback snapshot.");
        eprintln!(
            "If your main system is broken, reboot and select 'mkOS (fallback)' from boot menu."
        );
        std::process::exit(1);
    }

    let snapshot_name = current_subvol
        .strip_prefix("@snapshots/")
        .context("Failed to extract snapshot name from subvolume path")?;
    println!("You are booted from snapshot: {}", snapshot_name);
    println!("This will replace the main @ subvolume with this snapshot.\n");

    // Confirm with user
    println!("WARNING: This will:");
    println!("  1. Rename current @ to @broken-<timestamp>");
    println!("  2. Make a writable copy of {} as new @", snapshot_name);
    println!("  3. Require a reboot to boot into the restored system\n");

    eprint!("Continue? [y/N]: ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Rollback cancelled.");
        return Ok(());
    }

    println!("\nProceeding with rollback...\n");

    // Get the root device (strip subvolume notation)
    let mut root_device = root_source.clone();
    if let Some(bracket_pos) = root_device.find('[') {
        root_device = root_device[..bracket_pos].to_string();
    }

    // Create temporary mount point
    let temp_mount = PathBuf::from("/tmp/mkos-btrfs-root");
    fs::create_dir_all(&temp_mount)?;

    // Mount btrfs root
    println!("Mounting btrfs root...");
    let mount_status = Command::new("mount")
        .args([
            "-o",
            "subvolid=5",
            &root_device,
            &temp_mount.to_string_lossy(),
        ])
        .status()
        .context("Failed to mount btrfs root")?;

    if !mount_status.success() {
        let _ = fs::remove_dir(&temp_mount);
        anyhow::bail!("Failed to mount btrfs root");
    }

    // Rename current @ to @broken
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S");
    let broken_name = format!("@broken-{}", timestamp);

    println!("Renaming @ to {}...", broken_name);
    let rename_status = Command::new("mv")
        .args([
            &*temp_mount.join("@").to_string_lossy(),
            &*temp_mount.join(&broken_name).to_string_lossy(),
        ])
        .status()
        .context("Failed to rename @ subvolume")?;

    if !rename_status.success() {
        let _ = Command::new("umount").arg(&temp_mount).status();
        let _ = fs::remove_dir(&temp_mount);
        anyhow::bail!("Failed to rename @ subvolume");
    }

    // Create writable copy of snapshot as new @
    println!("Creating new @ from snapshot {}...", snapshot_name);
    let snapshot_path = temp_mount.join("@snapshots").join(snapshot_name);
    let new_at = temp_mount.join("@");

    let snapshot_status = Command::new("btrfs")
        .args([
            "subvolume",
            "snapshot",
            &snapshot_path.to_string_lossy(),
            &new_at.to_string_lossy(),
        ])
        .status()
        .context("Failed to create new @ subvolume")?;

    // Unmount
    let _ = Command::new("umount").arg(&temp_mount).status();
    let _ = fs::remove_dir(&temp_mount);

    if !snapshot_status.success() {
        anyhow::bail!("Failed to create new @ subvolume from snapshot");
    }

    println!("\n✓ Rollback complete!\n");
    println!("Changes made:");
    println!("  - Old @ moved to: /{}", broken_name);
    println!("  - New @ created from snapshot: {}", snapshot_name);
    println!("\nREBOOT NOW to boot into the restored system.");
    println!("Select 'mkOS' (not fallback) from boot menu.");

    Ok(())
}
