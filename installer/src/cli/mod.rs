mod config;
mod gpu;
mod prompts;

use anyhow::Result;
use std::env;

use crate::install::{InstallConfig, Installer};
use crate::manifest::{self, Manifest, ManifestBundle, ManifestSource};
use crate::mirror;

use config::build_config;
use prompts::prompt_raw;

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

    let confirm = prompt_raw("Type 'yes' to continue: ")?;
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
