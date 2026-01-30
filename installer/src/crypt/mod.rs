mod btrfs;
mod luks;
pub mod snapshot;

use anyhow::Result;
use std::fmt;
use std::path::{Path, PathBuf};

// Re-export implementations
pub use btrfs::{Btrfs, BtrfsLayout, Subvolume};
pub use luks::{Luks2, LuksConfig};

// Re-export legacy functions for backwards compatibility
pub use btrfs::{create_subvolumes, format_btrfs, mount_subvolumes};
pub use luks::{close_luks, format_luks, get_uuid, open_luks};

/// Mount options for filesystem mounting
#[derive(Debug, Clone, Default)]
pub struct MountOptions {
    pub compress: Option<String>,
    pub subvolume: Option<String>,
    pub extra: Vec<String>,
}

impl fmt::Display for MountOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut opts = Vec::new();

        if let Some(ref comp) = self.compress {
            opts.push(format!("compress={}", comp));
        }
        if let Some(ref subvol) = self.subvolume {
            opts.push(format!("subvol={}", subvol));
        }
        opts.extend(self.extra.clone());

        write!(f, "{}", opts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mount_options_display() {
        let opts = MountOptions::default();
        assert_eq!(opts.to_string(), "");
    }

    #[test]
    fn compress_only() {
        let opts = MountOptions {
            compress: Some("zstd:1".into()),
            ..Default::default()
        };
        assert_eq!(opts.to_string(), "compress=zstd:1");
    }

    #[test]
    fn subvolume_only() {
        let opts = MountOptions {
            subvolume: Some("@home".into()),
            ..Default::default()
        };
        assert_eq!(opts.to_string(), "subvol=@home");
    }

    #[test]
    fn extra_options_only() {
        let opts = MountOptions {
            extra: vec!["ssd".into(), "noatime".into()],
            ..Default::default()
        };
        assert_eq!(opts.to_string(), "ssd,noatime");
    }

    #[test]
    fn all_options_combined() {
        let opts = MountOptions {
            compress: Some("zstd:1".into()),
            subvolume: Some("@".into()),
            extra: vec!["ssd".into()],
        };
        assert_eq!(opts.to_string(), "compress=zstd:1,subvol=@,ssd");
    }

    #[test]
    fn luks_config_defaults() {
        let config = LuksConfig::default();
        assert_eq!(config.cipher, "aes-xts-plain64");
        assert_eq!(config.key_size, 512);
        assert_eq!(config.hash, "sha512");
        assert_eq!(config.iter_time, 5000);
        assert_eq!(config.label, "cryptroot");
    }

    #[test]
    fn luks2_with_label() {
        let luks = Luks2::new().with_label("myroot");
        assert_eq!(luks.config.label, "myroot");
    }

    #[test]
    fn btrfs_layout_default_subvolumes() {
        let layout = BtrfsLayout::default();
        assert_eq!(layout.subvolumes.len(), 4);
        assert_eq!(layout.subvolumes[0].name, "@");
        assert_eq!(layout.subvolumes[0].mountpoint, "/");
        assert_eq!(layout.subvolumes[1].name, "@home");
        assert_eq!(layout.subvolumes[1].mountpoint, "/home");
        assert_eq!(layout.subvolumes[2].name, "@snapshots");
        assert_eq!(layout.subvolumes[2].mountpoint, "/.snapshots");
        assert_eq!(layout.subvolumes[3].name, "@swap");
        assert_eq!(layout.subvolumes[3].mountpoint, "/swap");
        assert_eq!(layout.compress, "zstd:1");
    }

    #[test]
    fn btrfs_new_defaults() {
        let btrfs = Btrfs::new();
        assert_eq!(btrfs.compress, "zstd:1");
        assert_eq!(btrfs.mount_options, vec!["ssd", "noatime"]);
    }

    #[test]
    fn btrfs_with_compress() {
        let btrfs = Btrfs::new().with_compress("lzo");
        assert_eq!(btrfs.compress, "lzo");
    }
}

/// Trait for filesystem implementations
pub trait Filesystem: Send + Sync {
    /// Filesystem name (e.g., "btrfs", "ext4", "xfs")
    fn name(&self) -> &str;

    /// Format a device with this filesystem
    fn format(&self, device: &Path, label: &str) -> Result<()>;

    /// Mount the filesystem
    fn mount(&self, device: &Path, target: &Path, options: &MountOptions) -> Result<()>;

    /// Unmount the filesystem
    fn unmount(&self, target: &Path) -> Result<()>;

    /// Check if this filesystem supports subvolumes/datasets
    fn supports_subvolumes(&self) -> bool {
        false
    }

    /// Create subvolumes/datasets (for btrfs/zfs)
    fn create_subvolumes(&self, _device: &Path, _subvolumes: &[Subvolume]) -> Result<()> {
        Ok(())
    }

    /// Mount with subvolume layout (for btrfs/zfs)
    fn mount_subvolumes(
        &self,
        device: &Path,
        subvolumes: &[Subvolume],
        target: &Path,
        options: &MountOptions,
    ) -> Result<()>;

    /// Create a snapshot (for btrfs/zfs)
    fn snapshot(&self, _source: &Path, _dest: &Path, _readonly: bool) -> Result<()> {
        anyhow::bail!("Snapshots not supported by this filesystem")
    }

    /// Check if this filesystem supports snapshots
    fn supports_snapshots(&self) -> bool {
        false
    }
}

/// Trait for disk encryption implementations
pub trait DiskEncryption: Send + Sync {
    /// Encryption type name (e.g., "luks2", "luks1")
    fn name(&self) -> &str;

    /// Format/encrypt a partition
    fn format(&self, partition: &Path, passphrase: &str) -> Result<()>;

    /// Open/unlock an encrypted partition
    fn open(&self, partition: &Path, name: &str, passphrase: &str) -> Result<PathBuf>;

    /// Close/lock an encrypted partition
    fn close(&self, name: &str) -> Result<()>;

    /// Get the UUID of an encrypted partition
    fn get_uuid(&self, partition: &Path) -> Result<String>;
}
