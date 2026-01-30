use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn update() -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: mkos update must be run as root (use sudo)");
        std::process::exit(1);
    }

    println!("=== mkOS Update Package Indexes ===\n");

    // Detect package manager and run update
    let (pkg_mgr, args) = if Path::new("/usr/bin/pacman").exists() {
        ("pacman", vec!["-Sy"])
    } else if Path::new("/usr/bin/xbps-install").exists() {
        ("xbps-install", vec!["-S"])
    } else {
        eprintln!("Error: Unknown package manager");
        std::process::exit(1);
    };

    println!("Running {} {}...\n", pkg_mgr, args.join(" "));

    let status = Command::new(pkg_mgr)
        .args(&args)
        .status()
        .context("Failed to run package manager")?;

    if !status.success() {
        anyhow::bail!("Package manager failed with exit code: {:?}", status.code());
    }

    println!("\n✓ Package indexes updated");

    Ok(())
}

pub fn upgrade() -> Result<()> {
    use crate::crypt::snapshot;

    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: mkos upgrade must be run as root (use sudo)");
        std::process::exit(1);
    }

    println!("=== mkOS System Upgrade ===\n");

    // Check if filesystem is btrfs
    if !snapshot::is_btrfs_root() {
        println!("Warning: Root filesystem is not btrfs, skipping snapshot.\n");
        return run_upgrade();
    }

    // Create pre-upgrade snapshot
    println!("Creating pre-upgrade snapshot...");

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S");
    let snapshot_name = format!("pre-upgrade-{}", timestamp);

    super::snapshot::create_btrfs_snapshot(&snapshot_name).context("Failed to create snapshot")?;

    println!("✓ Created snapshot: {}\n", snapshot_name);

    // Run the upgrade
    let result = run_upgrade();

    if result.is_ok() {
        println!("\n✓ Upgrade completed successfully!");
        println!("  Snapshot available at: /.snapshots/{}", snapshot_name);
    } else {
        println!("\n✗ Upgrade failed!");
        println!("  You can restore from: /.snapshots/{}", snapshot_name);
    }

    result
}

fn run_upgrade() -> Result<()> {
    let (pkg_mgr, args) = if Path::new("/usr/bin/pacman").exists() {
        ("pacman", vec!["-Syu"])
    } else if Path::new("/usr/bin/xbps-install").exists() {
        ("xbps-install", vec!["-Su"])
    } else {
        eprintln!("Error: Unknown package manager");
        std::process::exit(1);
    };

    println!("Running {} {}...\n", pkg_mgr, args.join(" "));

    let status = Command::new(pkg_mgr)
        .args(&args)
        .status()
        .context("Failed to run package manager")?;

    if !status.success() {
        anyhow::bail!("Package manager failed with exit code: {:?}", status.code());
    }

    Ok(())
}
