use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, SysVinit};
use crate::pkgmgr::PackageManager;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Devuan {
    repo: String,
    package_map: HashMap<String, String>,
    init_system: SysVinit,
}

impl Default for Devuan {
    fn default() -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Devuan package names
        package_map.insert("base-system".into(), "systemd-shim".into());
        package_map.insert("linux-kernel".into(), "linux-image-amd64".into());
        package_map.insert("linux-firmware".into(), "firmware-linux".into());
        package_map.insert("intel-ucode".into(), "intel-microcode".into());
        package_map.insert("amd-ucode".into(), "amd64-microcode".into());
        package_map.insert("dracut".into(), "dracut".into());
        package_map.insert("efibootmgr".into(), "efibootmgr".into());
        package_map.insert("sbsigntools".into(), "sbsigntool".into());
        package_map.insert("cryptsetup".into(), "cryptsetup".into());
        package_map.insert("btrfs-progs".into(), "btrfs-progs".into());
        package_map.insert("dhcpcd".into(), "dhcpcd5".into());
        package_map.insert("iwd".into(), "iwd".into());

        // Init systems
        package_map.insert("openrc".into(), "openrc".into());
        package_map.insert("s6".into(), "s6".into());
        package_map.insert("s6-rc".into(), "s6-rc".into());
        package_map.insert("s6-linux-init".into(), "s6-linux-init".into());
        package_map.insert("runit".into(), "runit".into());

        // Wayland
        package_map.insert("wayland".into(), "libwayland-client0".into());
        package_map.insert("wayland-protocols".into(), "wayland-protocols".into());
        package_map.insert("wlroots".into(), "libwlroots11".into());
        package_map.insert("xwayland".into(), "xwayland".into());
        package_map.insert("libinput".into(), "libinput10".into());
        package_map.insert("mesa".into(), "mesa-utils".into());

        // Display managers
        package_map.insert("kitty".into(), "kitty".into());
        package_map.insert("rofi-wayland".into(), "rofi".into());

        // Audio
        package_map.insert("pipewire".into(), "pipewire".into());
        package_map.insert("wireplumber".into(), "wireplumber".into());
        package_map.insert("pipewire-pulse".into(), "pipewire-pulse".into());
        package_map.insert("pipewire-alsa".into(), "pipewire-alsa".into());
        package_map.insert("pipewire-jack".into(), "pipewire-jack".into());

        // Fonts
        package_map.insert("font-hack".into(), "fonts-hack".into());
        package_map.insert("font-noto".into(), "fonts-noto".into());
        package_map.insert("font-noto-emoji".into(), "fonts-noto-color-emoji".into());

        // XDG portals
        package_map.insert("xdg-desktop-portal".into(), "xdg-desktop-portal".into());
        package_map.insert(
            "xdg-desktop-portal-gtk".into(),
            "xdg-desktop-portal-gtk".into(),
        );
        package_map.insert("xdg-utils".into(), "xdg-utils".into());

        // GPU drivers
        package_map.insert("nvidia".into(), "nvidia-driver".into());
        package_map.insert("nvidia-prime".into(), "nvidia-prime".into());

        // Network services
        package_map.insert("avahi".into(), "avahi-daemon".into());
        package_map.insert("nss-mdns".into(), "libnss-mdns".into());
        package_map.insert("openssh".into(), "openssh-server".into());
        package_map.insert("nftables".into(), "nftables".into());

        // System services
        package_map.insert("dbus".into(), "dbus".into());
        package_map.insert("polkit".into(), "policykit-1".into());
        package_map.insert("seatd".into(), "seatd".into());
        package_map.insert("elogind".into(), "elogind".into());
        package_map.insert("pam_rundir".into(), "libpam-rundir".into());

        Self {
            repo: "https://deb.devuan.org/merged".into(),
            package_map,
            init_system: SysVinit::devuan(),
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
        self.package_map.get(generic).cloned()
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
        // Devuan doesn't have genfstab, generate manually
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
        todo!("Devuan PackageManager trait implementation")
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
