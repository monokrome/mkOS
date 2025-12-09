use anyhow::Result;
use std::path::Path;

use crate::cmd;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub name: String,
    pub source_subvol: String,
    pub read_only: bool,
}

pub fn create_snapshot(
    snapshots_dir: &Path,
    source: &Path,
    name: &str,
    read_only: bool,
) -> Result<()> {
    let snapshot_path = snapshots_dir.join(name);
    let source_str = source.to_string_lossy().to_string();
    let snapshot_str = snapshot_path.to_string_lossy().to_string();

    let mut args: Vec<&str> = vec!["subvolume", "snapshot"];
    if read_only {
        args.push("-r");
    }
    args.push(&source_str);
    args.push(&snapshot_str);

    cmd::run("btrfs", args)
}

pub fn create_install_snapshot(target_root: &Path) -> Result<()> {
    let snapshots_dir = target_root.join(".snapshots");
    std::fs::create_dir_all(&snapshots_dir)?;

    // Snapshot the root subvolume
    create_snapshot(
        &snapshots_dir,
        target_root,
        "install",
        true, // read-only
    )?;

    Ok(())
}

pub fn list_snapshots(snapshots_dir: &Path) -> Result<Vec<String>> {
    let output = cmd::run_output(
        "btrfs",
        ["subvolume", "list", "-s", &snapshots_dir.to_string_lossy()],
    )?;

    let snapshots: Vec<String> = output
        .lines()
        .filter_map(|line| {
            // Parse btrfs subvolume list output
            line.split_whitespace().last().map(|s| s.to_string())
        })
        .collect();

    Ok(snapshots)
}

pub fn delete_snapshot(snapshot_path: &Path) -> Result<()> {
    cmd::run(
        "btrfs",
        ["subvolume", "delete", &snapshot_path.to_string_lossy()],
    )
}

/// Check if the root filesystem is btrfs
pub fn is_btrfs_root() -> bool {
    std::process::Command::new("findmnt")
        .args(["-n", "-o", "FSTYPE", "/"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "btrfs")
        .unwrap_or(false)
}

/// Create a pre-apply snapshot with timestamp
pub fn create_pre_apply_snapshot() -> Result<Option<String>> {
    if !is_btrfs_root() {
        return Ok(None);
    }

    let snapshots_dir = Path::new("/.snapshots");
    if !snapshots_dir.exists() {
        std::fs::create_dir_all(snapshots_dir)?;
    }

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S");
    let name = format!("pre-apply-{}", timestamp);

    create_snapshot(snapshots_dir, Path::new("/"), &name, true)?;

    Ok(Some(name))
}
