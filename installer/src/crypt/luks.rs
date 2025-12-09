use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cmd;

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

pub fn format_luks(partition: &Path, passphrase: &str, config: &LuksConfig) -> Result<()> {
    let partition_str = partition.to_string_lossy();

    cmd::run_with_stdin(
        "cryptsetup",
        [
            "luksFormat",
            "--type",
            "luks2",
            "--cipher",
            &config.cipher,
            "--key-size",
            &config.key_size.to_string(),
            "--hash",
            &config.hash,
            "--iter-time",
            &config.iter_time.to_string(),
            "--label",
            &config.label,
            "--pbkdf",
            "argon2id",
            "--batch-mode",
            "--key-file=-",
            &partition_str,
        ],
        passphrase.as_bytes(),
    )
}

pub fn open_luks(partition: &Path, name: &str, passphrase: &str) -> Result<PathBuf> {
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

pub fn close_luks(name: &str) -> Result<()> {
    cmd::run("cryptsetup", ["close", name])
}

pub fn get_uuid(partition: &Path) -> Result<String> {
    cmd::run_output("cryptsetup", ["luksUUID", &partition.to_string_lossy()])
}
