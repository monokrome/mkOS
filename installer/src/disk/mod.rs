mod partition;

pub use partition::*;

use crate::cmd;
use anyhow::Result;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BlockDevice {
    pub path: String,
    pub size_bytes: u64,
    pub model: Option<String>,
    pub removable: bool,
}

#[derive(Debug, Clone)]
pub struct PartitionLayout {
    pub efi_size_mb: u64,
    pub root_size_mb: Option<u64>, // None = use remaining space
    pub home_size_mb: Option<u64>, // None = use remaining after root
}

impl Default for PartitionLayout {
    fn default() -> Self {
        Self {
            efi_size_mb: 1024,  // 1GB for UKI
            root_size_mb: None, // Will be calculated
            home_size_mb: None, // Rest goes to home
        }
    }
}

pub fn list_block_devices() -> Result<Vec<BlockDevice>> {
    let output = Command::new("lsblk")
        .args(["-b", "-d", "-n", "-o", "PATH,SIZE,MODEL,RM"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let path = parts[0].to_string();

            // Skip loop devices and optical drives
            if path.contains("loop") || path.contains("sr") {
                continue;
            }

            let size_bytes = parts[1].parse().unwrap_or(0);
            let removable = parts.last().map(|s| *s == "1").unwrap_or(false);
            let model = if parts.len() > 3 {
                Some(parts[2..parts.len() - 1].join(" "))
            } else {
                None
            };

            devices.push(BlockDevice {
                path,
                size_bytes,
                model,
                removable,
            });
        }
    }

    Ok(devices)
}

/// Validate that a path is a valid block device suitable for installation
pub fn validate_device(device: &Path) -> Result<()> {
    // Check device exists
    if !device.exists() {
        anyhow::bail!("Device {} does not exist", device.display());
    }

    // Check it's a block device
    let metadata = std::fs::metadata(device)?;
    if !metadata.file_type().is_block_device() {
        anyhow::bail!(
            "{} is not a block device (might be a partition or regular file)",
            device.display()
        );
    }

    // Check it's a whole disk, not a partition (path shouldn't end with a number)
    let path_str = device.to_string_lossy();
    if path_str
        .chars()
        .last()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        // Could be a partition like /dev/sda1 or /dev/nvme0n1p1
        // But /dev/nvme0n1 is valid (ends in digit but is a whole disk)
        // Check if it looks like a partition suffix
        if path_str.contains('p')
            && path_str
                .rfind('p')
                .map(|i| path_str[i + 1..].chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        {
            anyhow::bail!(
                "{} appears to be a partition, not a whole disk",
                device.display()
            );
        }
        // For traditional devices like /dev/sda1
        if !path_str.contains("nvme") && !path_str.contains("mmcblk") {
            anyhow::bail!(
                "{} appears to be a partition, not a whole disk",
                device.display()
            );
        }
    }

    Ok(())
}

pub fn wipe_device(device: &Path) -> Result<()> {
    validate_device(device)?;
    cmd::run("wipefs", ["--all", "--force", &device.to_string_lossy()])
}

pub fn create_gpt(device: &Path) -> Result<()> {
    cmd::run(
        "parted",
        ["-s", &device.to_string_lossy(), "mklabel", "gpt"],
    )
}
