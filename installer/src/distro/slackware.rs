use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, SysVinit};
use crate::pkgmgr::PackageManager;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Package manager variant for Slackware
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlackwarePkgManager {
    /// Official slackpkg (no dependency resolution)
    Slackpkg,
    /// slapt-get (APT-like with dependency resolution)
    SlaptGet,
}

pub struct Slackware {
    repo: String,
    package_map: HashMap<String, String>,
    init_system: SysVinit,
    pkg_manager: SlackwarePkgManager,
}

impl Default for Slackware {
    fn default() -> Self {
        Self::with_pkg_manager(SlackwarePkgManager::SlaptGet)
    }
}

impl Slackware {
    /// Create Slackware configuration with specific package manager
    pub fn with_pkg_manager(pkg_manager: SlackwarePkgManager) -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Slackware package names
        package_map.insert("base-system".into(), "aaa_base".into());
        package_map.insert("linux-kernel".into(), "kernel-generic".into());
        package_map.insert("linux-firmware".into(), "kernel-firmware".into());
        package_map.insert("intel-ucode".into(), "".into()); // Not in repos
        package_map.insert("amd-ucode".into(), "".into()); // Not in repos
        package_map.insert("dracut".into(), "dracut".into());
        package_map.insert("efibootmgr".into(), "efibootmgr".into());
        package_map.insert("sbsigntools".into(), "".into()); // Not in official repos
        package_map.insert("cryptsetup".into(), "cryptsetup".into());
        package_map.insert("btrfs-progs".into(), "btrfs-progs".into());
        package_map.insert("dhcpcd".into(), "dhcpcd".into());
        package_map.insert("iwd".into(), "iwd".into());

        // Init systems (Slackware uses SysVinit, but these are for user services)
        package_map.insert("s6".into(), "".into()); // Not in repos
        package_map.insert("s6-rc".into(), "".into());
        package_map.insert("s6-linux-init".into(), "".into());
        package_map.insert("runit".into(), "".into()); // Not in official repos

        // Wayland
        package_map.insert("wayland".into(), "wayland".into());
        package_map.insert("wayland-protocols".into(), "wayland-protocols".into());
        package_map.insert("wlroots".into(), "wlroots".into());
        package_map.insert("xwayland".into(), "xorg-server-xwayland".into());
        package_map.insert("libinput".into(), "libinput".into());
        package_map.insert("mesa".into(), "mesa".into());

        // Display managers
        package_map.insert("greetd".into(), "".into()); // Not in repos
        package_map.insert("greetd-tuigreet".into(), "".into());
        package_map.insert("kitty".into(), "kitty".into());
        package_map.insert("rofi-wayland".into(), "".into()); // rofi exists, not -wayland variant

        // Audio
        package_map.insert("pipewire".into(), "pipewire".into());
        package_map.insert("wireplumber".into(), "wireplumber".into());
        package_map.insert("pipewire-pulse".into(), "".into()); // Included in pipewire
        package_map.insert("pipewire-alsa".into(), "".into()); // Included in pipewire
        package_map.insert("pipewire-jack".into(), "".into()); // Included in pipewire

        // Fonts
        package_map.insert("font-hack".into(), "hack-fonts-ttf".into());
        package_map.insert("font-noto".into(), "noto-fonts-ttf".into());
        package_map.insert("font-noto-emoji".into(), "noto-emoji".into());

        // XDG portals
        package_map.insert("xdg-desktop-portal".into(), "xdg-desktop-portal".into());
        package_map.insert("xdg-desktop-portal-wlr".into(), "".into());
        package_map.insert("xdg-desktop-portal-gtk".into(), "xdg-desktop-portal-gtk".into());
        package_map.insert("xdg-utils".into(), "xdg-utils".into());

        // GPU drivers
        package_map.insert("nvidia".into(), "nvidia-driver".into());
        package_map.insert("nvidia-utils".into(), "".into()); // Included in nvidia-driver
        package_map.insert("vulkan-radeon".into(), "mesa".into()); // Included in mesa
        package_map.insert("lib32-mesa".into(), "".into()); // No multilib in Slackware64

        // Network services
        package_map.insert("avahi".into(), "avahi".into());
        package_map.insert("nss-mdns".into(), "".into()); // Included in avahi
        package_map.insert("openssh".into(), "openssh".into());
        package_map.insert("nftables".into(), "nftables".into());

        // System services
        package_map.insert("dbus".into(), "dbus".into());
        package_map.insert("polkit".into(), "polkit".into());
        package_map.insert("seatd".into(), "seatd".into());
        package_map.insert("elogind".into(), "elogind".into());

        Self {
            repo: "https://mirrors.slackware.com/slackware/slackware64-current".into(),
            package_map,
            init_system: SysVinit::slackware(),
            pkg_manager,
        }
    }

    /// Install packages using slackpkg
    fn slackpkg_install(&self, root: &Path, packages: &[&str]) -> Result<()> {
        let root_str = root.to_string_lossy();

        // slackpkg doesn't have a built-in chroot mode, so we use installpkg with ROOT
        for pkg in packages {
            cmd::run(
                "ROOT",
                [
                    &root_str,
                    "slackpkg",
                    "install",
                    "-default_answer=y",
                    "-batch=on",
                    pkg,
                ],
            )?;
        }

        Ok(())
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
        match self.pkg_manager {
            SlackwarePkgManager::Slackpkg => "slackpkg",
            SlackwarePkgManager::SlaptGet => "slapt-get",
        }
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        self.package_map.get(generic).cloned().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
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

        match self.pkg_manager {
            SlackwarePkgManager::Slackpkg => self.slackpkg_install(root, &pkg_refs),
            SlackwarePkgManager::SlaptGet => self.slaptget_install(root, &pkg_refs),
        }
    }

    fn update_system(&self) -> Result<()> {
        match self.pkg_manager {
            SlackwarePkgManager::Slackpkg => {
                cmd::run("slackpkg", ["update"])?;
                cmd::run("slackpkg", ["upgrade-all", "-default_answer=y", "-batch=on"])
            }
            SlackwarePkgManager::SlaptGet => {
                cmd::run("slapt-get", ["--update"])?;
                cmd::run("slapt-get", ["--upgrade", "--yes"])
            }
        }
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
        root: &Path,
        dm: &str,
        greeter: Option<&str>,
        _configure_pam_rundir: bool,
    ) -> Result<()> {
        // Most display managers aren't in Slackware repos
        // Would need SlackBuilds
        println!(
            "Note: {} may need to be installed from SlackBuilds.org",
            dm
        );

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
            match *backend {
                "gtk" => packages.push("xdg-desktop-portal-gtk"),
                // wlr and kde not in official repos
                _ => {}
            }
        }

        self.install_packages(root, &packages)
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        // Slackware doesn't have genfstab, generate manually
        use std::process::Command;

        let output = Command::new("findmnt")
            .args(["-R", "-n", "-o", "SOURCE,TARGET,FSTYPE,OPTIONS"])
            .arg(root)
            .output()
            .context("Failed to run findmnt")?;

        if !output.status.success() {
            anyhow::bail!("findmnt failed");
        }

        let mut fstab = String::from("# /etc/fstab\n# Generated by mkOS installer\n\n");

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                fstab.push_str(&format!(
                    "{}\t{}\t{}\t{}\t0 0\n",
                    parts[0], parts[1], parts[2], parts[3]
                ));
            }
        }

        Ok(fstab)
    }

    fn package_manager(&self) -> &dyn PackageManager {
        // For now, return a stub - we'll implement Slackware package manager trait later
        todo!("Slackware PackageManager trait implementation")
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
        println!(
            "  Note: You'll need to manually run this after kernel upgrades"
        );

        Ok(())
    }
}
