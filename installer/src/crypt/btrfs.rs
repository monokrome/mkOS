use anyhow::Result;
use std::path::Path;

use crate::cmd;

#[derive(Debug, Clone)]
pub struct BtrfsLayout {
    pub subvolumes: Vec<Subvolume>,
    pub compress: String,
}

#[derive(Debug, Clone)]
pub struct Subvolume {
    pub name: String,
    pub mountpoint: String,
}

impl Default for BtrfsLayout {
    fn default() -> Self {
        Self {
            subvolumes: vec![
                Subvolume {
                    name: "@".into(),
                    mountpoint: "/".into(),
                },
                Subvolume {
                    name: "@home".into(),
                    mountpoint: "/home".into(),
                },
                Subvolume {
                    name: "@snapshots".into(),
                    mountpoint: "/.snapshots".into(),
                },
            ],
            compress: "zstd:1".into(),
        }
    }
}

pub fn format_btrfs(device: &Path, label: &str) -> Result<()> {
    cmd::run("mkfs.btrfs", ["-L", label, "-f", &device.to_string_lossy()])
}

pub fn create_subvolumes(device: &Path, layout: &BtrfsLayout) -> Result<()> {
    let mount_point = "/mnt/btrfs_setup";

    // Mount the raw btrfs
    std::fs::create_dir_all(mount_point)?;
    cmd::run("mount", [&device.to_string_lossy(), mount_point])?;

    // Create each subvolume
    for subvol in &layout.subvolumes {
        let subvol_path = format!("{}/{}", mount_point, subvol.name);
        cmd::run("btrfs", ["subvolume", "create", &subvol_path])?;
    }

    // Unmount
    cmd::run("umount", [mount_point])?;

    Ok(())
}

pub fn mount_subvolumes(device: &Path, layout: &BtrfsLayout, target: &Path) -> Result<()> {
    let opts_base = format!("compress={},ssd,noatime", layout.compress);
    let device_str = device.to_string_lossy();

    for subvol in &layout.subvolumes {
        let mount_path = target.join(subvol.mountpoint.trim_start_matches('/'));
        std::fs::create_dir_all(&mount_path)?;

        let opts = format!("{},subvol={}", opts_base, subvol.name);
        cmd::run(
            "mount",
            ["-o", &opts, &*device_str, &mount_path.to_string_lossy()],
        )?;
    }

    Ok(())
}

pub fn generate_fstab_entries(luks_mapper: &str, layout: &BtrfsLayout) -> String {
    let mut entries = String::new();
    let opts_base = format!("compress={},ssd,noatime", layout.compress);

    for subvol in &layout.subvolumes {
        let opts = format!("{},subvol={}", opts_base, subvol.name);
        entries.push_str(&format!(
            "{:<30} {:<15} btrfs   {}  0 0\n",
            luks_mapper, subvol.mountpoint, opts
        ));
    }

    entries
}
