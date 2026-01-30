use anyhow::{bail, Result};
use std::path::PathBuf;

use crate::disk;
use crate::distro::DistroKind;
use crate::install::{DesktopConfig, InstallConfig, SwapConfig};
use crate::manifest::Manifest;

use super::gpu::{detect_gpus, get_nvidia_packages, GpuVendor};
use super::prompts::{
    prompt_default, prompt_display_manager, prompt_greeter, prompt_passphrase,
    prompt_password_confirm, prompt_seat_manager, prompt_yes_no, select_device,
};

pub fn build_config(manifest: &Manifest) -> Result<InstallConfig> {
    // Get device - from manifest or prompt
    let device = match &manifest.disk.device {
        Some(dev) => {
            println!("Using device from manifest: {}", dev);
            PathBuf::from(dev)
        }
        None => {
            let devices = disk::list_block_devices()?;
            if devices.is_empty() {
                bail!("No block devices found");
            }
            println!("Available disks:");
            for (i, dev) in devices.iter().enumerate() {
                let size_gb = dev.size_bytes / 1_000_000_000;
                let model = dev.model.as_deref().unwrap_or("Unknown");
                println!("  [{}] {} - {} GB - {}", i + 1, dev.path, size_gb, model);
            }
            let selected = select_device(&devices)?;
            println!("\nSelected: {}\n", selected.path);
            PathBuf::from(&selected.path)
        }
    };

    // Always prompt for passphrase (never in manifest for security)
    let passphrase = if manifest.disk.encryption {
        prompt_passphrase()?
    } else {
        String::new()
    };

    // Always prompt for root password (never in manifest for security)
    println!();
    let root_password = prompt_password_confirm("Root password")?;

    // Get hostname - from manifest or prompt
    let hostname = if manifest.system.hostname != "mkos" {
        println!("Using hostname from manifest: {}", manifest.system.hostname);
        manifest.system.hostname.clone()
    } else {
        prompt_default("Hostname", "mkos")?
    };

    // Get timezone - from manifest or prompt
    let timezone = if manifest.system.timezone != "UTC" {
        println!("Using timezone from manifest: {}", manifest.system.timezone);
        manifest.system.timezone.clone()
    } else {
        prompt_default("Timezone", "UTC")?
    };

    // Get locale from manifest
    let locale = manifest.system.locale.clone();

    // Get keymap from manifest
    let keymap = manifest.system.keymap.clone();

    // Get distro - from manifest, auto-detect, or prompt
    let distro = if manifest.distro == "artix" {
        // Default value - try to auto-detect, prompt if detection fails
        match crate::distro::detect() {
            Ok(detected) => {
                println!("Detected live environment: {}", detected.name());
                detected
            }
            Err(_) => {
                println!("Could not auto-detect distribution.");
                super::prompts::prompt_distro()?
            }
        }
    } else {
        // Non-default value from manifest, use it
        println!("Using distro from manifest: {}", manifest.distro);
        match manifest.distro.as_str() {
            "artix" => DistroKind::Artix,
            "void" => DistroKind::Void,
            "slackware" => DistroKind::Slackware,
            "alpine" => DistroKind::Alpine,
            "gentoo" => DistroKind::Gentoo,
            "devuan" => DistroKind::Devuan,
            other => bail!(
                "Unknown distro: {}. Supported: artix, void, slackware, alpine, gentoo, devuan",
                other
            ),
        }
    };

    // Enable networking - check if any networking services are requested
    let enable_networking =
        manifest.services.enable.iter().any(|s| {
            s == "dhcpcd" || s == "networkmanager" || s == "connman" || s.contains("network")
        }) || prompt_yes_no("Enable networking (DHCP)", true)?;

    // Detect GPUs and offer proprietary drivers
    let mut extra_packages = Vec::new();
    let gpus = detect_gpus();

    if gpus.contains(&GpuVendor::Nvidia) {
        println!("\nNVIDIA GPU detected.");
        if prompt_yes_no("Install proprietary NVIDIA drivers", false)? {
            extra_packages.extend(get_nvidia_packages());
            println!("  Will install: nvidia, nvidia-utils, nvidia-prime, lib32-nvidia-utils");
        }
    }

    // Desktop environment setup - from manifest or prompt
    let desktop = if manifest.desktop.enabled {
        println!("Using desktop config from manifest");
        DesktopConfig {
            enabled: true,
            seat_manager: manifest.desktop.seat_manager.clone(),
            display_manager: manifest.desktop.display_manager.clone(),
            greeter: manifest.desktop.greeter.clone(),
            user_services: manifest.desktop.user_services,
            portals: manifest.desktop.portals,
            portal_backends: manifest.desktop.portal_backends.clone(),
            greetd_config: manifest.desktop.greetd.clone(),
        }
    } else {
        prompt_desktop_config()?
    };

    // Swap configuration - from manifest or prompt
    let swap = if manifest.swap.zram || manifest.swap.swapfile {
        println!("Using swap config from manifest");
        SwapConfig {
            zram_enabled: manifest.swap.zram,
            zram_size_gb: manifest.swap.zram_size,
            swapfile_enabled: manifest.swap.swapfile,
            swapfile_size_gb: manifest.swap.swapfile_size,
            swappiness: manifest.swap.swappiness,
        }
    } else {
        prompt_swap_config()?
    };

    // Audio configuration - from manifest or prompt
    let audio = if manifest.audio.enabled {
        println!("Audio enabled from manifest");
        manifest.audio.clone()
    } else if desktop.enabled {
        println!("\n  Audio: pipewire (automatic with desktop)");
        // Enable audio with defaults when desktop is enabled
        crate::manifest::AudioConfig {
            enabled: true,
            ..Default::default()
        }
    } else if prompt_yes_no("\nEnable audio support (pipewire)", false)? {
        crate::manifest::AudioConfig {
            enabled: true,
            ..Default::default()
        }
    } else {
        crate::manifest::AudioConfig::default()
    };

    // Network config from manifest (no interactive prompts yet)
    let network = manifest.network.clone();

    // Firewall config from manifest (no interactive prompts yet)
    let firewall = manifest.firewall.clone();

    // Microcode - detect CPU and prompt user
    let microcode = prompt_microcode()?;

    Ok(InstallConfig {
        device,
        passphrase,
        root_password,
        hostname,
        timezone,
        locale,
        keymap,
        distro,
        enable_networking,
        extra_packages,
        desktop,
        swap,
        audio,
        network,
        firewall,
        secureboot: crate::install::SecureBootConfig::default(),
        microcode,
    })
}

fn prompt_desktop_config() -> Result<DesktopConfig> {
    println!("\n=== Desktop Environment ===");

    if !prompt_yes_no("Install graphical session support", false)? {
        return Ok(DesktopConfig::default());
    }

    // Ask about seat manager
    let seat_manager = prompt_seat_manager()?;

    println!(
        "  Will install: {}, polkit, xdg-utils",
        seat_manager.as_deref().unwrap_or("seatd")
    );

    // Ask about display manager
    let display_manager = prompt_display_manager()?;
    let greeter = if display_manager.is_some() {
        prompt_greeter(display_manager.as_deref())?
    } else {
        None
    };

    // Ask about user-level services if display manager is set
    let user_services = if display_manager.is_some() {
        prompt_yes_no("Enable user services (pipewire, etc. via user s6)", true)?
    } else {
        false
    };

    // Ask about XDG desktop portals
    let portals = prompt_yes_no(
        "Enable XDG desktop portals (screen sharing, file dialogs)",
        true,
    )?;
    let portal_backends = if portals {
        // Default to wlr + gtk for Wayland setups
        vec!["wlr".to_string(), "gtk".to_string()]
    } else {
        Vec::new()
    };

    Ok(DesktopConfig {
        enabled: true,
        seat_manager,
        display_manager,
        greeter,
        user_services,
        portals,
        portal_backends,
        greetd_config: None, // Uses defaults, manifest can override
    })
}

fn prompt_swap_config() -> Result<SwapConfig> {
    println!("\n=== Swap Configuration ===");

    // Get system RAM for defaults
    let ram_gb = get_system_ram_gb();
    let default_zram_gb = std::cmp::min(ram_gb / 2, 16).max(1);
    let default_swapfile_gb = ram_gb.max(1);

    // zram
    let zram_enabled = prompt_yes_no("Enable zram (compressed RAM swap)", true)?;
    let zram_size_gb = if zram_enabled {
        let size_str = prompt_default("  zram size (GB)", &format!("{}", default_zram_gb))?;
        Some(size_str.parse::<u32>().unwrap_or(default_zram_gb))
    } else {
        None
    };

    // swapfile
    let swapfile_enabled = prompt_yes_no("Enable swapfile (disk swap)", false)?;
    let swapfile_size_gb = if swapfile_enabled {
        let size_str = prompt_default("  Swapfile size (GB)", &format!("{}", default_swapfile_gb))?;
        Some(size_str.parse::<u32>().unwrap_or(default_swapfile_gb))
    } else {
        None
    };

    // swappiness (only ask if any swap is enabled)
    let swappiness = if zram_enabled || swapfile_enabled {
        let swap_str = prompt_default("  Swappiness (0-100, lower = prefer RAM)", "20")?;
        swap_str.parse::<u8>().unwrap_or(20).min(100)
    } else {
        20
    };

    Ok(SwapConfig {
        zram_enabled,
        zram_size_gb,
        swapfile_enabled,
        swapfile_size_gb,
        swappiness,
    })
}

fn get_system_ram_gb() -> u32 {
    // Read /proc/meminfo to get total RAM
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                // Format: "MemTotal:       16384000 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return (kb / 1_048_576) as u32; // KB to GB
                    }
                }
            }
        }
    }
    // Default to 8GB if we can't detect
    8
}

fn prompt_microcode() -> Result<bool> {
    use crate::util::{detect_cpu_vendor, CpuVendor};

    let vendor = detect_cpu_vendor();

    match vendor {
        CpuVendor::Intel | CpuVendor::Amd => prompt_yes_no(
            &format!(
                "Install {} microcode updates (CPU security fixes, proprietary)",
                vendor.name()
            ),
            false,
        ),
        CpuVendor::Unknown => Ok(false),
    }
}

