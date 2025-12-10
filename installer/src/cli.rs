use anyhow::{bail, Context, Result};
use rpassword;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::disk::{self, BlockDevice};
use crate::distro::DistroKind;
use crate::install::{DesktopConfig, InstallConfig, Installer};
use crate::manifest::{self, Manifest, ManifestBundle, ManifestSource};
use crate::mirror;

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
    println!("  - Artix Linux (s6 init)");
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

    // Get distro from manifest
    let distro = match manifest.distro.as_str() {
        "artix" => DistroKind::Artix,
        "void" => DistroKind::Void,
        other => bail!("Unknown distro: {}. Supported: artix, void", other),
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

    // Desktop environment setup
    let desktop = prompt_desktop_config()?;

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
    })
}

fn prompt_desktop_config() -> Result<DesktopConfig> {
    println!("\n=== Desktop Environment ===");

    if !prompt_yes_no("Install graphical session support", false)? {
        return Ok(DesktopConfig::default());
    }

    println!("  Will install: seatd, polkit, xdg-utils");

    // Ask about display manager
    let display_manager = prompt_display_manager()?;
    let greeter = if display_manager.is_some() {
        prompt_greeter(display_manager.as_deref())?
    } else {
        None
    };

    Ok(DesktopConfig {
        enabled: true,
        display_manager,
        greeter,
    })
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
        println!("  Desktop:    enabled (seatd, polkit)");
        if let Some(dm) = &config.desktop.display_manager {
            let greeter_str = config
                .desktop
                .greeter
                .as_ref()
                .map(|g| format!(" + {}", g))
                .unwrap_or_default();
            println!("  Display Mgr: {}{}", dm, greeter_str);
        }
    } else {
        println!("  Desktop:    disabled (console only)");
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

    // If EOF is reached (bytes_read == 0), fail fast instead of looping
    if bytes_read == 0 {
        bail!("Unexpected end of input. Is stdin connected to a terminal?");
    }

    Ok(input.trim().to_string())
}

fn prompt_default(name: &str, default: &str) -> Result<String> {
    let input = prompt(&format!("{} [{}]: ", name, default))?;
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

fn prompt_yes_no(name: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    let input = prompt(&format!("{} [{}]: ", name, default_str))?;
    let input_lower = input.to_lowercase();

    if input_lower.is_empty() {
        Ok(default)
    } else if input_lower == "y" || input_lower == "yes" {
        Ok(true)
    } else if input_lower == "n" || input_lower == "no" {
        Ok(false)
    } else {
        Ok(default)
    }
}

fn prompt_passphrase() -> Result<String> {
    loop {
        let pass1 = rpassword::prompt_password("Encryption passphrase: ")
            .context("Failed to read passphrase")?;

        if pass1.len() < 8 {
            println!("Passphrase must be at least 8 characters");
            continue;
        }

        let pass2 = rpassword::prompt_password("Confirm passphrase: ")
            .context("Failed to read passphrase")?;

        if pass1 != pass2 {
            println!("Passphrases do not match");
            continue;
        }

        return Ok(pass1);
    }
}

fn prompt_password_confirm(name: &str) -> Result<String> {
    loop {
        let pass1 =
            rpassword::prompt_password(format!("{}: ", name)).context("Failed to read password")?;

        if pass1.is_empty() {
            println!("Password cannot be empty");
            continue;
        }

        let pass2 = rpassword::prompt_password(format!("Confirm {}: ", name.to_lowercase()))
            .context("Failed to read password")?;

        if pass1 != pass2 {
            println!("Passwords do not match");
            continue;
        }

        return Ok(pass1);
    }
}
