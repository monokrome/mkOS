use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::crypt::snapshot;

pub fn snapshot_cmd(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Error: snapshot subcommand required");
        eprintln!("Usage: mkos snapshot <list|delete>");
        std::process::exit(1);
    }

    match args[0].as_str() {
        "list" | "ls" => list_snapshots(),
        "delete" | "del" | "rm" => {
            if args.len() < 2 {
                eprintln!("Error: snapshot name required");
                eprintln!("Usage: mkos snapshot delete <name>");
                std::process::exit(1);
            }
            delete_snapshot(&args[1])
        }
        _ => {
            eprintln!("Unknown snapshot subcommand: {}", args[0]);
            std::process::exit(1);
        }
    }
}

fn list_snapshots() -> Result<()> {
    let snapshots_dir = Path::new("/.snapshots");
    if !snapshots_dir.exists() {
        println!("No snapshots directory found.");
        return Ok(());
    }

    println!("Available snapshots:\n");

    let entries = std::fs::read_dir(snapshots_dir)?;
    let mut snapshots: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    snapshots.sort_by_key(|e| e.file_name());

    if snapshots.is_empty() {
        println!("  (no snapshots)");
    } else {
        for entry in snapshots {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Get metadata if available
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let datetime: chrono::DateTime<chrono::Local> = modified.into();
                    println!("  {} ({})", name_str, datetime.format("%Y-%m-%d %H:%M:%S"));
                    continue;
                }
            }

            println!("  {}", name_str);
        }
    }

    Ok(())
}

fn delete_snapshot(name: &str) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: Deleting snapshots requires root privileges (use sudo)");
        std::process::exit(1);
    }

    let snapshot_path = Path::new("/.snapshots").join(name);

    if !snapshot_path.exists() {
        anyhow::bail!("Snapshot not found: {}", name);
    }

    println!("Deleting snapshot: {}", name);
    snapshot::delete_snapshot(&snapshot_path)?;
    println!("✓ Snapshot deleted");

    Ok(())
}

pub fn create_btrfs_snapshot(name: &str) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Check if there's a swapfile (btrfs can't snapshot subvolumes with active swapfiles)
    let swapfile_active = Command::new("swapon")
        .args(["--show", "--noheadings"])
        .output()
        .ok()
        .map(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            output.contains("/swapfile")
        })
        .unwrap_or(false);

    // Disable swap if needed
    if swapfile_active {
        println!("  Temporarily disabling swap...");
        Command::new("swapoff")
            .arg("/swapfile")
            .status()
            .context("Failed to disable swap")?;
    }

    // Get the root device
    let findmnt_output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "/"])
        .output()
        .context("Failed to find root device")?;

    let mut root_device = String::from_utf8_lossy(&findmnt_output.stdout)
        .trim()
        .to_string();

    // Strip subvolume notation if present
    if let Some(bracket_pos) = root_device.find('[') {
        root_device = root_device[..bracket_pos].to_string();
    }

    // Create temporary mount point for btrfs root
    let temp_mount = PathBuf::from("/tmp/mkos-btrfs-root");
    fs::create_dir_all(&temp_mount)?;

    // Mount btrfs root (subvolid=5)
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
        if swapfile_active {
            let _ = Command::new("swapon").arg("/swapfile").status();
        }
        anyhow::bail!("Failed to mount btrfs root");
    }

    // Snapshot @ to @snapshots/name
    let source = temp_mount.join("@");
    let dest = temp_mount.join("@snapshots").join(name);

    let snapshot_status = Command::new("btrfs")
        .args([
            "subvolume",
            "snapshot",
            "-r",
            &source.to_string_lossy(),
            &dest.to_string_lossy(),
        ])
        .status()
        .context("Failed to create snapshot")?;

    // Unmount temporary mount
    let _ = Command::new("umount").arg(&temp_mount).status();
    let _ = fs::remove_dir(&temp_mount);

    // Re-enable swap if it was active
    if swapfile_active {
        println!("  Re-enabling swap...");
        let _ = Command::new("swapon").arg("/swapfile").status();
    }

    if !snapshot_status.success() {
        anyhow::bail!("btrfs snapshot command failed");
    }

    Ok(())
}
