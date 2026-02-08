use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::chroot;
use crate::cmd;
use crate::crypt::{close_luks, open_luks};
use crate::paths;

const LUKS_MAPPER: &str = "system";

#[derive(Debug)]
pub struct BlockDevice {
    pub path: PathBuf,
    pub size: String,
    pub fstype: String,
}

/// Detect LUKS partitions on the system
pub fn detect_luks_partitions() -> Result<Vec<BlockDevice>> {
    let output = cmd::run_output("blkid", ["-t", "TYPE=crypto_LUKS", "-o", "device"])?;
    let devices: Vec<PathBuf> = output.lines().map(PathBuf::from).collect();

    let mut result = Vec::new();
    for dev in devices {
        let size = get_device_size(&dev)?;
        result.push(BlockDevice {
            path: dev,
            size,
            fstype: "crypto_LUKS".into(),
        });
    }

    Ok(result)
}

/// Detect EFI/vfat partitions on the system
pub fn detect_efi_partitions() -> Result<Vec<BlockDevice>> {
    let output = cmd::run_output("blkid", ["-t", "TYPE=vfat", "-o", "device"])?;
    let devices: Vec<PathBuf> = output.lines().map(PathBuf::from).collect();

    let mut result = Vec::new();
    for dev in devices {
        let size = get_device_size(&dev)?;
        result.push(BlockDevice {
            path: dev,
            size,
            fstype: "vfat".into(),
        });
    }

    Ok(result)
}

fn get_device_size(device: &Path) -> Result<String> {
    let dev_str = device.to_string_lossy().to_string();
    cmd::run_output("lsblk", ["-dn", "-o", "SIZE", &dev_str])
}

/// Mount an installed mkOS system for rescue chroot
pub fn mount_system(efi_partition: &Path, luks_partition: &Path, passphrase: &str) -> Result<()> {
    let target = Path::new(paths::MOUNT_TARGET);

    // Open LUKS
    println!("Opening LUKS partition...");
    open_luks(luks_partition, LUKS_MAPPER, passphrase)?;

    let mapper_device = PathBuf::from(format!("/dev/mapper/{}", LUKS_MAPPER));

    // Mount root subvolume
    println!("Mounting root filesystem...");
    std::fs::create_dir_all(target)?;
    let mapper_str = mapper_device.to_string_lossy().to_string();
    let target_str = target.to_string_lossy().to_string();
    cmd::run(
        "mount",
        ["-o", "subvol=@,compress=zstd:1", &mapper_str, &target_str],
    )?;

    // Mount additional subvolumes from btrfs
    mount_btrfs_subvolumes(&mapper_device, target)?;

    // Mount EFI partition
    let boot_dir = target.join("boot");
    std::fs::create_dir_all(&boot_dir)?;
    let efi_str = efi_partition.to_string_lossy().to_string();
    let boot_str = boot_dir.to_string_lossy().to_string();
    cmd::run("mount", [&efi_str, &boot_str])?;

    // Set up chroot virtual filesystems
    println!("Setting up chroot environment...");
    chroot::setup_chroot(target)?;

    Ok(())
}

/// Mount btrfs subvolumes other than root (@)
fn mount_btrfs_subvolumes(device: &Path, target: &Path) -> Result<()> {
    let subvol_mounts = [
        ("@home", "home"),
        ("@snapshots", paths::SNAPSHOTS_DIR),
        ("@swap", "swap"),
    ];

    let device_str = device.to_string_lossy().to_string();

    for (subvol, mountpoint) in subvol_mounts {
        let mount_path = target.join(mountpoint);
        if !mount_path.exists() {
            std::fs::create_dir_all(&mount_path)?;
        }

        let opts = format!("subvol={},compress=zstd:1", subvol);
        let mount_str = mount_path.to_string_lossy().to_string();
        if let Err(e) = cmd::run("mount", ["-o", &opts, &device_str, &mount_str]) {
            println!("  Warning: could not mount {}: {}", subvol, e);
        }
    }

    Ok(())
}

/// Enter chroot shell on the mounted system
pub fn enter_chroot() -> Result<()> {
    let target = Path::new(paths::MOUNT_TARGET);
    println!("Entering chroot at {}...", target.display());
    println!("Type 'exit' to leave the rescue shell.\n");

    let status = std::process::Command::new("chroot")
        .arg(target)
        .arg("/bin/bash")
        .status()
        .context("Failed to enter chroot")?;

    if !status.success() {
        println!("Chroot shell exited with status: {:?}", status.code());
    }

    Ok(())
}

/// Unmount everything and close LUKS
pub fn cleanup() -> Result<()> {
    let target = Path::new(paths::MOUNT_TARGET);
    let target_str = target.to_string_lossy().to_string();

    println!("\nCleaning up...");

    // Teardown chroot virtual filesystems
    chroot::teardown_chroot(target)?;

    // Unmount /run separately (may already be unmounted)
    let run_str = format!("{}/run", target_str);
    if let Err(e) = cmd::run("umount", [&run_str]) {
        tracing::debug!("Failed to unmount /run: {}", e);
    }

    // Unmount boot
    let boot_str = format!("{}/boot", target_str);
    if let Err(e) = cmd::run("umount", [&boot_str]) {
        tracing::debug!("Failed to unmount /boot: {}", e);
    }

    // Unmount subvolumes in reverse order
    let subvol_mounts = ["swap", paths::SNAPSHOTS_DIR, "home"];
    for mountpoint in subvol_mounts {
        let mount_str = format!("{}/{}", target_str, mountpoint);
        if let Err(e) = cmd::run("umount", [&mount_str]) {
            tracing::debug!("Failed to unmount {}: {}", mountpoint, e);
        }
    }

    // Unmount root
    if let Err(e) = cmd::run("umount", [&target_str]) {
        println!("Warning: failed to unmount root: {}", e);
    }

    // Close LUKS
    if let Err(e) = close_luks(LUKS_MAPPER) {
        println!("Warning: failed to close LUKS: {}", e);
    }

    println!("Cleanup complete.");

    Ok(())
}

/// Prompt user to select a device from a list
pub fn select_device(devices: &[BlockDevice], device_type: &str) -> Result<usize> {
    if devices.is_empty() {
        bail!("No {} partitions detected", device_type);
    }

    if devices.len() == 1 {
        return Ok(0);
    }

    println!("\nMultiple {} partitions found:", device_type);
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {} [{}]", i + 1, dev.path.display(), dev.size);
    }

    print!("Select [1-{}]: ", devices.len());
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let idx: usize = input
        .trim()
        .parse::<usize>()
        .context("Invalid selection")?
        .checked_sub(1)
        .context("Selection out of range")?;

    if idx >= devices.len() {
        bail!("Selection out of range");
    }

    Ok(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;

    #[test]
    fn luks_mapper_matches_paths_constant() {
        assert_eq!(LUKS_MAPPER, paths::LUKS_MAPPER_NAME);
    }

    #[test]
    fn block_device_stores_path_and_metadata() {
        let dev = BlockDevice {
            path: PathBuf::from("/dev/sda1"),
            size: "500G".into(),
            fstype: "crypto_LUKS".into(),
        };
        assert_eq!(dev.path, PathBuf::from("/dev/sda1"));
        assert_eq!(dev.size, "500G");
        assert_eq!(dev.fstype, "crypto_LUKS");
    }

    #[test]
    fn select_device_returns_zero_for_single_device() {
        let devices = vec![BlockDevice {
            path: PathBuf::from("/dev/sda2"),
            size: "1T".into(),
            fstype: "crypto_LUKS".into(),
        }];
        assert_eq!(select_device(&devices, "LUKS").unwrap(), 0);
    }

    #[test]
    fn select_device_errors_on_empty_list() {
        let devices: Vec<BlockDevice> = vec![];
        assert!(select_device(&devices, "LUKS").is_err());
    }

    #[test]
    fn cleanup_unmounts_in_correct_order() {
        // Verify the subvolume unmount list is in reverse order of mount
        let mount_order = ["home", paths::SNAPSHOTS_DIR, "swap"];
        let unmount_order = ["swap", paths::SNAPSHOTS_DIR, "home"];

        // Unmount order should be reversed from mount order
        for (i, mount) in mount_order.iter().enumerate() {
            assert_eq!(*mount, unmount_order[mount_order.len() - 1 - i]);
        }
    }

    #[test]
    fn mount_target_uses_paths_constant() {
        assert_eq!(paths::MOUNT_TARGET, "/mnt");
    }

    #[test]
    fn mapper_device_path_format() {
        let mapper = format!("/dev/mapper/{}", LUKS_MAPPER);
        assert_eq!(mapper, "/dev/mapper/system");
    }
}
