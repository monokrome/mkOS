use super::Distro;
use crate::cmd;
use crate::distro::packages::PackageDatabase;
use crate::init::{InitSystem, SysVinit};
use crate::pkgmgr::{Apt, PackageManager};
use anyhow::{Context, Result};
use std::path::Path;

pub struct Devuan {
    repo: String,
    init_system: SysVinit,
    pkg_manager: Apt,
}

impl Default for Devuan {
    fn default() -> Self {
        Self {
            repo: "https://deb.devuan.org/merged".into(),
            init_system: SysVinit::devuan(),
            pkg_manager: Apt::new(),
        }
    }
}

impl Distro for Devuan {
    fn name(&self) -> &str {
        "Devuan GNU+Linux"
    }

    fn pkg_manager(&self) -> &str {
        "apt"
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        PackageDatabase::global().map_for_distro(generic, "devuan")
    }

    fn map_service(&self, generic: &str) -> String {
        // Devuan uses simple service names
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

        // Use apt with --root option via chroot
        let mut args = vec!["chroot", &root_str, "apt-get", "install", "-y"];
        let mapped_refs: Vec<&str> = mapped.iter().map(|s| s.as_str()).collect();
        args.extend(mapped_refs);

        cmd::run("sudo", args)
    }

    fn update_system(&self) -> Result<()> {
        cmd::run("apt-get", ["update"])?;
        cmd::run("apt-get", ["upgrade", "-y"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        // Devuan bootstrap typically done via debootstrap
        let mut packages = vec!["systemd-shim", "linux-image-amd64"];

        if enable_networking {
            packages.push("dhcpcd5");
        }

        self.install_packages(root, &packages)?;

        // Enable networking service
        if enable_networking {
            // Devuan uses /etc/network/interfaces for networking
            let interfaces_content = r#"auto lo
iface lo inet loopback

auto eth0
iface eth0 inet dhcp
"#;
            std::fs::write(root.join("etc/network/interfaces"), interfaces_content)?;
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let packages = match seat_manager {
            "elogind" => vec!["elogind", "policykit-1", "xdg-utils"],
            _ => vec!["seatd", "policykit-1", "xdg-utils"],
        };

        self.install_packages(root, &packages)?;

        // SysVinit services are managed via update-rc.d
        let root_str = root.to_string_lossy();
        cmd::run(
            "chroot",
            [&root_str, "update-rc.d", seat_manager, "defaults"],
        )
    }

    fn install_display_manager(
        &self,
        root: &Path,
        _dm: &str,
        _greeter: Option<&str>,
        _configure_pam_rundir: bool,
    ) -> Result<()> {
        // Most display managers not available in Devuan by default
        println!("Note: Display managers may need manual installation on Devuan");
        let _ = root;
        Ok(())
    }

    fn install_portals(&self, root: &Path, backends: &[&str]) -> Result<()> {
        let mut packages = vec!["xdg-desktop-portal"];

        for backend in backends {
            if *backend == "gtk" {
                packages.push("xdg-desktop-portal-gtk");
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

        // Devuan/Debian use APT hooks in /etc/apt/apt.conf.d/
        // and kernel hooks in /etc/kernel/postinst.d/
        let hook_dir = target.join("etc/kernel/postinst.d");
        fs::create_dir_all(&hook_dir)?;

        let hook_content = r#"#!/bin/sh
# mkOS kernel hook for Devuan
# Called by kernel package postinst

set -e

VERSION="$1"

if [ -z "$VERSION" ]; then
    echo "ERROR: Kernel version not provided"
    exit 1
fi

# Only rebuild for the current kernel
if [ -d "/lib/modules/$VERSION" ]; then
    echo "mkOS: Rebuilding UKI for kernel $VERSION..."
    /usr/local/bin/mkos-rebuild-uki
fi

exit 0
"#;

        fs::write(hook_dir.join("zz-mkos-uki"), hook_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(hook_dir.join("zz-mkos-uki"))?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(hook_dir.join("zz-mkos-uki"), perms)?;
        }

        // Install the rebuild script
        crate::hooks::install_uki_rebuild_script(target)?;

        println!("✓ Installed kernel hook for Devuan (kernel postinst.d)");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devuan() -> Devuan {
        Devuan::default()
    }

    #[test]
    fn map_package_linux_kernel() {
        assert_eq!(
            devuan().map_package("linux-kernel"),
            Some("linux-image-amd64".into())
        );
    }

    #[test]
    fn map_package_nss_mdns() {
        assert_eq!(devuan().map_package("nss-mdns"), Some("libnss-mdns".into()));
    }

    #[test]
    fn map_package_polkit() {
        assert_eq!(devuan().map_package("polkit"), Some("policykit-1".into()));
    }

    #[test]
    fn map_package_avahi() {
        assert_eq!(devuan().map_package("avahi"), Some("avahi-daemon".into()));
    }

    #[test]
    fn map_package_openssh() {
        assert_eq!(
            devuan().map_package("openssh"),
            Some("openssh-server".into())
        );
    }

    #[test]
    fn map_package_font_noto_emoji() {
        assert_eq!(
            devuan().map_package("font-noto-emoji"),
            Some("fonts-noto-color-emoji".into())
        );
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(devuan().map_package("nonexistent"), None);
    }

    #[test]
    fn map_service_passes_through() {
        assert_eq!(devuan().map_service("dbus"), "dbus");
        assert_eq!(devuan().map_service("seatd"), "seatd");
    }

    #[test]
    fn distro_trait_name() {
        let d = devuan();
        assert_eq!(Distro::name(&d), "Devuan GNU+Linux");
    }
}
