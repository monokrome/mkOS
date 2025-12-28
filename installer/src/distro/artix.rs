use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, S6};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Artix {
    repo: String,
    package_map: HashMap<String, String>,
    service_map: HashMap<String, String>,
    init_system: S6,
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
        package_map.insert("pipewire-pulse".into(), "pipewire-pulse".into());
        package_map.insert("pipewire-alsa".into(), "pipewire-alsa".into());
        package_map.insert("pipewire-jack".into(), "pipewire-jack".into());

        // XDG Desktop Portals
        package_map.insert("xdg-desktop-portal".into(), "xdg-desktop-portal".into());
        package_map.insert(
            "xdg-desktop-portal-wlr".into(),
            "xdg-desktop-portal-wlr".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-gtk".into(),
            "xdg-desktop-portal-gtk".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-kde".into(),
            "xdg-desktop-portal-kde".into(),
        );

        // Fonts
        package_map.insert("font-hack".into(), "ttf-hack".into());
        package_map.insert("font-noto".into(), "noto-fonts".into());
        package_map.insert("font-noto-emoji".into(), "noto-fonts-emoji".into());

        // NVIDIA drivers
        package_map.insert("nvidia".into(), "nvidia".into());
        package_map.insert("nvidia-utils".into(), "nvidia-utils".into());
        package_map.insert("nvidia-prime".into(), "nvidia-prime".into());
        package_map.insert("lib32-nvidia-utils".into(), "lib32-nvidia-utils".into());

        // AMD drivers
        package_map.insert("vulkan-radeon".into(), "vulkan-radeon".into());
        package_map.insert("lib32-mesa".into(), "lib32-mesa".into());
        package_map.insert("lib32-vulkan-radeon".into(), "lib32-vulkan-radeon".into());

        // Network services
        package_map.insert("avahi".into(), "avahi".into());
        package_map.insert("nss-mdns".into(), "nss-mdns".into());
        package_map.insert("openssh".into(), "openssh".into());
        package_map.insert("openssh-s6".into(), "openssh-s6".into());
        package_map.insert("eternalterminal".into(), "eternalterminal".into());
        package_map.insert("nftables".into(), "nftables".into());

        // System services with s6 counterparts
        package_map.insert("seatd".into(), "seatd".into());
        package_map.insert("seatd-s6".into(), "seatd-s6".into());
        package_map.insert("dbus".into(), "dbus".into());
        package_map.insert("dbus-s6".into(), "dbus-s6".into());
        package_map.insert("polkit".into(), "polkit".into());

        // Service name mapping (generic -> Artix-specific)
        let mut service_map = HashMap::new();
        service_map.insert("dbus".into(), "dbus-srv".into());
        service_map.insert("seatd".into(), "seatd-srv".into());
        service_map.insert("elogind".into(), "elogind-srv".into());
        service_map.insert("avahi".into(), "avahi".into());
        service_map.insert("sshd".into(), "sshd".into());
        service_map.insert("etserver".into(), "etserver".into());
        service_map.insert("nftables".into(), "nftables".into());

        Self {
            repo: "https://mirrors.dotsrc.org/artix-linux/repos".into(),
            package_map,
            service_map,
            init_system: S6::artix(),
        }
    }
}

impl Artix {
    /// Configure pam_rundir in the display manager's PAM file
    fn configure_pam_rundir(&self, root: &Path, dm: &str) -> Result<()> {
        let pam_path = root.join("etc/pam.d").join(dm);

        if !pam_path.exists() {
            // PAM file doesn't exist yet, skip configuration
            return Ok(());
        }

        let content = std::fs::read_to_string(&pam_path)?;

        // Only add if not already present
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

    fn map_service(&self, generic: &str) -> String {
        self.service_map
            .get(generic)
            .cloned()
            .unwrap_or_else(|| generic.to_string())
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
            "dbus",
            "dbus-s6",
        ];

        if enable_networking {
            packages.push("dhcpcd");
            packages.push("dhcpcd-s6");
        }

        let root_str = root.to_string_lossy().to_string();
        let mut args = vec![root_str.as_str()];
        args.extend(packages);

        cmd::run("basestrap", args)?;

        // Enable essential services
        let dbus_service = self.map_service("dbus");
        self.init_system.enable_service(root, &dbus_service)?;

        if enable_networking {
            let dhcpcd_service = self.map_service("dhcpcd");
            self.init_system.enable_service(root, &dhcpcd_service)?;
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let (seat_packages, service_name): (Vec<&str>, &str) = match seat_manager {
            "elogind" => (vec!["elogind", "elogind-s6"], "elogind"),
            _ => (vec!["seatd", "seatd-s6", "pam_rundir"], "seatd"),
        };

        let mut packages = seat_packages;
        packages.extend(["polkit", "xdg-utils"]);

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["-S", "--noconfirm", "-r", &root_str];
        args.extend(packages);

        cmd::run("pacman", args)?;

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
        let root_str = root.to_string_lossy().to_string();

        let dm_packages: Vec<&str> = match dm {
            "greetd" => {
                let mut pkgs = vec!["greetd", "greetd-s6"];
                if let Some(g) = greeter {
                    match g {
                        "regreet" => pkgs.push("greetd-regreet"),
                        "tuigreet" => pkgs.push("greetd-tuigreet"),
                        "gtkgreet" => pkgs.push("greetd-gtkgreet"),
                        _ => {}
                    }
                }
                if greeter == Some("regreet") {
                    pkgs.push("cage");
                }
                pkgs
            }
            "ly" => vec!["ly", "ly-s6"],
            _ => return Ok(()),
        };

        if dm_packages.is_empty() {
            return Ok(());
        }

        let mut args: Vec<&str> = vec!["-S", "--noconfirm", "-r", &root_str];
        args.extend(dm_packages);

        cmd::run("pacman", args)?;

        // Configure pam_rundir for XDG_RUNTIME_DIR if using seatd
        if configure_pam_rundir {
            self.configure_pam_rundir(root, dm)?;
        }

        let service_name = self.map_service(dm);
        self.init_system.enable_service(root, &service_name)
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

        let root_str = root.to_string_lossy().to_string();
        let mut args: Vec<&str> = vec!["-S", "--noconfirm", "-r", &root_str];
        args.extend(packages);

        cmd::run("pacman", args)
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        cmd::run_output("fstabgen", ["-U", &root.to_string_lossy()])
            .context("Failed to generate fstab with fstabgen")
    }
}
