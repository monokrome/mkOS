use super::Distro;
use crate::cmd;
use crate::distro::packages::PackageDatabase;
use crate::init::{InitSystem, S6};
use crate::pkgmgr::{PackageManager, Xbps};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct Void {
    repo: String,
    init_system: S6,
    pkg_manager: Xbps,
}

impl Default for Void {
    fn default() -> Self {
        let repo = "https://repo-default.voidlinux.org/current".to_string();
        Self {
            pkg_manager: Xbps::new(&repo),
            repo,
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

    fn configure_pam_rundir(&self, root: &Path, dm: &str) -> Result<()> {
        super::configure_pam_rundir(root, dm)
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
        PackageDatabase::global().map_for_distro(generic, "void")
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
            anyhow::bail!("genfstab failed with exit code {:?}", output.status.code());
        }

        String::from_utf8(output.stdout).context("Invalid UTF-8 in fstab output")
    }

    fn package_manager(&self) -> &dyn PackageManager {
        &self.pkg_manager
    }

    fn install_kernel_hook(&self, target: &Path) -> Result<()> {
        use std::fs;

        // Void uses xbps triggers - create a kernel install hook
        // The trigger runs when linux package is installed/updated
        let trigger_dir = target.join("etc/kernel.d/post-install");
        fs::create_dir_all(&trigger_dir)?;

        let trigger_content = r#"#!/bin/sh
# mkOS kernel hook for Void Linux
# Called by xbps when kernel is installed/updated

KERNEL_VERSION="$1"

if [ -z "$KERNEL_VERSION" ]; then
    echo "ERROR: Kernel version not provided"
    exit 1
fi

# Run the UKI rebuild script
exec /usr/local/bin/mkos-rebuild-uki
"#;

        fs::write(trigger_dir.join("50-mkos-uki"), trigger_content)?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(trigger_dir.join("50-mkos-uki"))?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(trigger_dir.join("50-mkos-uki"), perms)?;
        }

        // Install the rebuild script (same as Artix)
        crate::hooks::install_uki_rebuild_script(target)?;

        println!("✓ Installed kernel hook for Void Linux");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn void() -> Void {
        Void::default()
    }

    #[test]
    fn map_package_xwayland() {
        assert_eq!(
            void().map_package("xwayland"),
            Some("xorg-server-xwayland".into())
        );
    }

    #[test]
    fn map_package_mesa() {
        assert_eq!(void().map_package("mesa"), Some("mesa-dri".into()));
    }

    #[test]
    fn map_package_font_hack() {
        assert_eq!(
            void().map_package("font-hack"),
            Some("font-hack-ttf".into())
        );
    }

    #[test]
    fn map_package_font_noto() {
        assert_eq!(
            void().map_package("font-noto"),
            Some("noto-fonts-ttf".into())
        );
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(void().map_package("nonexistent"), None);
    }

    #[test]
    fn map_service_passes_through() {
        assert_eq!(void().map_service("dbus"), "dbus");
        assert_eq!(void().map_service("seatd"), "seatd");
        assert_eq!(void().map_service("anything"), "anything");
    }

    #[test]
    fn distro_trait_name() {
        let v = void();
        assert_eq!(Distro::name(&v), "Void Linux");
    }
}
