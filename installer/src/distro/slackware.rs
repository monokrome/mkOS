use super::Distro;
use crate::cmd;
use crate::distro::packages::PackageDatabase;
use crate::init::{InitSystem, SysVinit};
use crate::pkgmgr::{PackageManager, SlaptGet};
use anyhow::Result;
use std::path::Path;

pub struct Slackware {
    repo: String,
    init_system: SysVinit,
    pkg_manager: SlaptGet,
}

impl Default for Slackware {
    fn default() -> Self {
        Self::new()
    }
}

impl Slackware {
    pub fn new() -> Self {
        Self {
            repo: "https://mirrors.slackware.com/slackware/slackware64-current".into(),
            init_system: SysVinit::slackware(),
            pkg_manager: SlaptGet::new(),
        }
    }

    /// Install packages using slapt-get
    fn slaptget_install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        let root_str = root.to_string_lossy();

        // slapt-get supports --root option
        let mut args = vec!["--root", &root_str, "--install", "--yes"];
        args.extend(packages);

        cmd::run("slapt-get", args)
    }
}

impl Distro for Slackware {
    fn name(&self) -> &str {
        "Slackware Linux"
    }

    fn pkg_manager(&self) -> &str {
        "slapt-get"
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        PackageDatabase::global().map_for_distro(generic, "slackware")
    }

    fn map_service(&self, generic: &str) -> String {
        // Slackware uses simple service names, no mapping needed
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

        self.slaptget_install(root, &pkg_refs)
    }

    fn update_system(&self) -> Result<()> {
        cmd::run("slapt-get", ["--update"])?;
        cmd::run("slapt-get", ["--upgrade", "--yes"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        // Slackware bootstrap is more manual - typically done via installpkg
        let mut packages = vec!["aaa_base", "kernel-generic", "cryptsetup", "btrfs-progs"];

        if enable_networking {
            packages.push("dhcpcd");
        }

        self.install_packages(root, &packages)?;

        // Slackware init scripts are in /etc/rc.d/
        // For dhcpcd, we need to make the rc.inet1 script executable
        if enable_networking {
            let rc_inet1 = root.join("etc/rc.d/rc.inet1");
            if rc_inet1.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&rc_inet1)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&rc_inet1, perms)?;
                }
            }
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let packages = match seat_manager {
            "elogind" => vec!["elogind", "polkit", "xdg-utils"],
            _ => vec!["seatd", "polkit", "xdg-utils"],
        };

        self.install_packages(root, &packages)?;

        // Make service scripts executable
        let service_name = match seat_manager {
            "elogind" => "rc.elogind",
            _ => "rc.seatd",
        };

        let rc_script = root.join("etc/rc.d").join(service_name);
        if rc_script.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&rc_script)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&rc_script, perms)?;
            }
        }

        Ok(())
    }

    fn install_display_manager(
        &self,
        _root: &Path,
        dm: &str,
        greeter: Option<&str>,
        _configure_pam_rundir: bool,
    ) -> Result<()> {
        // Most display managers aren't in Slackware repos
        // Would need SlackBuilds
        println!("Note: {} may need to be installed from SlackBuilds.org", dm);

        if let Some(g) = greeter {
            println!(
                "Note: {} greeter may need to be installed from SlackBuilds.org",
                g
            );
        }

        Ok(())
    }

    fn install_portals(&self, root: &Path, backends: &[&str]) -> Result<()> {
        let mut packages = vec!["xdg-desktop-portal"];

        for backend in backends {
            // wlr and kde not in official repos
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

        // Slackware uses /etc/rc.d/rc.local for custom startup
        // We'll create a hook that runs on kernel upgrade
        let hook_dir = target.join("etc/kernel.d/post-install");
        fs::create_dir_all(&hook_dir)?;

        let hook_content = r#"#!/bin/sh
# mkOS kernel hook for Slackware
# This should be called after kernel installation

KERNEL_VERSION="$1"

if [ -z "$KERNEL_VERSION" ]; then
    echo "ERROR: Kernel version not provided"
    exit 1
fi

# Run the UKI rebuild script
exec /usr/local/bin/mkos-rebuild-uki
"#;

        fs::write(hook_dir.join("50-mkos-uki"), hook_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(hook_dir.join("50-mkos-uki"))?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(hook_dir.join("50-mkos-uki"), perms)?;
        }

        // Install the rebuild script
        crate::hooks::install_uki_rebuild_script(target)?;

        println!("✓ Installed kernel hook for Slackware");
        println!("  Note: You'll need to manually run this after kernel upgrades");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slackware() -> Slackware {
        Slackware::default()
    }

    #[test]
    fn map_package_base_system() {
        assert_eq!(
            slackware().map_package("base-system"),
            Some("aaa_base".into())
        );
    }

    #[test]
    fn map_package_linux_kernel() {
        assert_eq!(
            slackware().map_package("linux-kernel"),
            Some("kernel-generic".into())
        );
    }

    #[test]
    fn map_package_font_hack() {
        assert_eq!(
            slackware().map_package("font-hack"),
            Some("hack-fonts-ttf".into())
        );
    }

    #[test]
    fn map_package_empty_string_returns_none() {
        // Packages mapped to empty string (unavailable) should return None
        assert_eq!(slackware().map_package("intel-ucode"), None);
        assert_eq!(slackware().map_package("amd-ucode"), None);
        assert_eq!(slackware().map_package("s6"), None);
        assert_eq!(slackware().map_package("sbsigntools"), None);
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(slackware().map_package("nonexistent"), None);
    }

    #[test]
    fn map_service_passes_through() {
        assert_eq!(slackware().map_service("dbus"), "dbus");
        assert_eq!(slackware().map_service("seatd"), "seatd");
    }

    #[test]
    fn distro_trait_name() {
        let s = slackware();
        assert_eq!(Distro::name(&s), "Slackware Linux");
    }
}
