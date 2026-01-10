use anyhow::{bail, Context, Result};
use std::env;
use std::path::Path;
use std::process::Command;

use mkos::crypt::snapshot;
use mkos::manifest::ManifestSource;

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
        "apply" => apply(&args[2..]),
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
    mkos apply <manifest> Apply manifest to system (with snapshot)
    mkos snapshot list    List all snapshots
    mkos snapshot delete <name>  Delete a snapshot
    mkos help             Show this help message

Examples:
    mkos update           # Update package database only
    mkos upgrade          # Update and upgrade all packages (creates snapshot first)
    mkos apply config.yml # Apply configuration from manifest file
    mkos snapshot list    # List all available snapshots
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

    create_btrfs_snapshot(&snapshot_name).context("Failed to create snapshot")?;

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
        .map(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            output.contains("/swapfile")
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
        .args([
            "-o",
            "subvolid=5",
            &root_device,
            temp_mount.to_str().unwrap(),
        ])
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

fn apply(args: &[String]) -> Result<()> {
    // Check if running as root
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: mkos apply must be run as root (use sudo)");
        std::process::exit(1);
    }

    let source = ManifestSource::from_arg(args.first().map(|s| s.as_str()));

    // Validate that a manifest was provided
    if matches!(source, ManifestSource::Interactive) {
        bail!("mkos apply requires a manifest. Usage: mkos apply <manifest>");
    }

    mkos::apply::run(source)
}
