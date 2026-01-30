use super::Distro;
use crate::cmd;
use crate::distro::packages::PackageDatabase;
use crate::init::{InitSystem, OpenRC};
use crate::pkgmgr::{Apk, PackageManager};
use anyhow::{Context, Result};
use std::path::Path;

pub struct Alpine {
    repo: String,
    init_system: OpenRC,
    pkg_manager: Apk,
}

impl Default for Alpine {
    fn default() -> Self {
        Self {
            repo: "https://dl-cdn.alpinelinux.org/alpine/edge/main".into(),
            init_system: OpenRC::alpine(),
            pkg_manager: Apk::new(),
        }
    }
}

impl Distro for Alpine {
    fn name(&self) -> &str {
        "Alpine Linux"
    }

    fn pkg_manager(&self) -> &str {
        "apk"
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        PackageDatabase::global().map_for_distro(generic, "alpine")
    }

    fn map_service(&self, generic: &str) -> String {
        // Alpine uses simple service names
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

        let root_str = root.to_string_lossy();
        let mut args = vec!["add", "--root", &root_str, "--no-cache"];
        let mapped_refs: Vec<&str> = mapped.iter().map(|s| s.as_str()).collect();
        args.extend(mapped_refs);

        cmd::run("apk", args)
    }

    fn update_system(&self) -> Result<()> {
        cmd::run("apk", ["update"])?;
        cmd::run("apk", ["upgrade"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        let mut packages = vec!["alpine-base", "openrc"];

        if enable_networking {
            packages.push("dhcpcd");
        }

        self.install_packages(root, &packages)?;

        // Enable services via OpenRC
        if enable_networking {
            let service = self.map_service("dhcpcd");
            self.init_system.enable_service(root, &service)?;
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let packages = match seat_manager {
            "elogind" => vec!["elogind", "polkit", "xdg-utils"],
            _ => vec!["seatd", "polkit", "xdg-utils"],
        };

        self.install_packages(root, &packages)?;

        let service = self.map_service(seat_manager);
        self.init_system.enable_service(root, &service)
    }

    fn install_display_manager(
        &self,
        root: &Path,
        dm: &str,
        greeter: Option<&str>,
        _configure_pam_rundir: bool,
    ) -> Result<()> {
        let dm_packages: Vec<&str> = match dm {
            "greetd" => {
                let mut pkgs = vec!["greetd"];
                if let Some("tuigreet") = greeter {
                    pkgs.push("greetd-tuigreet");
                }
                pkgs
            }
            _ => return Ok(()),
        };

        if dm_packages.is_empty() {
            return Ok(());
        }

        self.install_packages(root, &dm_packages)?;

        let service = self.map_service(dm);
        self.init_system.enable_service(root, &service)
    }

    fn install_portals(&self, root: &Path, backends: &[&str]) -> Result<()> {
        let mut packages = vec!["xdg-desktop-portal"];

        for backend in backends {
            match *backend {
                "wlr" => packages.push("xdg-desktop-portal-wlr"),
                "gtk" => packages.push("xdg-desktop-portal-gtk"),
                _ => {}
            }
        }

        self.install_packages(root, &packages)
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        super::generate_fstab_from_findmnt(root)
    }

    fn package_manager(&self) -> &dyn PackageManager {
        &self.pkg_manager
    }

    fn install_kernel_hook(&self, target: &Path) -> Result<()> {
        use std::fs;

        // Alpine uses apk triggers for package hooks
        // We'll create a trigger that runs when kernel package is upgraded
        let trigger_dir = target.join("etc/apk/triggers");
        fs::create_dir_all(&trigger_dir)?;

        // Create trigger for linux-lts package
        let trigger_content = "#!/bin/sh\n# mkOS kernel hook for Alpine Linux\nexec /usr/local/bin/mkos-rebuild-uki\n";

        fs::write(trigger_dir.join("mkos-uki.trigger"), "linux-lts\n")?;

        let script_path = target.join("usr/local/sbin/mkos-uki.trigger");
        fs::write(&script_path, trigger_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }

        // Install the rebuild script
        crate::hooks::install_uki_rebuild_script(target)?;

        println!("✓ Installed kernel hook for Alpine Linux (apk trigger)");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpine() -> Alpine {
        Alpine::default()
    }

    #[test]
    fn map_package_base_system() {
        assert_eq!(
            alpine().map_package("base-system"),
            Some("alpine-base".into())
        );
    }

    #[test]
    fn map_package_linux_kernel() {
        assert_eq!(
            alpine().map_package("linux-kernel"),
            Some("linux-lts".into())
        );
    }

    #[test]
    fn map_package_nss_mdns_alpine_specific() {
        assert_eq!(
            alpine().map_package("nss-mdns"),
            Some("avahi-nss-mdns".into())
        );
    }

    #[test]
    fn map_package_mesa() {
        assert_eq!(
            alpine().map_package("mesa"),
            Some("mesa-dri-gallium".into())
        );
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(alpine().map_package("nonexistent"), None);
    }

    #[test]
    fn map_service_passes_through() {
        assert_eq!(alpine().map_service("dbus"), "dbus");
        assert_eq!(alpine().map_service("seatd"), "seatd");
    }

    #[test]
    fn distro_trait_name() {
        let a = alpine();
        assert_eq!(Distro::name(&a), "Alpine Linux");
    }
}
