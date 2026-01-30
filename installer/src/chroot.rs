use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::cmd;
use crate::paths;

/// Mount special filesystems for chroot operations
pub fn setup_chroot(target: &Path) -> Result<()> {
    let target_str = target.to_string_lossy();

    // Bind mount /dev, /proc, /sys
    cmd::run("mount", ["--bind", "/dev", &format!("{}/dev", target_str)])?;
    cmd::run(
        "mount",
        ["--bind", "/dev/pts", &format!("{}/dev/pts", target_str)],
    )?;
    cmd::run(
        "mount",
        ["-t", "proc", "proc", &format!("{}/proc", target_str)],
    )?;
    cmd::run(
        "mount",
        ["-t", "sysfs", "sys", &format!("{}/sys", target_str)],
    )?;
    cmd::run("mount", ["--bind", "/run", &format!("{}/run", target_str)])?;

    Ok(())
}

/// Unmount special filesystems after chroot operations
pub fn teardown_chroot(target: &Path) -> Result<()> {
    let target_str = target.to_string_lossy();

    // Unmount in reverse order (log errors - some may not be mounted)
    let mounts = ["run", "sys", "proc", "dev/pts", "dev"];
    for mount in mounts {
        let path = format!("{}/{}", target_str, mount);
        if let Err(e) = cmd::run("umount", [&path]) {
            tracing::debug!("Failed to unmount {}: {}", path, e);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct SystemConfig {
    pub hostname: String,
    pub timezone: String,
    pub locale: String,
    pub keymap: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            hostname: "mkos".into(),
            timezone: "UTC".into(),
            locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
        }
    }
}

pub fn configure_system(target: &Path, config: &SystemConfig) -> Result<()> {
    configure_timezone(target, &config.timezone)?;
    configure_locale(target, &config.locale)?;
    configure_hostname(target, &config.hostname)?;
    configure_keymap(target, &config.keymap)?;
    Ok(())
}

fn configure_timezone(target: &Path, timezone: &str) -> Result<()> {
    let zoneinfo = format!("/usr/share/zoneinfo/{}", timezone);
    let localtime = target.join("etc/localtime");

    // Remove existing symlink if present
    let _ = fs::remove_file(&localtime);

    std::os::unix::fs::symlink(&zoneinfo, &localtime).context("Failed to set timezone")?;

    // Run hwclock in chroot (non-fatal - VMs often don't have hardware clock access)
    if cmd::run(
        "chroot",
        [&target.to_string_lossy(), "hwclock", "--systohc"],
    )
    .is_err()
    {
        println!("Warning: Could not set hardware clock. This is normal if you're in a VM.");
    }

    Ok(())
}

fn configure_locale(target: &Path, locale: &str) -> Result<()> {
    let locale_gen = target.join("etc/locale.gen");
    fs::write(&locale_gen, format!("{} UTF-8\n", locale)).context("Failed to write locale.gen")?;

    cmd::run("chroot", [&target.to_string_lossy(), "locale-gen"])?;

    let locale_conf = target.join("etc/locale.conf");
    fs::write(&locale_conf, format!("LANG={}\n", locale)).context("Failed to write locale.conf")?;

    Ok(())
}

fn configure_hostname(target: &Path, hostname: &str) -> Result<()> {
    let hostname_file = target.join("etc/hostname");
    fs::write(&hostname_file, format!("{}\n", hostname)).context("Failed to write hostname")?;

    let hosts_file = target.join("etc/hosts");
    let hosts_content = format!(
        "127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\t{}.localdomain\t{}\n",
        hostname, hostname
    );
    fs::write(&hosts_file, hosts_content).context("Failed to write hosts")?;

    Ok(())
}

fn configure_keymap(target: &Path, keymap: &str) -> Result<()> {
    let vconsole = target.join("etc/vconsole.conf");
    fs::write(&vconsole, format!("KEYMAP={}\n", keymap))
        .context("Failed to write vconsole.conf")?;

    Ok(())
}

pub fn set_root_password(target: &Path, password: &str) -> Result<()> {
    cmd::run_with_stdin(
        "chroot",
        [&target.to_string_lossy(), "chpasswd"],
        format!("root:{}\n", password).as_bytes(),
    )
}

pub fn generate_fstab(target: &Path, fstab_content: &str) -> Result<()> {
    let fstab_path = target.join("etc/fstab");
    fs::write(&fstab_path, fstab_content).context("Failed to write fstab")?;

    Ok(())
}

pub fn generate_crypttab(target: &Path, luks_uuid: &str) -> Result<()> {
    let crypttab_content = format!(
        "# <target name> <source device> <key file> <options>\n{} UUID={} none luks,discard\n",
        paths::LUKS_MAPPER_NAME,
        luks_uuid
    );

    let crypttab_path = target.join("etc/crypttab");
    fs::write(&crypttab_path, &crypttab_content).context("Failed to write crypttab")?;

    Ok(())
}

/// Create a user account with the specified groups
pub fn create_user(target: &Path, username: &str, password: &str, groups: &[&str]) -> Result<()> {
    let target_str = target.to_string_lossy().to_string();

    // Create user with home directory
    cmd::run(
        "chroot",
        [&target_str, "useradd", "-m", "-s", "/bin/bash", username],
    )
    .context(format!("Failed to create user '{}'", username))?;

    // Set password
    cmd::run_with_stdin(
        "chroot",
        [&target_str, "chpasswd"],
        format!("{}:{}\n", username, password).as_bytes(),
    )
    .context(format!("Failed to set password for '{}'", username))?;

    // Add to groups if any
    if !groups.is_empty() {
        let groups_str = groups.join(",");
        cmd::run(
            "chroot",
            [&target_str, "usermod", "-aG", &groups_str, username],
        )
        .context(format!("Failed to add '{}' to groups", username))?;
    }

    Ok(())
}

/// Determine user groups based on enabled features
pub fn determine_user_groups(
    desktop_enabled: bool,
    seat_manager: Option<&str>,
    audio_enabled: bool,
) -> Vec<&'static str> {
    let mut groups = vec!["wheel"]; // Always admin

    if desktop_enabled {
        groups.extend(["video", "render"]);

        let seat_mgr = seat_manager.unwrap_or("seatd");
        if seat_mgr == "seatd" {
            groups.push("seat");
        }
    }

    if audio_enabled {
        groups.push("audio");
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_always_include_wheel() {
        let groups = determine_user_groups(false, None, false);
        assert_eq!(groups, vec!["wheel"]);
    }

    #[test]
    fn desktop_adds_video_render_and_seat() {
        let groups = determine_user_groups(true, None, false);
        assert_eq!(groups, vec!["wheel", "video", "render", "seat"]);
    }

    #[test]
    fn desktop_with_seatd_adds_seat() {
        let groups = determine_user_groups(true, Some("seatd"), false);
        assert!(groups.contains(&"seat"));
    }

    #[test]
    fn desktop_with_elogind_omits_seat() {
        let groups = determine_user_groups(true, Some("elogind"), false);
        assert!(!groups.contains(&"seat"));
        assert!(groups.contains(&"video"));
        assert!(groups.contains(&"render"));
    }

    #[test]
    fn audio_adds_audio_group() {
        let groups = determine_user_groups(false, None, true);
        assert_eq!(groups, vec!["wheel", "audio"]);
    }

    #[test]
    fn desktop_and_audio_combined() {
        let groups = determine_user_groups(true, Some("seatd"), true);
        assert_eq!(groups, vec!["wheel", "video", "render", "seat", "audio"]);
    }

    #[test]
    fn desktop_elogind_with_audio() {
        let groups = determine_user_groups(true, Some("elogind"), true);
        assert_eq!(groups, vec!["wheel", "video", "render", "audio"]);
    }
}

/// Configure sudoers to allow wheel group sudo access
pub fn configure_sudoers(target: &Path) -> Result<()> {
    let sudoers_d = target.join("etc/sudoers.d");
    fs::create_dir_all(&sudoers_d)?;

    let wheel_file = sudoers_d.join("wheel");
    fs::write(&wheel_file, "%wheel ALL=(ALL:ALL) ALL\n")
        .context("Failed to write sudoers.d/wheel")?;

    // Set restrictive permissions (required by sudo)
    fs::set_permissions(&wheel_file, fs::Permissions::from_mode(0o440))
        .context("Failed to set permissions on sudoers.d/wheel")?;

    Ok(())
}

/// Configure NSSwitch for name resolution
///
/// When mDNS is enabled, configures the hosts line to use mdns_minimal
/// for .local domain resolution via Avahi.
pub fn configure_nsswitch(target: &Path, mdns_enabled: bool) -> Result<()> {
    let hosts_line = if mdns_enabled {
        "hosts:      files mdns_minimal [NOTFOUND=return] resolve [!UNAVAIL=return] dns"
    } else {
        "hosts:      files resolve [!UNAVAIL=return] dns"
    };

    let content = format!(
        "# /etc/nsswitch.conf - Name Service Switch configuration\n\
         # Generated by mkOS installer\n\n\
         passwd:     files\n\
         group:      files\n\
         shadow:     files\n\n\
         {}\n\n\
         networks:   files\n\
         protocols:  files\n\
         services:   files\n\
         ethers:     files\n\
         rpc:        files\n",
        hosts_line
    );

    fs::write(target.join("etc/nsswitch.conf"), content)
        .context("Failed to write nsswitch.conf")?;

    Ok(())
}
