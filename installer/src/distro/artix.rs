use super::Distro;
use crate::cmd;
use crate::distro::packages::PackageDatabase;
use crate::init::{InitSystem, S6};
use crate::pkgmgr::{PackageManager, Pacman};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Artix {
    repo: String,
    service_map: HashMap<String, String>,
    init_system: S6,
    pkg_manager: Pacman,
}

impl Default for Artix {
    fn default() -> Self {
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
            service_map,
            init_system: S6::artix(),
            pkg_manager: Pacman::new(),
        }
    }
}

impl Artix {
    fn configure_pam_rundir(&self, root: &Path, dm: &str) -> Result<()> {
        super::configure_pam_rundir(root, dm)
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
        PackageDatabase::global().map_for_distro(generic, "artix")
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

    fn package_manager(&self) -> &dyn PackageManager {
        &self.pkg_manager
    }

    fn install_kernel_hook(&self, target: &Path) -> Result<()> {
        // Install pacman hook and rebuild script
        crate::hooks::install_pacman_hooks(target)?;
        crate::hooks::install_uki_rebuild_script(target)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artix() -> Artix {
        Artix::default()
    }

    #[test]
    fn map_package_base_system() {
        assert_eq!(artix().map_package("base-system"), Some("base".into()));
    }

    #[test]
    fn map_package_xwayland() {
        assert_eq!(
            artix().map_package("xwayland"),
            Some("xorg-xwayland".into())
        );
    }

    #[test]
    fn map_package_font_hack() {
        assert_eq!(artix().map_package("font-hack"), Some("ttf-hack".into()));
    }

    #[test]
    fn map_package_nss_mdns() {
        assert_eq!(artix().map_package("nss-mdns"), Some("nss-mdns".into()));
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(artix().map_package("nonexistent-package"), None);
    }

    #[test]
    fn map_service_dbus() {
        assert_eq!(artix().map_service("dbus"), "dbus-srv");
    }

    #[test]
    fn map_service_seatd() {
        assert_eq!(artix().map_service("seatd"), "seatd-srv");
    }

    #[test]
    fn map_service_elogind() {
        assert_eq!(artix().map_service("elogind"), "elogind-srv");
    }

    #[test]
    fn map_service_unknown_passes_through() {
        assert_eq!(artix().map_service("unknown"), "unknown");
    }

    #[test]
    fn distro_trait_name() {
        let a = artix();
        assert_eq!(Distro::name(&a), "Artix Linux");
    }
}
