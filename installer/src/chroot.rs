use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::cmd;

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

    // Unmount in reverse order (ignore errors - some may not be mounted)
    let _ = cmd::run("umount", [&format!("{}/run", target_str)]);
    let _ = cmd::run("umount", [&format!("{}/sys", target_str)]);
    let _ = cmd::run("umount", [&format!("{}/proc", target_str)]);
    let _ = cmd::run("umount", [&format!("{}/dev/pts", target_str)]);
    let _ = cmd::run("umount", [&format!("{}/dev", target_str)]);

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
        "# <target name> <source device> <key file> <options>\ncryptroot UUID={} none luks,discard\n",
        luks_uuid
    );

    let crypttab_path = target.join("etc/crypttab");
    fs::write(&crypttab_path, &crypttab_content).context("Failed to write crypttab")?;

    Ok(())
}
