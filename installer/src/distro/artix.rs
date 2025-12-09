use super::Distro;
use crate::cmd;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Artix {
    repo: String,
    package_map: HashMap<String, String>,
}

impl Default for Artix {
    fn default() -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Artix/Arch-specific names
        package_map.insert("base-system".into(), "base".into());
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

        // s6 packages (Artix-specific)
        package_map.insert("s6".into(), "s6".into());
        package_map.insert("s6-rc".into(), "s6-rc".into());
        package_map.insert("s6-linux-init".into(), "s6-linux-init".into());
        package_map.insert("s6-base".into(), "s6-base".into());

        // Wayland
        package_map.insert("wayland".into(), "wayland".into());
        package_map.insert("wayland-protocols".into(), "wayland-protocols".into());
        package_map.insert("wlroots".into(), "wlroots".into());
        package_map.insert("xwayland".into(), "xorg-xwayland".into());
        package_map.insert("libinput".into(), "libinput".into());
        package_map.insert("mesa".into(), "mesa".into());

        // Desktop
        package_map.insert("greetd".into(), "greetd".into());
        package_map.insert("greetd-tuigreet".into(), "greetd-tuigreet".into());
        package_map.insert("kitty".into(), "kitty".into());
        package_map.insert("rofi-wayland".into(), "rofi-wayland".into());
        package_map.insert("pipewire".into(), "pipewire".into());
        package_map.insert("wireplumber".into(), "wireplumber".into());

        // Fonts
        package_map.insert("font-hack".into(), "ttf-hack".into());
        package_map.insert("font-noto".into(), "noto-fonts".into());
        package_map.insert("font-noto-emoji".into(), "noto-fonts-emoji".into());

        Self {
            repo: "https://mirrors.dotsrc.org/artix-linux/repos".into(),
            package_map,
        }
    }
}

impl Distro for Artix {
    fn name(&self) -> &str {
        "Artix Linux"
    }

    fn pkg_manager(&self) -> &str {
        "pacman"
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

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["-S", "--noconfirm", "-r", &root_str];
        let mapped_refs: Vec<&str> = mapped.iter().map(|s| s.as_str()).collect();
        args.extend(mapped_refs);

        cmd::run("pacman", args)
    }

    fn update_system(&self) -> Result<()> {
        cmd::run("pacman", ["-Syu", "--noconfirm"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        // Use basestrap (Artix's pacstrap equivalent)
        // elogind-s6 required to resolve dependency conflict with dinit
        let mut packages = vec![
            "base",
            "s6-base",
            "elogind-s6",
            "linux",
            "linux-firmware",
            "cryptsetup",
            "btrfs-progs",
            "efibootmgr",
            "dracut",
        ];

        if enable_networking {
            packages.push("dhcpcd");
            packages.push("dhcpcd-s6");
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args = vec![root_str.as_str()];
        args.extend(packages);

        cmd::run("basestrap", args)?;

        if enable_networking {
            self.enable_service(root, "dhcpcd")?;
        }

        Ok(())
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        // Artix s6: services are in /etc/s6/sv, enabled by symlinking to /etc/s6/adminsv/default
        let service_src = root.join("etc/s6/sv").join(service);
        let service_dst = root.join("etc/s6/adminsv/default").join(service);

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
        cmd::run_output("fstabgen", ["-U", &root.to_string_lossy()])
            .context("Failed to generate fstab with fstabgen")
    }
}
