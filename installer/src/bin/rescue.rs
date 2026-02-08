use anyhow::{bail, Result};
use std::path::PathBuf;

use mkos::prompt;
use mkos::rescue;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Check root
    if !nix::unistd::geteuid().is_root() {
        bail!("mkos-rescue must be run as root");
    }

    let args: Vec<String> = std::env::args().collect();
    let (efi_partition, luks_partition) = match args.len() {
        1 => detect_partitions()?,
        3 => (PathBuf::from(&args[1]), PathBuf::from(&args[2])),
        _ => {
            eprintln!("Usage: mkos-rescue [EFI_PARTITION LUKS_PARTITION]");
            eprintln!("  If no arguments given, partitions are auto-detected.");
            std::process::exit(1);
        }
    };

    println!(
        "EFI partition: {}\nLUKS partition: {}",
        efi_partition.display(),
        luks_partition.display()
    );

    // Prompt for LUKS passphrase
    let passphrase = rpassword::prompt_password("Enter LUKS passphrase: ")?;

    // Mount and enter chroot
    rescue::mount_system(&efi_partition, &luks_partition, &passphrase)?;
    rescue::enter_chroot()?;
    rescue::cleanup()?;

    Ok(())
}

fn detect_partitions() -> Result<(PathBuf, PathBuf)> {
    println!("Detecting partitions...\n");

    let luks_devices = rescue::detect_luks_partitions()?;
    let efi_devices = rescue::detect_efi_partitions()?;

    if luks_devices.is_empty() {
        bail!("No LUKS partitions found. Is the disk connected?");
    }
    if efi_devices.is_empty() {
        bail!("No EFI partitions found. Is the disk connected?");
    }

    let luks_idx = if luks_devices.len() == 1 {
        println!(
            "Found LUKS partition: {} [{}]",
            luks_devices[0].path.display(),
            luks_devices[0].size
        );
        0
    } else {
        rescue::select_device(&luks_devices, "LUKS")?
    };

    let efi_idx = if efi_devices.len() == 1 {
        println!(
            "Found EFI partition: {} [{}]",
            efi_devices[0].path.display(),
            efi_devices[0].size
        );
        0
    } else {
        rescue::select_device(&efi_devices, "EFI")?
    };

    // Confirm
    println!();
    if !prompt::prompt_yes_no(
        &format!(
            "Use {} as LUKS and {} as EFI?",
            luks_devices[luks_idx].path.display(),
            efi_devices[efi_idx].path.display()
        ),
        true,
    )? {
        bail!("Aborted by user");
    }

    Ok((
        efi_devices[efi_idx].path.clone(),
        luks_devices[luks_idx].path.clone(),
    ))
}
