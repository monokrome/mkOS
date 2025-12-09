use super::Distro;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct Void {
    repo: String,
    package_map: HashMap<String, String>,
}

impl Default for Void {
    fn default() -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Void-specific names
        package_map.insert("base-system".into(), "base-system".into());
        package_map.insert("linux-kernel".into(), "linux".into());
        package_map.insert("linux-firmware".into(), "linux-firmware".into());
        package_map.insert("intel-ucode".into(), "intel-ucode".into());
        package_map.insert("amd-ucode".into(), "amd-ucode".into());
        package_map.insert("dracut".into(), "dracut".into());
        package_map.insert("efibootmgr".into(), "efibootmgr".into());
        package_map.insert("sbsigntools".into(), "sbsigntools".into());
        package_map.insert("cryptsetup".into(), "cryptsetup".into());
        package_map.insert("btrfs-progs".into(), "btrfs-progs".into());
        package_map.insert("dhcpcd".into(), "dhcpcd".into());
        package_map.insert("iwd".into(), "iwd".into());
        package_map.insert("s6".into(), "s6".into());
        package_map.insert("s6-rc".into(), "s6-rc".into());
        package_map.insert("s6-linux-init".into(), "s6-linux-init".into());
        package_map.insert("wayland".into(), "wayland".into());
        package_map.insert("wayland-protocols".into(), "wayland-protocols".into());
        package_map.insert("wlroots".into(), "wlroots".into());
        package_map.insert("xwayland".into(), "xorg-server-xwayland".into());
        package_map.insert("libinput".into(), "libinput".into());
        package_map.insert("mesa".into(), "mesa-dri".into());
        package_map.insert("greetd".into(), "greetd".into());
        package_map.insert("greetd-tuigreet".into(), "greetd-tuigreet".into());
        package_map.insert("kitty".into(), "kitty".into());
        package_map.insert("rofi-wayland".into(), "rofi-wayland".into());
        package_map.insert("pipewire".into(), "pipewire".into());
        package_map.insert("wireplumber".into(), "wireplumber".into());
        package_map.insert("font-hack".into(), "font-hack-ttf".into());
        package_map.insert("font-noto".into(), "noto-fonts-ttf".into());
        package_map.insert("font-noto-emoji".into(), "noto-fonts-emoji".into());

        Self {
            repo: "https://repo-default.voidlinux.org/current".into(),
            package_map,
        }
    }
}

impl Distro for Void {
    fn name(&self) -> &str {
        "Void Linux"
    }

    fn pkg_manager(&self) -> &str {
        "xbps-install"
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        self.package_map.get(generic).cloned()
    }

    fn install_packages(&self, root: &Path, packages: &[&str]) -> Result<()> {
        let mapped: Vec<String> = packages
            .iter()
            .filter_map(|p| self.map_package(p))
            .collect();

        // Nothing to install if all packages failed to map or list is empty
        if mapped.is_empty() {
            return Ok(());
        }

        Command::new("xbps-install")
            .args(["-Sy", "-R", &self.repo, "-r"])
            .arg(root)
            .args(&mapped)
            .status()
            .context("Failed to install packages")?;

        Ok(())
    }

    fn update_system(&self) -> Result<()> {
        Command::new("xbps-install")
            .args(["-Syu"])
            .status()
            .context("Failed to update system")?;

        Ok(())
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        let mut packages = vec!["base-system"];

        if enable_networking {
            packages.push("dhcpcd");
        }

        Command::new("xbps-install")
            .args(["-Sy", "-R", &self.repo, "-r"])
            .arg(root)
            .args(&packages)
            .status()
            .context("Failed to bootstrap Void")?;

        if enable_networking {
            self.enable_service(root, "dhcpcd")?;
        }

        Ok(())
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        // Void with s6: create symlink in service directory
        let service_src = root.join("etc/s6/sv").join(service);
        let service_dst = root.join("etc/s6/rc/default").join(service);

        // Verify the service exists before trying to enable it
        if !service_src.exists() {
            anyhow::bail!(
                "Service '{}' not found at {}. The service package may not be installed.",
                service,
                service_src.display()
            );
        }

        std::fs::create_dir_all(service_dst.parent().unwrap())?;

        // Create symlink if it doesn't already exist
        if service_dst.exists() {
            // Already enabled, skip
            return Ok(());
        }

        std::os::unix::fs::symlink(&service_src, &service_dst)
            .context(format!("Failed to enable service '{}'", service))?;

        Ok(())
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        // Void uses genfstab from void-install-scripts
        let output = Command::new("genfstab")
            .args(["-U"])
            .arg(root)
            .output()
            .context("Failed to run genfstab")?;

        String::from_utf8(output.stdout).context("Invalid UTF-8 in fstab output")
    }
}
