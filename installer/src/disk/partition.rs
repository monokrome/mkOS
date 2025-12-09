use anyhow::Result;
use std::path::{Path, PathBuf};

use super::PartitionLayout;
use crate::cmd;

#[derive(Debug, Clone)]
pub struct CreatedPartitions {
    pub efi: PathBuf,
    pub luks: PathBuf,
}

pub fn create_partitions(device: &Path, layout: &PartitionLayout) -> Result<CreatedPartitions> {
    let device_str = device.to_string_lossy();

    // Use sfdisk for scriptable partitioning (available on base ISO, unlike parted)
    // Format: start, size, type, bootable
    let script = format!("label: gpt\n,{}M,U,*\n,,L\n", layout.efi_size_mb);

    cmd::run_with_stdin("sfdisk", [&*device_str], script.as_bytes())?;

    // Wait for kernel to re-read partition table
    // partprobe isn't available on base Artix ISO, so we just sleep
    std::thread::sleep(std::time::Duration::from_secs(2));

    let partitions = detect_partition_names(device)?;
    Ok(partitions)
}

fn detect_partition_names(device: &Path) -> Result<CreatedPartitions> {
    detect_partitions(device)
}

pub fn detect_partitions(device: &Path) -> Result<CreatedPartitions> {
    let device_str = device.to_string_lossy();

    // Handle nvme vs sata naming (nvme0n1p1 vs sda1)
    // Also handle virtio (vda1)
    let (efi, luks) = if device_str.contains("nvme") || device_str.contains("mmcblk") {
        (
            PathBuf::from(format!("{}p1", device_str)),
            PathBuf::from(format!("{}p2", device_str)),
        )
    } else {
        (
            PathBuf::from(format!("{}1", device_str)),
            PathBuf::from(format!("{}2", device_str)),
        )
    };

    Ok(CreatedPartitions { efi, luks })
}

pub fn format_efi(partition: &Path) -> Result<()> {
    cmd::run(
        "mkfs.fat",
        ["-F", "32", "-n", "MKOS_EFI", &partition.to_string_lossy()],
    )
}
