use crate::cmd;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

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
