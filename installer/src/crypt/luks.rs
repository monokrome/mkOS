use super::DiskEncryption;
use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cmd;

/// LUKS encryption configuration
#[derive(Debug, Clone)]
pub struct LuksConfig {
    pub cipher: String,
    pub key_size: u32,
    pub hash: String,
    pub iter_time: u32,
    pub label: String,
}

impl Default for LuksConfig {
    fn default() -> Self {
        Self {
            cipher: "aes-xts-plain64".into(),
            key_size: 512,
            hash: "sha512".into(),
            iter_time: 5000,
            label: "cryptroot".into(),
        }
    }
}

/// LUKS2 disk encryption implementation with Argon2id
#[derive(Debug, Clone, Default)]
pub struct Luks2 {
    pub config: LuksConfig,
}

impl Luks2 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: LuksConfig) -> Self {
        Self { config }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.config.label = label.into();
        self
    }
}

impl DiskEncryption for Luks2 {
    fn name(&self) -> &str {
        "luks2"
    }

    fn format(&self, partition: &Path, passphrase: &str) -> Result<()> {
        let partition_str = partition.to_string_lossy();

        cmd::run_with_stdin(
            "cryptsetup",
            [
                "luksFormat",
                "--type",
                "luks2",
                "--cipher",
                &self.config.cipher,
                "--key-size",
                &self.config.key_size.to_string(),
                "--hash",
                &self.config.hash,
                "--iter-time",
                &self.config.iter_time.to_string(),
                "--label",
                &self.config.label,
                "--pbkdf",
                "argon2id",
                "--batch-mode",
                "--key-file=-",
                &partition_str,
            ],
            passphrase.as_bytes(),
        )
    }

    fn open(&self, partition: &Path, name: &str, passphrase: &str) -> Result<PathBuf> {
        let partition_str = partition.to_string_lossy();

        cmd::run_with_stdin(
            "cryptsetup",
            [
                "open",
                "--type",
                "luks2",
                "--key-file=-",
                &partition_str,
                name,
            ],
            passphrase.as_bytes(),
        )?;

        Ok(PathBuf::from(format!("/dev/mapper/{}", name)))
    }

    fn close(&self, name: &str) -> Result<()> {
        cmd::run("cryptsetup", ["close", name])
    }

    fn get_uuid(&self, partition: &Path) -> Result<String> {
        cmd::run_output("cryptsetup", ["luksUUID", &partition.to_string_lossy()])
    }
}

// Legacy function wrappers for backwards compatibility during migration
pub fn format_luks(partition: &Path, passphrase: &str, config: &LuksConfig) -> Result<()> {
    Luks2::with_config(config.clone()).format(partition, passphrase)
}

pub fn open_luks(partition: &Path, name: &str, passphrase: &str) -> Result<PathBuf> {
    Luks2::new().open(partition, name, passphrase)
}

pub fn close_luks(name: &str) -> Result<()> {
    Luks2::new().close(name)
}

pub fn get_uuid(partition: &Path) -> Result<String> {
    Luks2::new().get_uuid(partition)
}
