use crate::cmd;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SecureBootKeys {
    pub pk: KeyPair,
    pub kek: KeyPair,
    pub db: KeyPair,
}

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub key: String,
    pub cert: String,
}

pub fn generate_keys(output_dir: &Path) -> Result<SecureBootKeys> {
    fs::create_dir_all(output_dir)?;

    let guid = uuid::Uuid::new_v4().to_string();
    fs::write(output_dir.join("GUID.txt"), &guid)?;

    generate_key_pair(output_dir, "PK", "mkOS Platform Key", &guid)?;
    generate_key_pair(output_dir, "KEK", "mkOS Key Exchange Key", &guid)?;
    generate_key_pair(output_dir, "db", "mkOS Signature Database Key", &guid)?;

    Ok(SecureBootKeys {
        pk: KeyPair {
            key: output_dir.join("PK.key").to_string_lossy().into(),
            cert: output_dir.join("PK.crt").to_string_lossy().into(),
        },
        kek: KeyPair {
            key: output_dir.join("KEK.key").to_string_lossy().into(),
            cert: output_dir.join("KEK.crt").to_string_lossy().into(),
        },
        db: KeyPair {
            key: output_dir.join("db.key").to_string_lossy().into(),
            cert: output_dir.join("db.crt").to_string_lossy().into(),
        },
    })
}

fn generate_key_pair(dir: &Path, name: &str, cn: &str, guid: &str) -> Result<()> {
    let key_path = dir.join(format!("{}.key", name));
    let cert_path = dir.join(format!("{}.crt", name));
    let esl_path = dir.join(format!("{}.esl", name));
    let auth_path = dir.join(format!("{}.auth", name));

    let key_str = key_path.to_string_lossy();
    let cert_str = cert_path.to_string_lossy();
    let esl_str = esl_path.to_string_lossy();
    let auth_str = auth_path.to_string_lossy();

    // Generate key and self-signed certificate
    cmd::run(
        "openssl",
        [
            "req",
            "-newkey",
            "rsa:4096",
            "-nodes",
            "-keyout",
            &key_str,
            "-x509",
            "-sha256",
            "-days",
            "3650",
            "-subj",
            &format!("/CN={}/", cn),
            "-out",
            &cert_str,
        ],
    )
    .context(format!("Failed to generate {} key pair", name))?;

    // Convert to EFI signature list
    cmd::run("cert-to-efi-sig-list", ["-g", guid, &cert_str, &esl_str])
        .context(format!("Failed to create {} ESL", name))?;

    // Create signed update for enrollment
    let (sign_key, sign_cert) = if name == "PK" {
        (key_path.clone(), cert_path.clone())
    } else {
        (dir.join("PK.key"), dir.join("PK.crt"))
    };

    let sign_key_str = sign_key.to_string_lossy();
    let sign_cert_str = sign_cert.to_string_lossy();

    cmd::run(
        "sign-efi-sig-list",
        [
            "-g",
            guid,
            "-k",
            &sign_key_str,
            "-c",
            &sign_cert_str,
            name,
            &esl_str,
            &auth_str,
        ],
    )
    .context(format!("Failed to sign {} ESL", name))?;

    Ok(())
}

pub fn sign_efi_binary(binary: &Path, keys: &SecureBootKeys) -> Result<()> {
    let binary_str = binary.to_string_lossy();

    cmd::run(
        "sbsign",
        [
            "--key",
            &keys.db.key,
            "--cert",
            &keys.db.cert,
            "--output",
            &binary_str,
            &binary_str,
        ],
    )
    .context("Failed to sign EFI binary")
}

pub fn enroll_keys(efi_mount: &Path, keys_dir: &Path) -> Result<()> {
    let key_target = efi_mount.join("keys");
    fs::create_dir_all(&key_target)?;

    for name in ["PK", "KEK", "db"] {
        let src = keys_dir.join(format!("{}.auth", name));
        let dst = key_target.join(format!("{}.auth", name));
        fs::copy(&src, &dst)?;
    }

    Ok(())
}

/// Trait for Secure Boot signing tools
pub trait SecureBootTool {
    /// Check if this tool is available on the system
    fn is_available(&self) -> bool;

    /// Check if keys are set up for this tool
    fn has_keys(&self) -> bool;

    /// Sign an EFI binary
    fn sign_binary(&self, binary: &Path) -> Result<()>;

    /// Get the name of this tool
    fn name(&self) -> &'static str;
}

/// sbctl-based Secure Boot (modern, simple)
pub struct SbctlTool;

impl SecureBootTool for SbctlTool {
    fn is_available(&self) -> bool {
        which::which("sbctl").is_ok()
    }

    fn has_keys(&self) -> bool {
        Path::new("/usr/share/secureboot").is_dir()
    }

    fn sign_binary(&self, binary: &Path) -> Result<()> {
        let binary_str = binary.to_string_lossy();
        cmd::run("sbctl", ["sign", "--save", &binary_str])
            .context("Failed to sign binary with sbctl")
    }

    fn name(&self) -> &'static str {
        "sbctl"
    }
}

/// Manual signing with sbsign/efitools (traditional approach)
pub struct ManualTool {
    keys_dir: PathBuf,
}

impl ManualTool {
    pub fn new(keys_dir: PathBuf) -> Self {
        Self { keys_dir }
    }
}

impl SecureBootTool for ManualTool {
    fn is_available(&self) -> bool {
        which::which("sbsign").is_ok()
    }

    fn has_keys(&self) -> bool {
        let key_path = self.keys_dir.join("db.key");
        let cert_path = self.keys_dir.join("db.crt");
        key_path.exists() && cert_path.exists()
    }

    fn sign_binary(&self, binary: &Path) -> Result<()> {
        let binary_str = binary.to_string_lossy();
        let key_path = self.keys_dir.join("db.key");
        let cert_path = self.keys_dir.join("db.crt");

        if !key_path.exists() || !cert_path.exists() {
            bail!("Secure Boot keys not found in {}", self.keys_dir.display());
        }

        let key_str = key_path.to_string_lossy();
        let cert_str = cert_path.to_string_lossy();

        cmd::run(
            "sbsign",
            [
                "--key",
                &key_str,
                "--cert",
                &cert_str,
                "--output",
                &binary_str,
                &binary_str,
            ],
        )
        .context("Failed to sign binary with sbsign")
    }

    fn name(&self) -> &'static str {
        "sbsign"
    }
}

/// Detect and return the available Secure Boot tool
pub fn detect_tool() -> Option<Box<dyn SecureBootTool>> {
    let sbctl = SbctlTool;
    if sbctl.is_available() && sbctl.has_keys() {
        return Some(Box::new(sbctl));
    }

    let manual = ManualTool::new(PathBuf::from("/root/.secureboot-keys"));
    if manual.is_available() && manual.has_keys() {
        return Some(Box::new(manual));
    }

    None
}
