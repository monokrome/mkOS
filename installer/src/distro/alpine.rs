use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, OpenRC};
use crate::pkgmgr::PackageManager;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct Alpine {
    repo: String,
    package_map: HashMap<String, String>,
    init_system: OpenRC,
}

impl Default for Alpine {
    fn default() -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Alpine package names
        package_map.insert("base-system".into(), "alpine-base".into());
        package_map.insert("linux-kernel".into(), "linux-lts".into());
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

        // Init systems
        package_map.insert("openrc".into(), "openrc".into());
        package_map.insert("s6".into(), "s6".into());
        package_map.insert("s6-rc".into(), "s6-rc".into());
        package_map.insert("s6-linux-init".into(), "s6-linux-init".into());
        package_map.insert("runit".into(), "runit".into());

        // Wayland
        package_map.insert("wayland".into(), "wayland".into());
        package_map.insert("wayland-protocols".into(), "wayland-protocols".into());
        package_map.insert("wlroots".into(), "wlroots".into());
        package_map.insert("xwayland".into(), "xwayland".into());
        package_map.insert("libinput".into(), "libinput".into());
        package_map.insert("mesa".into(), "mesa-dri-gallium".into());

        // Display managers
        package_map.insert("greetd".into(), "greetd".into());
        package_map.insert("greetd-tuigreet".into(), "greetd-tuigreet".into());
        package_map.insert("kitty".into(), "kitty".into());

        // Audio
        package_map.insert("pipewire".into(), "pipewire".into());
        package_map.insert("wireplumber".into(), "wireplumber".into());
        package_map.insert("pipewire-pulse".into(), "pipewire-pulse".into());
        package_map.insert("pipewire-alsa".into(), "pipewire-alsa".into());
        package_map.insert("pipewire-jack".into(), "pipewire-jack".into());

        // Fonts
        package_map.insert("font-hack".into(), "font-hack".into());
        package_map.insert("font-noto".into(), "font-noto".into());
        package_map.insert("font-noto-emoji".into(), "font-noto-emoji".into());

        // XDG portals
        package_map.insert("xdg-desktop-portal".into(), "xdg-desktop-portal".into());
        package_map.insert(
            "xdg-desktop-portal-wlr".into(),
            "xdg-desktop-portal-wlr".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-gtk".into(),
            "xdg-desktop-portal-gtk".into(),
        );
        package_map.insert("xdg-utils".into(), "xdg-utils".into());

        // Network services
        package_map.insert("avahi".into(), "avahi".into());
        package_map.insert("nss-mdns".into(), "avahi-nss-mdns".into()); // Different on Alpine!
        package_map.insert("openssh".into(), "openssh".into());
        package_map.insert("nftables".into(), "nftables".into());

        // System services
        package_map.insert("dbus".into(), "dbus".into());
        package_map.insert("polkit".into(), "polkit".into());
        package_map.insert("seatd".into(), "seatd".into());
        package_map.insert("elogind".into(), "elogind".into());

        Self {
            repo: "https://dl-cdn.alpinelinux.org/alpine/edge/main".into(),
            package_map,
            init_system: OpenRC::alpine(),
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
        self.package_map.get(generic).cloned()
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
        todo!("Alpine PackageManager trait implementation")
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
