use anyhow::{bail, Result};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::disk::{self, BlockDevice};
use crate::distro::DistroKind;
use crate::install::{DesktopConfig, InstallConfig, Installer, SwapConfig};
use crate::manifest::{self, Manifest, ManifestBundle, ManifestSource};
use crate::mirror;
use crate::prompt::{self, FieldSpec, FieldValue};

#[derive(Debug, Clone, PartialEq)]
enum GpuVendor {
    Nvidia,
    AmdDiscrete,
    Intel,
}

fn detect_gpus() -> Vec<GpuVendor> {
    let output = match Command::new("lspci").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(_) => {
            eprintln!("Warning: lspci returned an error, GPU detection skipped");
            return Vec::new();
        }
        Err(_) => {
            eprintln!("Warning: lspci not found, GPU detection skipped");
            return Vec::new();
        }
    };

    let mut gpus = Vec::new();

    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("vga") || line_lower.contains("3d controller") {
            if line_lower.contains("nvidia") {
                gpus.push(GpuVendor::Nvidia);
            } else if line_lower.contains("amd") || line_lower.contains("ati") {
                if line_lower.contains("radeon")
                    || line_lower.contains("navi")
                    || line_lower.contains("vega")
                {
                    gpus.push(GpuVendor::AmdDiscrete);
                }
            } else if line_lower.contains("intel") {
                gpus.push(GpuVendor::Intel);
            }
        }
    }

    gpus
}

fn get_nvidia_packages() -> Vec<String> {
    vec![
        "nvidia".into(),
        "nvidia-utils".into(),
        "nvidia-prime".into(),
        "lib32-nvidia-utils".into(),
    ]
}

pub fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let source = ManifestSource::from_arg(args.get(1).map(|s| s.as_str()));

    println!("\n=== mkOS Installer ===\n");
    println!("This will install mkOS with:");
    println!("  - LUKS2 encryption (Argon2id)");
    println!("  - btrfs with subvolumes");
    println!("  - Your choice of distribution");
    println!("  - EFISTUB boot\n");

    // Load manifest
    let bundle = match &source {
        ManifestSource::Interactive => {
            println!("Running in interactive mode (no manifest provided)\n");
            ManifestBundle {
                manifest: Manifest::default(),
                files_dir: None,
            }
        }
        ManifestSource::File(path) => {
            println!("Loading manifest from: {}\n", path.display());
            manifest::load(&source)?
        }
        ManifestSource::Url(url) => {
            println!("Loading manifest from: {}\n", url);
            manifest::load(&source)?
        }
        ManifestSource::Stdin => {
            println!("Loading manifest from stdin...\n");
            manifest::load(&source)?
        }
    };

    let manifest = bundle.manifest;

    // Collect missing configuration interactively
    let config = build_config(&manifest)?;

    // Show summary and confirm
    print_summary(&config);

    let confirm = prompt("Type 'yes' to continue: ")?;
    if confirm.to_lowercase() != "yes" {
        println!("Aborted.");
        return Ok(());
    }

    // Select mirror
    println!("\n=== Mirror Selection ===");
    if let Err(e) = mirror::setup_mirror() {
        println!("Warning: Could not configure mirror: {}", e);
        println!("Continuing with default mirror...");
    }

    // Run install
    println!("\n=== Installing ===\n");
    let installer = Installer::new(config);
    installer.run()?;

    println!("\n=== Installation Complete ===\n");
    println!("You can now reboot into your new system.");
    println!("Remember to remove the installation media.\n");

    Ok(())
}

fn build_config(manifest: &Manifest) -> Result<InstallConfig> {
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
        match detect_live_distro() {
            Ok(detected) => {
                println!("Detected live environment: {}", distro_name(detected));
                detected
            }
            Err(_) => {
                println!("Could not auto-detect distribution.");
                prompt_distro()?
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

fn prompt_seat_manager() -> Result<Option<String>> {
    println!("\nSeat manager options:");
    println!("  [1] seatd - Minimal seat management (recommended)");
    println!("  [2] elogind - Full session management (seat, login, power)");

    loop {
        let input = prompt("Select seat manager [1-2]: ")?;
        match input.as_str() {
            "1" | "" => return Ok(None), // None = default to seatd
            "2" => return Ok(Some("elogind".into())),
            _ => println!("Invalid selection"),
        }
    }
}

fn prompt_display_manager() -> Result<Option<String>> {
    println!("\nDisplay manager options:");
    println!("  [1] greetd - Minimal, flexible login daemon");
    println!("  [2] ly - TUI display manager");
    println!("  [3] None - Start session manually or via ~/.profile");

    loop {
        let input = prompt("Select display manager [1-3]: ")?;
        match input.as_str() {
            "1" => return Ok(Some("greetd".into())),
            "2" => return Ok(Some("ly".into())),
            "3" | "" => return Ok(None),
            _ => println!("Invalid selection"),
        }
    }
}

fn prompt_greeter(dm: Option<&str>) -> Result<Option<String>> {
    match dm {
        Some("greetd") => {
            println!("\nGreeter options for greetd:");
            println!("  [1] regreet - GTK4 graphical greeter (requires cage)");
            println!("  [2] tuigreet - Terminal-based greeter");
            println!("  [3] gtkgreet - Simple GTK greeter");
            println!("  [4] None - Use greetd with agreety (TTY)");

            loop {
                let input = prompt("Select greeter [1-4]: ")?;
                match input.as_str() {
                    "1" => return Ok(Some("regreet".into())),
                    "2" => return Ok(Some("tuigreet".into())),
                    "3" => return Ok(Some("gtkgreet".into())),
                    "4" | "" => return Ok(None),
                    _ => println!("Invalid selection"),
                }
            }
        }
        _ => Ok(None),
    }
}

fn print_summary(config: &InstallConfig) {
    println!("\n=== Summary ===");
    println!("  Device:     {}", config.device.display());
    println!("  Hostname:   {}", config.hostname);
    println!("  Timezone:   {}", config.timezone);
    println!("  Locale:     {}", config.locale);
    println!("  Keymap:     {}", config.keymap);
    println!("  Distro:     {:?}", config.distro);
    println!(
        "  Networking: {}",
        if config.enable_networking {
            "enabled"
        } else {
            "disabled"
        }
    );
    if config.desktop.enabled {
        let seat_mgr = config.desktop.seat_manager.as_deref().unwrap_or("seatd");
        println!("  Desktop:    enabled ({}, polkit)", seat_mgr);
        if let Some(dm) = &config.desktop.display_manager {
            let greeter_str = config
                .desktop
                .greeter
                .as_ref()
                .map(|g| format!(" + {}", g))
                .unwrap_or_default();
            println!("  Display Mgr: {}{}", dm, greeter_str);
        }
        if config.desktop.user_services {
            println!("  User Svcs:  enabled (s6 user supervisor)");
        }
    } else {
        println!("  Desktop:    disabled (console only)");
    }
    // Swap
    let swap_parts: Vec<String> = [
        config
            .swap
            .zram_enabled
            .then(|| format!("zram {}GB", config.swap.zram_size_gb.unwrap_or(8))),
        config
            .swap
            .swapfile_enabled
            .then(|| format!("swapfile {}GB", config.swap.swapfile_size_gb.unwrap_or(8))),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !swap_parts.is_empty() {
        println!(
            "  Swap:       {} (swappiness={})",
            swap_parts.join(" + "),
            config.swap.swappiness
        );
    } else {
        println!("  Swap:       disabled");
    }
    // Audio
    if config.audio.enabled {
        let mut compat = Vec::new();
        if config.audio.pulseaudio_compat {
            compat.push("pulse");
        }
        if config.audio.alsa_compat {
            compat.push("alsa");
        }
        if config.audio.jack_compat {
            compat.push("jack");
        }
        let compat_str = if compat.is_empty() {
            String::new()
        } else {
            format!(" ({})", compat.join("+"))
        };
        println!("  Audio:      pipewire{}", compat_str);
    } else {
        println!("  Audio:      disabled");
    }
    if !config.extra_packages.is_empty() {
        println!("  Extra pkgs: {}", config.extra_packages.join(", "));
    }
    println!(
        "\n⚠ WARNING: This will DESTROY all data on {}\n",
        config.device.display()
    );
}

fn select_device(devices: &[BlockDevice]) -> Result<&BlockDevice> {
    loop {
        let input = prompt(&format!("Select disk [1-{}]: ", devices.len()))?;
        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= devices.len() {
                return Ok(&devices[n - 1]);
            }
        }
        println!("Invalid selection");
    }
}

fn prompt(msg: &str) -> Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut input = String::new();
    let bytes_read = io::stdin().read_line(&mut input)?;

    if bytes_read == 0 {
        bail!("Unexpected end of input. Is stdin connected to a terminal?");
    }

    Ok(input.trim().to_string())
}

fn prompt_default(name: &str, default: &str) -> Result<String> {
    let spec = FieldSpec::text_default("_inline", name, default);
    match prompt::prompt_field(&spec)? {
        FieldValue::Text(s) => Ok(s),
        _ => Ok(default.to_string()),
    }
}

fn prompt_yes_no(name: &str, default: bool) -> Result<bool> {
    prompt::prompt_yes_no(name, default)
}

fn prompt_passphrase() -> Result<String> {
    loop {
        let pass1 = rpassword::prompt_password("Encryption passphrase: ")
            .map_err(|e| anyhow::anyhow!("Failed to read passphrase: {}", e))?;

        if pass1.len() < 8 {
            println!("Passphrase must be at least 8 characters");
            continue;
        }

        let pass2 = rpassword::prompt_password("Confirm passphrase: ")
            .map_err(|e| anyhow::anyhow!("Failed to read passphrase: {}", e))?;

        if pass1 != pass2 {
            println!("Passphrases do not match");
            continue;
        }

        return Ok(pass1);
    }
}

fn prompt_password_confirm(name: &str) -> Result<String> {
    let spec = FieldSpec::password_confirm("_inline", name);
    match prompt::prompt_field(&spec)? {
        FieldValue::Text(s) => Ok(s),
        _ => bail!("Password is required"),
    }
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

fn detect_live_distro() -> Result<DistroKind> {
    use std::fs;
    use std::path::Path;

    // Check distro-specific files
    if Path::new("/etc/artix-release").exists() {
        return Ok(DistroKind::Artix);
    }
    if Path::new("/etc/void-release").exists() {
        return Ok(DistroKind::Void);
    }
    if Path::new("/etc/slackware-version").exists() {
        return Ok(DistroKind::Slackware);
    }
    if Path::new("/etc/alpine-release").exists() {
        return Ok(DistroKind::Alpine);
    }
    if Path::new("/etc/gentoo-release").exists() {
        return Ok(DistroKind::Gentoo);
    }
    if Path::new("/etc/devuan_version").exists() {
        return Ok(DistroKind::Devuan);
    }

    // Check /etc/os-release
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        let content_lower = content.to_lowercase();

        if content_lower.contains("artix") {
            return Ok(DistroKind::Artix);
        }
        if content_lower.contains("void") {
            return Ok(DistroKind::Void);
        }
        if content_lower.contains("slackware") {
            return Ok(DistroKind::Slackware);
        }
        if content_lower.contains("alpine") {
            return Ok(DistroKind::Alpine);
        }
        if content_lower.contains("gentoo") {
            return Ok(DistroKind::Gentoo);
        }
        if content_lower.contains("devuan") {
            return Ok(DistroKind::Devuan);
        }
    }

    bail!("Could not detect distribution")
}

fn distro_name(distro: DistroKind) -> &'static str {
    match distro {
        DistroKind::Artix => "Artix Linux",
        DistroKind::Void => "Void Linux",
        DistroKind::Slackware => "Slackware Linux",
        DistroKind::Alpine => "Alpine Linux",
        DistroKind::Gentoo => "Gentoo Linux",
        DistroKind::Devuan => "Devuan GNU+Linux",
    }
}

fn prompt_distro() -> Result<DistroKind> {
    println!("\nSelect distribution to install:");
    println!("  [1] Artix Linux (systemd-free Arch, s6/runit/OpenRC)");
    println!("  [2] Void Linux (independent, runit, musl or glibc)");
    println!("  [3] Gentoo Linux (source-based, OpenRC)");
    println!("  [4] Alpine Linux (lightweight, musl, OpenRC)");
    println!("  [5] Slackware Linux (oldest active distro, SysVinit)");
    println!("  [6] Devuan GNU+Linux (systemd-free Debian, SysVinit)");

    loop {
        let input = prompt("Select distribution [1-6]: ")?;
        match input.as_str() {
            "1" => return Ok(DistroKind::Artix),
            "2" => return Ok(DistroKind::Void),
            "3" => return Ok(DistroKind::Gentoo),
            "4" => return Ok(DistroKind::Alpine),
            "5" => return Ok(DistroKind::Slackware),
            "6" => return Ok(DistroKind::Devuan),
            _ => println!("Invalid selection. Please enter 1-6."),
        }
    }
}
