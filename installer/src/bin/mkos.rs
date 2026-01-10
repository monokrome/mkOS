use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::process::Command;

use mkos::crypt::snapshot;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "update" => update(),
        "upgrade" | "up" => upgrade(),
        "snapshot" => snapshot_cmd(&args[2..]),
        "migrate-swap" => migrate_swap(),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!(
        r#"mkOS - System management tool

Usage:
    mkos update           Update package indexes
    mkos upgrade          Update indexes and upgrade packages (with snapshot)
    mkos snapshot list    List all snapshots
    mkos snapshot delete <name>  Delete a snapshot
    mkos migrate-swap     Migrate swapfile to @swap subvolume
    mkos help             Show this help message

Examples:
    mkos update           # Update package database only
    mkos upgrade          # Update and upgrade all packages (creates snapshot first)
    mkos snapshot list    # List all available snapshots
    mkos migrate-swap     # Migrate /swapfile to /swap/swapfile in @swap subvolume
"#
    );
}

fn update() -> Result<()> {
    // Check if running as root
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

fn upgrade() -> Result<()> {
    // Check if running as root
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

    create_btrfs_snapshot(&snapshot_name)
        .context("Failed to create snapshot")?;

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

fn create_btrfs_snapshot(name: &str) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Check if there's a swapfile (btrfs can't snapshot subvolumes with active swapfiles)
    let swapfile_active = Command::new("swapon")
        .args(["--show", "--noheadings"])
        .output()
        .ok()
        .and_then(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            Some(output.contains("/swapfile"))
        })
        .unwrap_or(false);

    // Disable swap if needed
    if swapfile_active {
        println!("  Temporarily disabling swap...");
        Command::new("swapoff")
            .arg("/swapfile")
            .status()
            .context("Failed to disable swap")?;
    }

    // Get the root device
    let findmnt_output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "/"])
        .output()
        .context("Failed to find root device")?;

    let mut root_device = String::from_utf8_lossy(&findmnt_output.stdout)
        .trim()
        .to_string();

    // Strip subvolume notation if present
    if let Some(bracket_pos) = root_device.find('[') {
        root_device = root_device[..bracket_pos].to_string();
    }

    // Create temporary mount point for btrfs root
    let temp_mount = PathBuf::from("/tmp/mkos-btrfs-root");
    fs::create_dir_all(&temp_mount)?;

    // Mount btrfs root (subvolid=5)
    let mount_status = Command::new("mount")
        .args(["-o", "subvolid=5", &root_device, temp_mount.to_str().unwrap()])
        .status()
        .context("Failed to mount btrfs root")?;

    if !mount_status.success() {
        let _ = fs::remove_dir(&temp_mount);
        if swapfile_active {
            let _ = Command::new("swapon").arg("/swapfile").status();
        }
        anyhow::bail!("Failed to mount btrfs root");
    }

    // Snapshot @ to @snapshots/name
    let source = temp_mount.join("@");
    let dest = temp_mount.join("@snapshots").join(name);

    let snapshot_status = Command::new("btrfs")
        .args([
            "subvolume",
            "snapshot",
            "-r",
            source.to_str().unwrap(),
            dest.to_str().unwrap(),
        ])
        .status()
        .context("Failed to create snapshot")?;

    // Unmount temporary mount
    let _ = Command::new("umount").arg(&temp_mount).status();
    let _ = fs::remove_dir(&temp_mount);

    // Re-enable swap if it was active
    if swapfile_active {
        println!("  Re-enabling swap...");
        let _ = Command::new("swapon").arg("/swapfile").status();
    }

    if !snapshot_status.success() {
        anyhow::bail!("btrfs snapshot command failed");
    }

    Ok(())
}

fn run_upgrade() -> Result<()> {
    // Detect package manager
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

fn snapshot_cmd(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Error: snapshot subcommand required");
        eprintln!("Usage: mkos snapshot <list|delete>");
        std::process::exit(1);
    }

    match args[0].as_str() {
        "list" | "ls" => list_snapshots(),
        "delete" | "del" | "rm" => {
            if args.len() < 2 {
                eprintln!("Error: snapshot name required");
                eprintln!("Usage: mkos snapshot delete <name>");
                std::process::exit(1);
            }
            delete_snapshot(&args[1])
        }
        _ => {
            eprintln!("Unknown snapshot subcommand: {}", args[0]);
            std::process::exit(1);
        }
    }
}

fn list_snapshots() -> Result<()> {
    let snapshots_dir = Path::new("/.snapshots");
    if !snapshots_dir.exists() {
        println!("No snapshots directory found.");
        return Ok(());
    }

    println!("Available snapshots:\n");

    let entries = std::fs::read_dir(snapshots_dir)?;
    let mut snapshots: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    snapshots.sort_by_key(|e| e.file_name());

    if snapshots.is_empty() {
        println!("  (no snapshots)");
    } else {
        for entry in snapshots {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Get metadata if available
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let datetime: chrono::DateTime<chrono::Local> = modified.into();
                    println!("  {} ({})", name_str, datetime.format("%Y-%m-%d %H:%M:%S"));
                    continue;
                }
            }

            println!("  {}", name_str);
        }
    }

    Ok(())
}

fn delete_snapshot(name: &str) -> Result<()> {
    // Check if running as root
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: Deleting snapshots requires root privileges (use sudo)");
        std::process::exit(1);
    }

    let snapshot_path = Path::new("/.snapshots").join(name);

    if !snapshot_path.exists() {
        anyhow::bail!("Snapshot not found: {}", name);
    }

    println!("Deleting snapshot: {}", name);
    snapshot::delete_snapshot(&snapshot_path)?;
    println!("✓ Snapshot deleted");

    Ok(())
}

fn migrate_swap() -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Check if running as root
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: Migrating swap requires root privileges (use sudo)");
        std::process::exit(1);
    }

    println!("=== mkOS Swap Migration ===\n");

    // Check if /swapfile exists (old setup)
    let old_swapfile = Path::new("/swapfile");
    if !old_swapfile.exists() {
        println!("No /swapfile found. System may already be using @swap subvolume.");
        println!("If you have swap in /swap/swapfile, migration is not needed.\n");
        return Ok(());
    }

    println!("Found /swapfile in root subvolume. Migrating to @swap subvolume...\n");

    // Check if filesystem is btrfs
    if !snapshot::is_btrfs_root() {
        anyhow::bail!("Root filesystem is not btrfs. This migration is only for btrfs systems.");
    }

    // Get the root device
    let findmnt_output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "/"])
        .output()
        .context("Failed to find root device")?;

    let mut root_device = String::from_utf8_lossy(&findmnt_output.stdout)
        .trim()
        .to_string();

    // Strip subvolume notation if present
    if let Some(bracket_pos) = root_device.find('[') {
        root_device = root_device[..bracket_pos].to_string();
    }

    // Disable swap if active
    let swap_active = Command::new("swapon")
        .args(["--show", "--noheadings"])
        .output()
        .ok()
        .and_then(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            Some(output.contains("/swapfile"))
        })
        .unwrap_or(false);

    if swap_active {
        println!("[1/7] Disabling swap...");
        Command::new("swapoff")
            .arg("/swapfile")
            .status()
            .context("Failed to disable swap")?;
        println!("✓ Swap disabled\n");
    } else {
        println!("[1/7] Swap already disabled\n");
    }

    // Mount btrfs root
    println!("[2/7] Mounting btrfs root...");
    let temp_mount = PathBuf::from("/tmp/mkos-btrfs-root");
    fs::create_dir_all(&temp_mount)?;

    let mount_status = Command::new("mount")
        .args(["-o", "subvolid=5", &root_device, temp_mount.to_str().unwrap()])
        .status()
        .context("Failed to mount btrfs root")?;

    if !mount_status.success() {
        let _ = fs::remove_dir(&temp_mount);
        anyhow::bail!("Failed to mount btrfs root");
    }
    println!("✓ Btrfs root mounted at {}\n", temp_mount.display());

    // Check if @swap subvolume exists
    let swap_subvol = temp_mount.join("@swap");
    if !swap_subvol.exists() {
        println!("[3/7] Creating @swap subvolume...");
        let create_status = Command::new("btrfs")
            .args(["subvolume", "create", swap_subvol.to_str().unwrap()])
            .status()
            .context("Failed to create @swap subvolume")?;

        if !create_status.success() {
            let _ = Command::new("umount").arg(&temp_mount).status();
            let _ = fs::remove_dir(&temp_mount);
            anyhow::bail!("Failed to create @swap subvolume");
        }
        println!("✓ Created @swap subvolume\n");
    } else {
        println!("[3/7] @swap subvolume already exists\n");
    }

    // Create /swap mount point if it doesn't exist
    println!("[4/7] Creating /swap mount point...");
    fs::create_dir_all("/swap")?;
    println!("✓ Created /swap directory\n");

    // Mount @swap subvolume at /swap
    println!("[5/7] Mounting @swap subvolume...");
    let swap_mount = Command::new("mount")
        .args(["-o", "subvol=@swap", &root_device, "/swap"])
        .status()
        .context("Failed to mount @swap")?;

    if !swap_mount.success() {
        let _ = Command::new("umount").arg(&temp_mount).status();
        let _ = fs::remove_dir(&temp_mount);
        anyhow::bail!("Failed to mount @swap subvolume");
    }
    println!("✓ @swap mounted at /swap\n");

    // Move swapfile
    println!("[6/7] Moving swapfile...");
    let new_swapfile = Path::new("/swap/swapfile");
    fs::rename("/swapfile", new_swapfile)
        .context("Failed to move swapfile")?;
    println!("✓ Moved /swapfile to /swap/swapfile\n");

    // Update /etc/fstab
    println!("[7/7] Updating /etc/fstab...");
    let fstab_path = Path::new("/etc/fstab");
    let fstab_content = fs::read_to_string(fstab_path)
        .context("Failed to read /etc/fstab")?;

    let mut new_fstab = String::new();
    let mut updated = false;
    let mut has_swap_mount = false;

    for line in fstab_content.lines() {
        if line.trim().starts_with('#') || line.trim().is_empty() {
            new_fstab.push_str(line);
            new_fstab.push('\n');
            continue;
        }

        // Check if this is the old swapfile entry
        if line.contains("/swapfile") && line.contains("swap") {
            // Replace /swapfile with /swap/swapfile
            let new_line = line.replace("/swapfile", "/swap/swapfile");
            new_fstab.push_str(&new_line);
            new_fstab.push('\n');
            updated = true;
            continue;
        }

        // Check if @swap mount entry exists
        if line.contains("@swap") && line.contains("/swap") {
            has_swap_mount = true;
        }

        new_fstab.push_str(line);
        new_fstab.push('\n');
    }

    // Add @swap mount entry if it doesn't exist
    if !has_swap_mount {
        new_fstab.push_str(&format!(
            "{} /swap btrfs subvol=@swap,defaults 0 0\n",
            root_device
        ));
    }

    if !updated {
        println!("Warning: Could not find /swapfile entry in fstab. You may need to add:");
        println!("  /swap/swapfile none swap defaults,pri=10 0 0");
    }

    fs::write(fstab_path, new_fstab)
        .context("Failed to write /etc/fstab")?;
    println!("✓ Updated /etc/fstab\n");

    // Re-enable swap
    println!("Re-enabling swap...");
    let swapon_status = Command::new("swapon")
        .arg("/swap/swapfile")
        .status()
        .context("Failed to enable swap")?;

    // Cleanup
    let _ = Command::new("umount").arg(&temp_mount).status();
    let _ = fs::remove_dir(&temp_mount);

    if !swapon_status.success() {
        println!("\nWarning: Failed to enable swap. You may need to run:");
        println!("  sudo swapon /swap/swapfile");
        println!("\nOr reboot to apply the changes.");
    } else {
        println!("✓ Swap enabled\n");
    }

    println!("════════════════════════════════════════════════════════════");
    println!("✓ Migration complete!");
    println!();
    println!("Your swapfile is now in the @swap subvolume at /swap/swapfile");
    println!("This allows snapshots to work without disabling swap.");
    println!();
    println!("Changes made:");
    println!("  • Created @swap btrfs subvolume");
    println!("  • Moved /swapfile to /swap/swapfile");
    println!("  • Updated /etc/fstab");
    println!("  • Mounted @swap at /swap");
    println!();
    println!("You can now run 'mkos upgrade' without swap-related issues.");
    println!("════════════════════════════════════════════════════════════");

    Ok(())
}
