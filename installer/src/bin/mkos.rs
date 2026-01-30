use anyhow::{bail, Result};
use std::env;

use mkos::commands::{rollback, snapshot, update};
use mkos::manifest::ManifestSource;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "update" => update::update(),
        "upgrade" | "up" => update::upgrade(),
        "rollback" => rollback::rollback(),
        "snapshot" => snapshot::snapshot_cmd(&args[2..]),
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
    mkos rollback         Restore system to current snapshot (use when booted to fallback)
    mkos apply <manifest> Apply manifest to system (with snapshot)
    mkos snapshot list    List all snapshots
    mkos snapshot delete <name>  Delete a snapshot
    mkos help             Show this help message

Examples:
    mkos update           # Update package database only
    mkos upgrade          # Update and upgrade all packages (creates snapshot first)
    mkos rollback         # Restore system from fallback snapshot (when main system is broken)
    mkos apply config.yml # Apply configuration from manifest file
    mkos snapshot list    # List all available snapshots
"#
    );
}

fn apply(args: &[String]) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("Error: mkos apply must be run as root (use sudo)");
        std::process::exit(1);
    }

    let source = ManifestSource::from_arg(args.first().map(|s| s.as_str()));

    if matches!(source, ManifestSource::Interactive) {
        bail!("mkos apply requires a manifest. Usage: mkos apply <manifest>");
    }

    mkos::apply::run(source)
}
