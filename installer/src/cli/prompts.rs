use anyhow::{bail, Result};
use std::io::{self, Write};

use crate::disk::BlockDevice;
use crate::distro::DistroKind;
use crate::prompt::{self, FieldSpec, FieldValue};

pub fn prompt_seat_manager() -> Result<Option<String>> {
    println!("\nSeat manager options:");
    println!("  [1] seatd - Minimal seat management (recommended)");
    println!("  [2] elogind - Full session management (seat, login, power)");

    loop {
        let input = prompt_raw("Select seat manager [1-2]: ")?;
        match input.as_str() {
            "1" | "" => return Ok(None), // None = default to seatd
            "2" => return Ok(Some("elogind".into())),
            _ => println!("Invalid selection"),
        }
    }
}

pub fn prompt_display_manager() -> Result<Option<String>> {
    println!("\nDisplay manager options:");
    println!("  [1] greetd - Minimal, flexible login daemon");
    println!("  [2] ly - TUI display manager");
    println!("  [3] None - Start session manually or via ~/.profile");

    loop {
        let input = prompt_raw("Select display manager [1-3]: ")?;
        match input.as_str() {
            "1" => return Ok(Some("greetd".into())),
            "2" => return Ok(Some("ly".into())),
            "3" | "" => return Ok(None),
            _ => println!("Invalid selection"),
        }
    }
}

pub fn prompt_greeter(dm: Option<&str>) -> Result<Option<String>> {
    match dm {
        Some("greetd") => {
            println!("\nGreeter options for greetd:");
            println!("  [1] regreet - GTK4 graphical greeter (requires cage)");
            println!("  [2] tuigreet - Terminal-based greeter");
            println!("  [3] gtkgreet - Simple GTK greeter");
            println!("  [4] None - Use greetd with agreety (TTY)");

            loop {
                let input = prompt_raw("Select greeter [1-4]: ")?;
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

pub fn select_device(devices: &[BlockDevice]) -> Result<&BlockDevice> {
    loop {
        let input = prompt_raw(&format!("Select disk [1-{}]: ", devices.len()))?;
        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= devices.len() {
                return Ok(&devices[n - 1]);
            }
        }
        println!("Invalid selection");
    }
}

pub fn prompt_raw(msg: &str) -> Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut input = String::new();
    let bytes_read = io::stdin().read_line(&mut input)?;

    if bytes_read == 0 {
        bail!("Unexpected end of input. Is stdin connected to a terminal?");
    }

    Ok(input.trim().to_string())
}

pub fn prompt_default(name: &str, default: &str) -> Result<String> {
    let spec = FieldSpec::text_default("_inline", name, default);
    match prompt::prompt_field(&spec)? {
        FieldValue::Text(s) => Ok(s),
        _ => Ok(default.to_string()),
    }
}

pub fn prompt_yes_no(name: &str, default: bool) -> Result<bool> {
    prompt::prompt_yes_no(name, default)
}

pub fn prompt_passphrase() -> Result<String> {
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

pub fn prompt_password_confirm(name: &str) -> Result<String> {
    let spec = FieldSpec::password_confirm("_inline", name);
    match prompt::prompt_field(&spec)? {
        FieldValue::Text(s) => Ok(s),
        _ => bail!("Password is required"),
    }
}

pub fn prompt_distro() -> Result<DistroKind> {
    println!("\nSelect distribution to install:");
    println!("  [1] Artix Linux (systemd-free Arch, s6/runit/OpenRC)");
    println!("  [2] Void Linux (independent, runit, musl or glibc)");
    println!("  [3] Gentoo Linux (source-based, OpenRC)");
    println!("  [4] Alpine Linux (lightweight, musl, OpenRC)");
    println!("  [5] Slackware Linux (oldest active distro, SysVinit)");
    println!("  [6] Devuan GNU+Linux (systemd-free Debian, SysVinit)");

    loop {
        let input = prompt_raw("Select distribution [1-6]: ")?;
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
