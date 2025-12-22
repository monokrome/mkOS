use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, S6};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct Void {
    repo: String,
    package_map: HashMap<String, String>,
    init_system: S6,
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
            init_system: S6::void(),
        }
    }
}

impl Void {
    fn xbps_install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        let root_str = root.to_string_lossy();
        let mut args = vec!["-Sy", "-R", &self.repo, "-r", &root_str];
        args.extend(packages);
        cmd::run("xbps-install", args)
    }

    /// Configure pam_rundir in the display manager's PAM file
    fn configure_pam_rundir(&self, root: &Path, dm: &str) -> Result<()> {
        let pam_path = root.join("etc/pam.d").join(dm);

        if !pam_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&pam_path)?;

        if !content.contains("pam_rundir.so") {
            let new_content = format!(
                "{}\nsession    optional   pam_rundir.so\n",
                content.trim_end()
            );
            std::fs::write(&pam_path, new_content)?;
        }

        Ok(())
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

    fn map_service(&self, generic: &str) -> String {
        // Void uses generic service names (no mapping needed)
        generic.to_string()
    }

    fn init_system(&self) -> &dyn InitSystem {
        &self.init_system
    }

    fn install_packages(&self, root: &Path, packages: &[&str]) -> Result<()> {
        let mapped: Vec<String> = packages
            .iter()
            .filter_map(|p| self.map_package(p))
            .collect();

        if mapped.is_empty() {
            return Ok(());
        }

        let pkg_refs: Vec<&str> = mapped.iter().map(|s| s.as_str()).collect();
        self.xbps_install(root, &pkg_refs)
    }

    fn update_system(&self) -> Result<()> {
        cmd::run("xbps-install", ["-Syu"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        let mut packages = vec!["base-system"];

        if enable_networking {
            packages.push("dhcpcd");
        }

        self.xbps_install(root, &packages)?;

        if enable_networking {
            let service = self.map_service("dhcpcd");
            self.init_system.enable_service(root, &service)?;
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let (seat_packages, service_name): (Vec<&str>, &str) = match seat_manager {
            "elogind" => (vec!["elogind"], "elogind"),
            _ => (vec!["seatd", "pam_rundir"], "seatd"),
        };

        let mut packages = seat_packages;
        packages.extend(["polkit", "xdg-utils"]);
        self.xbps_install(root, &packages)?;

        let service = self.map_service(service_name);
        self.init_system.enable_service(root, &service)
    }

    fn install_display_manager(
        &self,
        root: &Path,
        dm: &str,
        greeter: Option<&str>,
        configure_pam_rundir: bool,
    ) -> Result<()> {
        let dm_packages: Vec<&str> = match dm {
            "greetd" => {
                let mut pkgs = vec!["greetd"];
                if let Some(g) = greeter {
                    match g {
                        "tuigreet" => pkgs.push("greetd-tuigreet"),
                        "gtkgreet" => pkgs.push("greetd-gtkgreet"),
                        _ => {}
                    }
                }
                pkgs
            }
            "ly" => vec!["ly"],
            _ => return Ok(()),
        };

        if dm_packages.is_empty() {
            return Ok(());
        }

        self.xbps_install(root, &dm_packages)?;

        // Configure pam_rundir for XDG_RUNTIME_DIR if using seatd
        if configure_pam_rundir {
            self.configure_pam_rundir(root, dm)?;
        }

        let service = self.map_service(dm);
        self.init_system.enable_service(root, &service)
    }

    fn install_portals(&self, root: &Path, backends: &[&str]) -> Result<()> {
        // Core portal package (always installed)
        let mut packages = vec!["xdg-desktop-portal"];

        // Add backend-specific packages
        for backend in backends {
            match *backend {
                "wlr" => packages.push("xdg-desktop-portal-wlr"),
                "gtk" => packages.push("xdg-desktop-portal-gtk"),
                "kde" => packages.push("xdg-desktop-portal-kde"),
                _ => {}
            }
        }

        self.xbps_install(root, &packages)
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        let output = Command::new("genfstab")
            .args(["-U"])
            .arg(root)
            .output()
            .context("Failed to run genfstab")?;

        if !output.status.success() {
            anyhow::bail!(
                "genfstab failed with exit code {:?}",
                output.status.code()
            );
        }

        String::from_utf8(output.stdout).context("Invalid UTF-8 in fstab output")
    }
}
