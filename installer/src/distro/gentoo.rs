use super::Distro;
use crate::cmd;
use crate::init::{InitSystem, OpenRC};
use crate::pkgmgr::PackageManager;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct Gentoo {
    repo: String,
    package_map: HashMap<String, String>,
    init_system: OpenRC,
}

impl Default for Gentoo {
    fn default() -> Self {
        let mut package_map = HashMap::new();

        // Map generic names to Gentoo package names (category/name format)
        package_map.insert("base-system".into(), "@system".into());
        package_map.insert("linux-kernel".into(), "sys-kernel/gentoo-kernel-bin".into());
        package_map.insert("linux-firmware".into(), "sys-kernel/linux-firmware".into());
        package_map.insert("intel-ucode".into(), "sys-firmware/intel-microcode".into());
        package_map.insert("amd-ucode".into(), "sys-firmware/amd-microcode".into());
        package_map.insert("dracut".into(), "sys-kernel/dracut".into());
        package_map.insert("efibootmgr".into(), "sys-boot/efibootmgr".into());
        package_map.insert("sbsigntools".into(), "app-crypt/sbsigntools".into());
        package_map.insert("cryptsetup".into(), "sys-fs/cryptsetup".into());
        package_map.insert("btrfs-progs".into(), "sys-fs/btrfs-progs".into());
        package_map.insert("dhcpcd".into(), "net-misc/dhcpcd".into());
        package_map.insert("iwd".into(), "net-wireless/iwd".into());

        // Init systems
        package_map.insert("openrc".into(), "sys-apps/openrc".into());
        package_map.insert("s6".into(), "sys-apps/s6".into());
        package_map.insert("s6-rc".into(), "sys-apps/s6-rc".into());
        package_map.insert("s6-linux-init".into(), "sys-apps/s6-linux-init".into());
        package_map.insert("runit".into(), "sys-process/runit".into());

        // Wayland
        package_map.insert("wayland".into(), "dev-libs/wayland".into());
        package_map.insert(
            "wayland-protocols".into(),
            "dev-libs/wayland-protocols".into(),
        );
        package_map.insert("wlroots".into(), "gui-libs/wlroots".into());
        package_map.insert("xwayland".into(), "x11-base/xwayland".into());
        package_map.insert("libinput".into(), "dev-libs/libinput".into());
        package_map.insert("mesa".into(), "media-libs/mesa".into());

        // Display managers
        package_map.insert("greetd".into(), "gui-apps/greetd".into());
        package_map.insert("greetd-tuigreet".into(), "gui-apps/tuigreet".into());
        package_map.insert("greetd-gtkgreet".into(), "gui-apps/gtkgreet".into());
        package_map.insert("kitty".into(), "x11-terms/kitty".into());
        package_map.insert("rofi-wayland".into(), "x11-misc/rofi".into());

        // Audio
        package_map.insert("pipewire".into(), "media-video/pipewire".into());
        package_map.insert("wireplumber".into(), "media-video/wireplumber".into());

        // Fonts
        package_map.insert("font-hack".into(), "media-fonts/hack".into());
        package_map.insert("font-noto".into(), "media-fonts/noto".into());
        package_map.insert("font-noto-emoji".into(), "media-fonts/noto-emoji".into());

        // XDG portals
        package_map.insert(
            "xdg-desktop-portal".into(),
            "sys-apps/xdg-desktop-portal".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-wlr".into(),
            "gui-libs/xdg-desktop-portal-wlr".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-gtk".into(),
            "sys-apps/xdg-desktop-portal-gtk".into(),
        );
        package_map.insert(
            "xdg-desktop-portal-kde".into(),
            "kde-plasma/xdg-desktop-portal-kde".into(),
        );
        package_map.insert("xdg-utils".into(), "x11-misc/xdg-utils".into());

        // GPU drivers
        package_map.insert("nvidia".into(), "x11-drivers/nvidia-drivers".into());

        // Network services
        package_map.insert("avahi".into(), "net-dns/avahi".into());
        package_map.insert("nss-mdns".into(), "sys-auth/nss-mdns".into());
        package_map.insert("openssh".into(), "net-misc/openssh".into());
        package_map.insert("nftables".into(), "net-firewall/nftables".into());

        // System services
        package_map.insert("dbus".into(), "sys-apps/dbus".into());
        package_map.insert("polkit".into(), "sys-auth/polkit".into());
        package_map.insert("seatd".into(), "sys-auth/seatd".into());
        package_map.insert("elogind".into(), "sys-auth/elogind".into());

        Self {
            repo: "https://gentoo.osuosl.org/".into(),
            package_map,
            init_system: OpenRC::gentoo(),
        }
    }
}

impl Distro for Gentoo {
    fn name(&self) -> &str {
        "Gentoo Linux"
    }

    fn pkg_manager(&self) -> &str {
        "emerge"
    }

    fn repo_url(&self) -> &str {
        &self.repo
    }

    fn map_package(&self, generic: &str) -> Option<String> {
        self.package_map.get(generic).cloned()
    }

    fn map_service(&self, generic: &str) -> String {
        // Gentoo uses simple service names
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
        let mut args = vec!["--root", &root_str, "--ask", "n"];
        let mapped_refs: Vec<&str> = mapped.iter().map(|s| s.as_str()).collect();
        args.extend(mapped_refs);

        cmd::run("emerge", args)
    }

    fn update_system(&self) -> Result<()> {
        // Sync repos
        cmd::run("emerge", ["--sync"])?;
        // Update world set
        cmd::run("emerge", ["--update", "--deep", "--newuse", "@world"])
    }

    fn bootstrap(&self, root: &Path, enable_networking: bool) -> Result<()> {
        println!("\n=== Gentoo Bootstrap ===");

        // Check if stage3 is already extracted
        if !is_stage3_extracted(root)? {
            println!("Stage3 not found. Downloading and extracting...\n");
            download_and_extract_stage3(root)?;
        } else {
            println!("Stage3 already extracted, skipping download.\n");
        }

        // Install kernel and essential packages
        println!("Installing kernel and essential packages...");
        let mut packages = vec!["sys-kernel/gentoo-kernel-bin"];

        if enable_networking {
            packages.push("net-misc/dhcpcd");
        }

        self.install_packages(root, &packages)?;

        if enable_networking {
            let service = self.map_service("dhcpcd");
            self.init_system.enable_service(root, &service)?;
        }

        Ok(())
    }

    fn install_desktop_base(&self, root: &Path, seat_manager: &str) -> Result<()> {
        let packages = match seat_manager {
            "elogind" => vec!["sys-auth/elogind", "sys-auth/polkit", "x11-misc/xdg-utils"],
            _ => vec!["sys-auth/seatd", "sys-auth/polkit", "x11-misc/xdg-utils"],
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
                let mut pkgs = vec!["gui-apps/greetd"];
                if let Some(g) = greeter {
                    match g {
                        "tuigreet" => pkgs.push("gui-apps/tuigreet"),
                        "gtkgreet" => pkgs.push("gui-apps/gtkgreet"),
                        _ => {}
                    }
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
        let mut packages = vec!["sys-apps/xdg-desktop-portal"];

        for backend in backends {
            match *backend {
                "wlr" => packages.push("gui-libs/xdg-desktop-portal-wlr"),
                "gtk" => packages.push("sys-apps/xdg-desktop-portal-gtk"),
                "kde" => packages.push("kde-plasma/xdg-desktop-portal-kde"),
                _ => {}
            }
        }

        self.install_packages(root, &packages)
    }

    fn generate_fstab(&self, root: &Path) -> Result<String> {
        // Gentoo doesn't have genfstab, generate manually
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
        todo!("Gentoo PackageManager trait implementation")
    }

    fn install_kernel_hook(&self, target: &Path) -> Result<()> {
        use std::fs;

        // Gentoo uses /etc/portage/postsync.d/ for hooks after emerge --sync
        // But for kernel upgrades, we use /etc/kernel/postinst.d/
        let hook_dir = target.join("etc/kernel/postinst.d");
        fs::create_dir_all(&hook_dir)?;

        let hook_content = r#"#!/bin/sh
# mkOS kernel hook for Gentoo
# Called after kernel installation

KERNEL_VERSION="$1"

if [ -z "$KERNEL_VERSION" ]; then
    # Try to detect from /usr/src/linux
    if [ -L "/usr/src/linux" ]; then
        KERNEL_VERSION=$(basename "$(readlink /usr/src/linux)" | sed 's/linux-//')
    else
        echo "ERROR: Could not detect kernel version"
        exit 1
    fi
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

        println!("✓ Installed kernel hook for Gentoo (kernel postinst.d)");

        Ok(())
    }
}

// Helper functions for stage3 bootstrap

fn is_stage3_extracted(root: &Path) -> Result<bool> {
    // Check if essential Gentoo directories exist
    let markers = [
        root.join("etc/portage"),
        root.join("var/db/repos/gentoo"),
        root.join("usr/portage"), // Older location
    ];

    // If any marker exists, assume stage3 is extracted
    Ok(markers.iter().any(|p| p.exists()))
}

fn download_and_extract_stage3(root: &Path) -> Result<()> {
    use std::process::Command;

    // Detect architecture
    let arch = detect_architecture()?;
    println!("Detected architecture: {}", arch);

    // Prompt for stage3 variant
    let variant = prompt_stage3_variant()?;
    println!("Selected variant: {}\n", variant);

    // Construct download URL
    let mirror = "https://distfiles.gentoo.org/releases";
    let autobuilds = format!("{}/{}/autobuilds", mirror, arch);

    // Get latest stage3 filename
    println!("Fetching latest stage3 information...");
    let latest_file = format!("latest-stage3-{}-{}.txt", arch, variant);
    let latest_url = format!("{}/{}", autobuilds, latest_file);

    let output = Command::new("curl")
        .args(["-sL", &latest_url])
        .output()
        .context("Failed to fetch latest stage3 info. Is curl installed?")?;

    if !output.status.success() {
        anyhow::bail!("Failed to fetch latest stage3 info from Gentoo mirrors");
    }

    let latest_content = String::from_utf8_lossy(&output.stdout);

    // Parse the latest file - format is:
    // # comment lines
    // YYYYMMDDTHHMMSSZ/stage3-amd64-openrc-YYYYMMDDTHHMMSSZ.tar.xz
    let stage3_path = latest_content
        .lines()
        .find(|line| !line.starts_with('#') && line.contains("stage3"))
        .ok_or_else(|| anyhow::anyhow!("Could not parse latest stage3 file"))?
        .trim();

    let stage3_url = format!("{}/{}", autobuilds, stage3_path);
    let filename = stage3_path.split('/').next_back().unwrap();

    println!("Downloading: {}", filename);
    println!("This may take several minutes...\n");

    // Download to /tmp
    let tmp_path = format!("/tmp/{}", filename);

    let status = Command::new("curl")
        .args([
            "-L",             // Follow redirects
            "--progress-bar", // Show progress
            "-o",
            &tmp_path, // Output file
            &stage3_url,
        ])
        .status()
        .context("Failed to download stage3 tarball")?;

    if !status.success() {
        anyhow::bail!("Failed to download stage3 tarball");
    }

    println!("\n✓ Download complete");

    // Extract to root
    println!("Extracting stage3 tarball to {}...", root.display());
    println!("This will take a few minutes...\n");

    let root_str = root.to_string_lossy();
    let status = Command::new("tar")
        .args([
            "xpf",
            &tmp_path,
            "--xattrs-include=*.*",
            "--numeric-owner",
            "-C",
            &root_str,
        ])
        .status()
        .context("Failed to extract stage3 tarball")?;

    if !status.success() {
        anyhow::bail!("Failed to extract stage3 tarball");
    }

    // Clean up
    let _ = std::fs::remove_file(&tmp_path);

    println!("✓ Stage3 extracted successfully\n");

    Ok(())
}

fn detect_architecture() -> Result<&'static str> {
    use std::process::Command;

    let output = Command::new("uname")
        .arg("-m")
        .output()
        .context("Failed to detect architecture")?;

    let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    match arch.as_str() {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        "armv7l" => Ok("arm"),
        "ppc64le" => Ok("ppc64le"),
        other => anyhow::bail!("Unsupported architecture: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gentoo() -> Gentoo {
        Gentoo::default()
    }

    #[test]
    fn map_package_base_system() {
        assert_eq!(gentoo().map_package("base-system"), Some("@system".into()));
    }

    #[test]
    fn map_package_linux_kernel() {
        assert_eq!(
            gentoo().map_package("linux-kernel"),
            Some("sys-kernel/gentoo-kernel-bin".into())
        );
    }

    #[test]
    fn map_package_cryptsetup() {
        assert_eq!(
            gentoo().map_package("cryptsetup"),
            Some("sys-fs/cryptsetup".into())
        );
    }

    #[test]
    fn map_package_greetd_tuigreet() {
        assert_eq!(
            gentoo().map_package("greetd-tuigreet"),
            Some("gui-apps/tuigreet".into())
        );
    }

    #[test]
    fn map_package_intel_ucode() {
        assert_eq!(
            gentoo().map_package("intel-ucode"),
            Some("sys-firmware/intel-microcode".into())
        );
    }

    #[test]
    fn map_package_unknown_returns_none() {
        assert_eq!(gentoo().map_package("nonexistent"), None);
    }

    #[test]
    fn map_service_passes_through() {
        assert_eq!(gentoo().map_service("dbus"), "dbus");
        assert_eq!(gentoo().map_service("seatd"), "seatd");
    }

    #[test]
    fn distro_trait_name() {
        let g = gentoo();
        assert_eq!(Distro::name(&g), "Gentoo Linux");
    }
}

fn prompt_stage3_variant() -> Result<&'static str> {
    use std::io::{self, Write};

    println!("Select Gentoo stage3 variant:");
    println!("  [1] openrc - Standard OpenRC init (recommended)");
    println!("  [2] openrc-hardened - Hardened OpenRC with security features");
    println!("  [3] musl-openrc - OpenRC with musl libc (lightweight)");

    loop {
        print!("Select variant [1-3]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" | "" => return Ok("openrc"),
            "2" => return Ok("openrc-hardened"),
            "3" => return Ok("musl-openrc"),
            _ => println!("Invalid selection. Please enter 1-3."),
        }
    }
}
