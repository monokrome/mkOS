mod btrfs;
mod luks;
pub mod snapshot;

use anyhow::Result;
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

impl MountOptions {
    pub fn to_string(&self) -> String {
        let mut opts = Vec::new();

        if let Some(ref comp) = self.compress {
            opts.push(format!("compress={}", comp));
        }
        if let Some(ref subvol) = self.subvolume {
            opts.push(format!("subvol={}", subvol));
        }
        opts.extend(self.extra.clone());

        opts.join(",")
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
