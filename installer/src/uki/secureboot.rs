use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SecureBootKeys {
    pub pk: KeyPair,
    pub kek: KeyPair,
    pub db: KeyPair,
}

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub key: String,  // Path to private key
    pub cert: String, // Path to certificate
}

pub fn generate_keys(output_dir: &Path) -> Result<SecureBootKeys> {
    fs::create_dir_all(output_dir)?;

    let guid = uuid::Uuid::new_v4().to_string();
    fs::write(output_dir.join("GUID.txt"), &guid)?;

    // Generate Platform Key
    generate_key_pair(output_dir, "PK", "mkOS Platform Key", &guid)?;

    // Generate Key Exchange Key
    generate_key_pair(output_dir, "KEK", "mkOS Key Exchange Key", &guid)?;

    // Generate Signature Database Key
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

    // Generate key and self-signed certificate
    Command::new("openssl")
        .args(["req", "-newkey", "rsa:4096", "-nodes", "-keyout"])
        .arg(&key_path)
        .args(["-x509", "-sha256", "-days", "3650", "-subj"])
        .arg(format!("/CN={}/", cn))
        .arg("-out")
        .arg(&cert_path)
        .status()
        .context(format!("Failed to generate {} key pair", name))?;

    // Convert to EFI signature list
    Command::new("cert-to-efi-sig-list")
        .args(["-g", guid])
        .arg(&cert_path)
        .arg(&esl_path)
        .status()
        .context(format!("Failed to create {} ESL", name))?;

    // Create signed update for enrollment
    let sign_key = if name == "PK" {
        &key_path
    } else {
        &dir.join("PK.key")
    };
    let sign_cert = if name == "PK" {
        &cert_path
    } else {
        &dir.join("PK.crt")
    };

    Command::new("sign-efi-sig-list")
        .args(["-g", guid, "-k"])
        .arg(sign_key)
        .arg("-c")
        .arg(sign_cert)
        .arg(name)
        .arg(&esl_path)
        .arg(&auth_path)
        .status()
        .context(format!("Failed to sign {} ESL", name))?;

    Ok(())
}

pub fn sign_efi_binary(binary: &Path, keys: &SecureBootKeys) -> Result<()> {
    Command::new("sbsign")
        .args(["--key", &keys.db.key])
        .args(["--cert", &keys.db.cert])
        .args(["--output"])
        .arg(binary)
        .arg(binary)
        .status()
        .context("Failed to sign EFI binary")?;

    Ok(())
}

pub fn enroll_keys(efi_mount: &Path, keys_dir: &Path) -> Result<()> {
    let key_target = efi_mount.join("keys");
    fs::create_dir_all(&key_target)?;

    // Copy .auth files for manual enrollment
    for name in ["PK", "KEK", "db"] {
        let src = keys_dir.join(format!("{}.auth", name));
        let dst = key_target.join(format!("{}.auth", name));
        fs::copy(&src, &dst)?;
    }

    Ok(())
}
