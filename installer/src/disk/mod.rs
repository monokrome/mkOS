mod partition;

pub use partition::*;

use crate::cmd;
use anyhow::Result;
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

pub fn wipe_device(device: &Path) -> Result<()> {
    cmd::run("wipefs", ["--all", "--force", &device.to_string_lossy()])
}

pub fn create_gpt(device: &Path) -> Result<()> {
    cmd::run(
        "parted",
        ["-s", &device.to_string_lossy(), "mklabel", "gpt"],
    )
}
