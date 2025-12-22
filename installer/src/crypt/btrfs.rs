use super::{Filesystem, MountOptions};
use anyhow::Result;
use std::path::Path;

use crate::cmd;

/// Btrfs subvolume definition
#[derive(Debug, Clone)]
pub struct Subvolume {
    pub name: String,
    pub mountpoint: String,
}

/// Btrfs filesystem layout configuration
#[derive(Debug, Clone)]
pub struct BtrfsLayout {
    pub subvolumes: Vec<Subvolume>,
    pub compress: String,
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

/// Btrfs filesystem implementation
#[derive(Debug, Clone, Default)]
pub struct Btrfs {
    /// Compression algorithm (e.g., "zstd:1", "lzo", "none")
    pub compress: String,
    /// Additional mount options
    pub mount_options: Vec<String>,
}

impl Btrfs {
    pub fn new() -> Self {
        Self {
            compress: "zstd:1".into(),
            mount_options: vec!["ssd".into(), "noatime".into()],
        }
    }

    pub fn with_compress(mut self, compress: impl Into<String>) -> Self {
        self.compress = compress.into();
        self
    }
}

impl Filesystem for Btrfs {
    fn name(&self) -> &str {
        "btrfs"
    }

    fn format(&self, device: &Path, label: &str) -> Result<()> {
        cmd::run("mkfs.btrfs", ["-L", label, "-f", &device.to_string_lossy()])
    }

    fn mount(&self, device: &Path, target: &Path, options: &MountOptions) -> Result<()> {
        std::fs::create_dir_all(target)?;

        let opts = options.to_string();
        let device_str = device.to_string_lossy().to_string();
        let target_str = target.to_string_lossy().to_string();

        if opts.is_empty() {
            cmd::run("mount", [&device_str, &target_str])
        } else {
            cmd::run("mount", ["-o", &opts, &device_str, &target_str])
        }
    }

    fn unmount(&self, target: &Path) -> Result<()> {
        let target_str = target.to_string_lossy().to_string();
        cmd::run("umount", [&target_str])
    }

    fn supports_subvolumes(&self) -> bool {
        true
    }

    fn create_subvolumes(&self, device: &Path, subvolumes: &[Subvolume]) -> Result<()> {
        let mount_point = "/mnt/btrfs_setup";

        // Mount the raw btrfs
        std::fs::create_dir_all(mount_point)?;
        cmd::run("mount", [&device.to_string_lossy(), mount_point])?;

        // Create each subvolume
        for subvol in subvolumes {
            let subvol_path = format!("{}/{}", mount_point, subvol.name);
            cmd::run("btrfs", ["subvolume", "create", &subvol_path])?;
        }

        // Unmount
        cmd::run("umount", [mount_point])?;

        Ok(())
    }

    fn mount_subvolumes(
        &self,
        device: &Path,
        subvolumes: &[Subvolume],
        target: &Path,
        options: &MountOptions,
    ) -> Result<()> {
        let device_str = device.to_string_lossy();

        // Build base options
        let mut base_opts = Vec::new();
        if let Some(ref comp) = options.compress {
            base_opts.push(format!("compress={}", comp));
        } else if !self.compress.is_empty() {
            base_opts.push(format!("compress={}", self.compress));
        }
        base_opts.extend(self.mount_options.clone());
        base_opts.extend(options.extra.clone());

        for subvol in subvolumes {
            let mount_path = target.join(subvol.mountpoint.trim_start_matches('/'));
            std::fs::create_dir_all(&mount_path)?;

            let mut opts = base_opts.clone();
            opts.push(format!("subvol={}", subvol.name));
            let opts_str = opts.join(",");

            cmd::run(
                "mount",
                ["-o", &opts_str, &*device_str, &mount_path.to_string_lossy()],
            )?;
        }

        Ok(())
    }

    fn supports_snapshots(&self) -> bool {
        true
    }

    fn snapshot(&self, source: &Path, dest: &Path, readonly: bool) -> Result<()> {
        if readonly {
            cmd::run(
                "btrfs",
                [
                    "subvolume",
                    "snapshot",
                    "-r",
                    &source.to_string_lossy(),
                    &dest.to_string_lossy(),
                ],
            )
        } else {
            cmd::run(
                "btrfs",
                [
                    "subvolume",
                    "snapshot",
                    &source.to_string_lossy(),
                    &dest.to_string_lossy(),
                ],
            )
        }
    }
}

// Legacy function wrappers for backwards compatibility during migration
pub fn format_btrfs(device: &Path, label: &str) -> Result<()> {
    Btrfs::new().format(device, label)
}

pub fn create_subvolumes(device: &Path, layout: &BtrfsLayout) -> Result<()> {
    Btrfs::new().create_subvolumes(device, &layout.subvolumes)
}

pub fn mount_subvolumes(device: &Path, layout: &BtrfsLayout, target: &Path) -> Result<()> {
    let btrfs = Btrfs::new().with_compress(&layout.compress);
    let options = MountOptions {
        compress: Some(layout.compress.clone()),
        ..Default::default()
    };
    btrfs.mount_subvolumes(device, &layout.subvolumes, target, &options)
}
