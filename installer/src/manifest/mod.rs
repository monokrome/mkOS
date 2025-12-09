mod schema;

pub use schema::*;

use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Represents a loaded manifest bundle (manifest + optional files from tar)
#[derive(Debug)]
pub struct ManifestBundle {
    pub manifest: Manifest,
    pub files_dir: Option<PathBuf>,
}

/// Input source for manifest loading
#[derive(Debug, Clone)]
pub enum ManifestSource {
    File(PathBuf),
    Url(String),
    Stdin,
    Interactive,
}

impl ManifestSource {
    /// Parse from command line argument
    pub fn from_arg(arg: Option<&str>) -> Self {
        match arg {
            None => Self::Interactive,
            Some("-") => Self::Stdin,
            Some(s) if s.starts_with("http://") || s.starts_with("https://") => {
                Self::Url(s.to_string())
            }
            Some(s) => Self::File(PathBuf::from(s)),
        }
    }
}

/// Load manifest from any supported source
pub fn load(source: &ManifestSource) -> Result<ManifestBundle> {
    match source {
        ManifestSource::File(path) => load_from_file(path),
        ManifestSource::Url(url) => load_from_url(url),
        ManifestSource::Stdin => load_from_stdin(),
        ManifestSource::Interactive => Ok(ManifestBundle {
            manifest: Manifest::default(),
            files_dir: None,
        }),
    }
}

/// Load manifest from a file (YAML, JSON, or tar)
fn load_from_file(path: &Path) -> Result<ManifestBundle> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "tar" | "tgz" | "tar.gz" => load_from_tar_file(path),
        "yaml" | "yml" => {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
            let manifest = parse_yaml(&content)?;
            Ok(ManifestBundle {
                manifest,
                files_dir: path.parent().map(|p| p.to_path_buf()),
            })
        }
        "json" => {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
            let manifest = parse_json(&content)?;
            Ok(ManifestBundle {
                manifest,
                files_dir: path.parent().map(|p| p.to_path_buf()),
            })
        }
        _ => {
            // Try to detect format from content
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
            let manifest = parse_auto(&content)?;
            Ok(ManifestBundle {
                manifest,
                files_dir: path.parent().map(|p| p.to_path_buf()),
            })
        }
    }
}

/// Load manifest from a tar archive
fn load_from_tar_file(path: &Path) -> Result<ManifestBundle> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = File::open(path)
        .with_context(|| format!("Failed to open tar archive: {}", path.display()))?;

    // Check if gzipped
    let is_gzip = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "tgz" || e == "gz")
        .unwrap_or(false);

    // Extract to temp directory
    let extract_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let extract_path = extract_dir.path().to_path_buf();

    if is_gzip {
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(&extract_path)
            .context("Failed to extract tar.gz archive")?;
    } else {
        let mut archive = Archive::new(file);
        archive
            .unpack(&extract_path)
            .context("Failed to extract tar archive")?;
    }

    // Find manifest file in extracted contents
    let manifest_path = find_manifest_in_dir(&extract_path)?;
    let content = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read manifest from tar: {}",
            manifest_path.display()
        )
    })?;

    let manifest = parse_auto(&content)?;

    // Keep the temp dir alive (will be cleaned up on process exit)
    let files_dir = extract_dir.keep();

    Ok(ManifestBundle {
        manifest,
        files_dir: Some(files_dir),
    })
}

/// Find manifest.yaml or manifest.json in a directory
fn find_manifest_in_dir(dir: &Path) -> Result<PathBuf> {
    let yaml_path = dir.join("manifest.yaml");
    if yaml_path.exists() {
        return Ok(yaml_path);
    }

    let yml_path = dir.join("manifest.yml");
    if yml_path.exists() {
        return Ok(yml_path);
    }

    let json_path = dir.join("manifest.json");
    if json_path.exists() {
        return Ok(json_path);
    }

    bail!(
        "No manifest.yaml or manifest.json found in archive root. \
         Expected one of: manifest.yaml, manifest.yml, manifest.json"
    );
}

/// Load manifest from URL (YAML, JSON, or tar)
fn load_from_url(url: &str) -> Result<ManifestBundle> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("Failed to fetch manifest from URL: {}", url))?;

    let content_type = response.header("content-type").unwrap_or("").to_lowercase();

    // Check if it's a tar file based on content-type or URL
    let is_tar = content_type.contains("application/x-tar")
        || content_type.contains("application/gzip")
        || url.ends_with(".tar")
        || url.ends_with(".tgz")
        || url.ends_with(".tar.gz");

    if is_tar {
        load_tar_from_url(url, response)
    } else {
        let content = response
            .into_string()
            .context("Failed to read response body")?;
        let manifest = parse_auto(&content)?;
        Ok(ManifestBundle {
            manifest,
            files_dir: None,
        })
    }
}

/// Load tar archive from URL
fn load_tar_from_url(url: &str, response: ureq::Response) -> Result<ManifestBundle> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let is_gzip = url.ends_with(".tgz") || url.ends_with(".tar.gz");

    // Download to temp file
    let mut temp_file = tempfile::NamedTempFile::new().context("Failed to create temp file")?;
    let mut reader = response.into_reader();
    io::copy(&mut reader, &mut temp_file).context("Failed to download tar archive")?;

    // Reopen for reading
    let file = File::open(temp_file.path()).context("Failed to open downloaded tar")?;

    // Extract
    let extract_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let extract_path = extract_dir.path().to_path_buf();

    if is_gzip {
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(&extract_path)
            .context("Failed to extract tar.gz archive")?;
    } else {
        let mut archive = Archive::new(file);
        archive
            .unpack(&extract_path)
            .context("Failed to extract tar archive")?;
    }

    let manifest_path = find_manifest_in_dir(&extract_path)?;
    let content = fs::read_to_string(&manifest_path)?;
    let manifest = parse_auto(&content)?;

    let files_dir = extract_dir.keep();

    Ok(ManifestBundle {
        manifest,
        files_dir: Some(files_dir),
    })
}

/// Load manifest from stdin
fn load_from_stdin() -> Result<ManifestBundle> {
    let mut content = String::new();
    io::stdin()
        .read_to_string(&mut content)
        .context("Failed to read manifest from stdin")?;

    let manifest = parse_auto(&content)?;
    Ok(ManifestBundle {
        manifest,
        files_dir: None,
    })
}

/// Parse YAML content
fn parse_yaml(content: &str) -> Result<Manifest> {
    serde_yaml::from_str(content).context("Failed to parse YAML manifest")
}

/// Parse JSON content
fn parse_json(content: &str) -> Result<Manifest> {
    serde_json::from_str(content).context("Failed to parse JSON manifest")
}

/// Auto-detect format and parse
fn parse_auto(content: &str) -> Result<Manifest> {
    let trimmed = content.trim();

    // JSON starts with { or [
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json(content)
    } else {
        // Assume YAML (which is a superset of JSON anyway)
        parse_yaml(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_yaml_manifest() {
        let yaml = r#"
system:
  hostname: test
"#;
        let manifest = parse_yaml(yaml).unwrap();
        assert_eq!(manifest.system.hostname, "test");
        assert_eq!(manifest.system.timezone, "UTC");
        assert_eq!(manifest.distro, "artix");
    }

    #[test]
    fn test_minimal_json_manifest() {
        let json = r#"{"system": {"hostname": "test"}}"#;
        let manifest = parse_json(json).unwrap();
        assert_eq!(manifest.system.hostname, "test");
    }

    #[test]
    fn test_full_manifest() {
        let yaml = r#"
system:
  hostname: workstation
  timezone: America/Denver
  locale: en_US.UTF-8
  keymap: us

disk:
  device: /dev/sda
  encryption: true
  filesystem: btrfs
  subvolumes:
    - name: "@"
      mountpoint: /
    - name: "@home"
      mountpoint: /home

packages:
  base:
    - s6-base
    - linux
  desktop:
    - dwl

services:
  enable:
    - dhcpcd

users:
  polar:
    shell: /bin/zsh
    groups:
      - wheel
      - video

files:
  - path: /etc/motd
    content: "Welcome to mkOS"
    mode: "0644"

distro: artix
"#;

        let manifest = parse_yaml(yaml).unwrap();
        assert_eq!(manifest.system.hostname, "workstation");
        assert_eq!(manifest.disk.device, Some("/dev/sda".into()));
        assert!(manifest.disk.encryption);
        assert_eq!(manifest.packages.get("base").unwrap().len(), 2);
        assert!(manifest.users.contains_key("polar"));
        assert_eq!(manifest.files.len(), 1);
    }

    #[test]
    fn test_auto_detect_json() {
        let json = r#"{"system": {"hostname": "test"}}"#;
        let manifest = parse_auto(json).unwrap();
        assert_eq!(manifest.system.hostname, "test");
    }

    #[test]
    fn test_auto_detect_yaml() {
        let yaml = "system:\n  hostname: test\n";
        let manifest = parse_auto(yaml).unwrap();
        assert_eq!(manifest.system.hostname, "test");
    }

    #[test]
    fn test_default_subvolumes() {
        let yaml = "system:\n  hostname: test\n";
        let manifest = parse_yaml(yaml).unwrap();
        assert_eq!(manifest.disk.subvolumes.len(), 3);
        assert_eq!(manifest.disk.subvolumes[0].name, "@");
        assert_eq!(manifest.disk.subvolumes[1].name, "@home");
        assert_eq!(manifest.disk.subvolumes[2].name, "@snapshots");
    }

    #[test]
    fn test_source_from_arg() {
        assert!(matches!(
            ManifestSource::from_arg(None),
            ManifestSource::Interactive
        ));
        assert!(matches!(
            ManifestSource::from_arg(Some("-")),
            ManifestSource::Stdin
        ));
        assert!(matches!(
            ManifestSource::from_arg(Some("https://example.com/manifest.yaml")),
            ManifestSource::Url(_)
        ));
        assert!(matches!(
            ManifestSource::from_arg(Some("/path/to/manifest.yaml")),
            ManifestSource::File(_)
        ));
    }
}
