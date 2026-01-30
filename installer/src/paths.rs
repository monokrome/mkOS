/// Default mount target for installation
pub const MOUNT_TARGET: &str = "/mnt";

/// Directory name for btrfs snapshots
pub const SNAPSHOTS_DIR: &str = ".snapshots";

/// Temporary mount point for btrfs root operations
pub const TEMP_BTRFS_MOUNT: &str = "/tmp/mkos-btrfs-root";

/// Default LUKS device mapper name
pub const LUKS_MAPPER_NAME: &str = "cryptroot";
