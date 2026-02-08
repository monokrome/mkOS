/// Default mount target for installation
pub const MOUNT_TARGET: &str = "/mnt";

/// Directory name for btrfs snapshots
pub const SNAPSHOTS_DIR: &str = ".snapshots";

/// Temporary mount point for btrfs root operations
pub const TEMP_BTRFS_MOUNT: &str = "/tmp/mkos-btrfs-root";

/// Default LUKS device mapper name
pub const LUKS_MAPPER_NAME: &str = "system";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapper_name_is_system() {
        assert_eq!(LUKS_MAPPER_NAME, "system");
    }

    #[test]
    fn mapper_device_path() {
        let path = format!("/dev/mapper/{}", LUKS_MAPPER_NAME);
        assert_eq!(path, "/dev/mapper/system");
    }

    #[test]
    fn mount_target_is_mnt() {
        assert_eq!(MOUNT_TARGET, "/mnt");
    }

    #[test]
    fn snapshots_dir_is_dotted() {
        assert!(SNAPSHOTS_DIR.starts_with('.'));
    }
}
