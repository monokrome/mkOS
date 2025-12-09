use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tracing_subscriber::EnvFilter;

use mkos::cmd::run as run_cmd;
use mkos::crypt::snapshot::create_pre_apply_snapshot;
use mkos::distro::{get_distro, DistroKind};
use mkos::manifest::{self, Manifest, ManifestSource};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    run()
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let source = ManifestSource::from_arg(args.get(1).map(|s| s.as_str()));

    println!("\n=== mkOS Apply ===\n");
    println!("Applying manifest to existing system...\n");

    // Load manifest
    let bundle = match &source {
        ManifestSource::Interactive => {
            bail!("mkos-apply requires a manifest. Usage: mkos-apply <manifest>");
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
    let files_dir = bundle.files_dir;

    // Create snapshot before making changes
    match create_pre_apply_snapshot() {
        Ok(Some(name)) => println!("Created snapshot: {}\n", name),
        Ok(None) => println!("Skipping snapshot (not btrfs)\n"),
        Err(e) => println!("Warning: Could not create snapshot: {}\n", e),
    }

    // Detect distro
    let distro_kind = detect_distro()?;
    let distro = get_distro(distro_kind);

    // Apply system configuration
    apply_system_config(&manifest)?;

    // Install packages
    apply_packages(&manifest, distro.as_ref())?;

    // Apply services
    apply_services(&manifest, distro.as_ref())?;

    // Apply users
    apply_users(&manifest)?;

    // Apply files
    apply_files(&manifest, files_dir.as_deref())?;

    // Run post-apply scripts
    run_scripts(&manifest.scripts.post_apply)?;

    println!("\n=== Apply Complete ===\n");
    println!("System has been updated to match the manifest.\n");

    Ok(())
}

fn detect_distro() -> Result<DistroKind> {
    if Path::new("/etc/artix-release").exists() {
        return Ok(DistroKind::Artix);
    }
    if Path::new("/etc/void-release").exists() {
        return Ok(DistroKind::Void);
    }

    // Check /etc/os-release
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        if content.contains("artix") || content.contains("Artix") {
            return Ok(DistroKind::Artix);
        }
        if content.contains("void") || content.contains("Void") {
            return Ok(DistroKind::Void);
        }
    }

    bail!("Could not detect distro. Supported: Artix, Void");
}

fn apply_system_config(manifest: &Manifest) -> Result<()> {
    println!("Applying system configuration...");

    // Hostname
    let current_hostname = fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();

    if current_hostname != manifest.system.hostname {
        println!("  Setting hostname: {}", manifest.system.hostname);
        fs::write("/etc/hostname", format!("{}\n", manifest.system.hostname))
            .context("Failed to write /etc/hostname")?;

        run_cmd("hostname", [&manifest.system.hostname])?;
    }

    // Timezone
    let tz_path = format!("/usr/share/zoneinfo/{}", manifest.system.timezone);
    if Path::new(&tz_path).exists() {
        println!("  Setting timezone: {}", manifest.system.timezone);
        let _ = fs::remove_file("/etc/localtime");
        std::os::unix::fs::symlink(&tz_path, "/etc/localtime")
            .context("Failed to symlink timezone")?;
    }

    // Locale
    let locale_gen = Path::new("/etc/locale.gen");
    if locale_gen.exists() {
        let content = fs::read_to_string(locale_gen)?;
        let locale_line = format!("{} UTF-8", manifest.system.locale);
        if !content.contains(&locale_line) || content.contains(&format!("#{}", locale_line)) {
            println!("  Setting locale: {}", manifest.system.locale);
            let new_content = content
                .lines()
                .map(|line| {
                    if line.trim().starts_with('#') && line.contains(&manifest.system.locale) {
                        line.trim_start_matches('#').trim().to_string()
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            fs::write(locale_gen, new_content)?;
            let _ = run_cmd("locale-gen", &[] as &[&str]);
        }
    }

    // Keymap
    let vconsole = "/etc/vconsole.conf";
    let keymap_line = format!("KEYMAP={}", manifest.system.keymap);
    if let Ok(content) = fs::read_to_string(vconsole) {
        if !content.contains(&keymap_line) {
            println!("  Setting keymap: {}", manifest.system.keymap);
            let new_content = content
                .lines()
                .filter(|l| !l.starts_with("KEYMAP="))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
                + &keymap_line
                + "\n";
            fs::write(vconsole, new_content)?;
        }
    } else {
        fs::write(vconsole, format!("{}\n", keymap_line))?;
    }

    Ok(())
}

fn apply_packages(manifest: &Manifest, distro: &dyn mkos::distro::Distro) -> Result<()> {
    let packages: Vec<&str> = manifest
        .packages
        .values()
        .flat_map(|pkgs| pkgs.iter().map(|s| s.as_str()))
        .collect();

    if packages.is_empty() {
        return Ok(());
    }

    println!("Installing packages ({} total)...", packages.len());

    // Get currently installed packages (simplified)
    let installed = get_installed_packages(distro)?;
    let to_install: Vec<&str> = packages
        .iter()
        .filter(|p| !installed.contains(**p))
        .copied()
        .collect();

    if to_install.is_empty() {
        println!("  All packages already installed");
        return Ok(());
    }

    println!("  Installing {} new packages...", to_install.len());
    distro.install_packages(Path::new("/"), &to_install)?;

    Ok(())
}

fn get_installed_packages(distro: &dyn mkos::distro::Distro) -> Result<HashSet<String>> {
    let mut installed = HashSet::new();

    let output = match distro.pkg_manager() {
        "pacman" => Command::new("pacman").args(["-Qq"]).output(),
        "xbps-install" => Command::new("xbps-query").args(["-l"]).output(),
        _ => return Ok(installed),
    };

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // xbps-query output: "ii package-1.0_1  description"
            // pacman output: "package"
            let pkg = line.split_whitespace().next().unwrap_or("");
            if !pkg.is_empty() {
                installed.insert(pkg.to_string());
            }
        }
    }

    Ok(installed)
}

fn apply_services(manifest: &Manifest, distro: &dyn mkos::distro::Distro) -> Result<()> {
    if manifest.services.enable.is_empty() && manifest.services.disable.is_empty() {
        return Ok(());
    }

    println!("Configuring services...");

    for service in &manifest.services.enable {
        println!("  Enabling: {}", service);
        distro.enable_service(Path::new("/"), service)?;
    }

    for service in &manifest.services.disable {
        println!("  Disabling: {}", service);
        // TODO: Add disable_service to Distro trait
        // For now, just warn - most users only enable services
        println!("    Warning: Service disabling not yet implemented");
    }

    Ok(())
}

fn apply_users(manifest: &Manifest) -> Result<()> {
    if manifest.users.is_empty() {
        return Ok(());
    }

    println!("Configuring users...");

    for (username, config) in &manifest.users {
        println!("  User: {}", username);

        // Check if user exists
        let user_exists = Command::new("id")
            .arg(username)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !user_exists {
            // Create user
            let mut args = vec!["-m".to_string()];

            if let Some(home) = &config.home {
                args.push("-d".into());
                args.push(home.clone());
            }

            args.push("-s".into());
            args.push(config.shell.clone());

            if !config.groups.is_empty() {
                args.push("-G".into());
                args.push(config.groups.join(","));
            }

            args.push(username.clone());

            println!("    Creating user...");
            run_cmd(
                "useradd",
                args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )?;
        } else {
            // Modify existing user
            let mut args = vec![];

            args.push("-s".into());
            args.push(config.shell.clone());

            if !config.groups.is_empty() {
                args.push("-G".into());
                args.push(config.groups.join(","));
            }

            args.push(username.clone());

            println!("    Updating user...");
            run_cmd(
                "usermod",
                args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )?;
        }

        // Add SSH keys
        if !config.ssh_keys.is_empty() {
            let home = config
                .home
                .clone()
                .unwrap_or_else(|| format!("/home/{}", username));
            let ssh_dir = format!("{}/.ssh", home);
            let auth_keys = format!("{}/authorized_keys", ssh_dir);

            fs::create_dir_all(&ssh_dir)?;
            let keys = config.ssh_keys.join("\n") + "\n";
            fs::write(&auth_keys, keys)?;

            // Set permissions
            fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700))?;
            fs::set_permissions(&auth_keys, fs::Permissions::from_mode(0o600))?;

            // Set ownership
            run_cmd(
                "chown",
                ["-R", &format!("{}:{}", username, username), &ssh_dir],
            )?;
        }
    }

    Ok(())
}

fn apply_files(manifest: &Manifest, files_dir: Option<&Path>) -> Result<()> {
    if manifest.files.is_empty() {
        return Ok(());
    }

    println!("Deploying files...");

    for file in &manifest.files {
        println!("  {}", file.path);

        // Create parent directories
        if let Some(parent) = Path::new(&file.path).parent() {
            fs::create_dir_all(parent)?;
        }

        // Get content
        let content = if let Some(content) = &file.content {
            content.clone()
        } else if let Some(source) = &file.source {
            // Source is relative to files_dir (from tar) or absolute
            let source_path = if let Some(base) = files_dir {
                base.join(source)
            } else {
                Path::new(source).to_path_buf()
            };

            fs::read_to_string(&source_path)
                .with_context(|| format!("Failed to read source file: {}", source_path.display()))?
        } else {
            bail!("File {} has no content or source", file.path);
        };

        fs::write(&file.path, content)?;

        // Set mode if specified
        if let Some(mode) = &file.mode {
            let mode_int = u32::from_str_radix(mode.trim_start_matches('0'), 8)
                .context("Invalid file mode")?;
            fs::set_permissions(&file.path, fs::Permissions::from_mode(mode_int))?;
        }

        // Set ownership if specified
        if file.owner.is_some() || file.group.is_some() {
            let owner = file.owner.as_deref().unwrap_or("");
            let group = file.group.as_deref().unwrap_or("");
            let ownership = if !owner.is_empty() && !group.is_empty() {
                format!("{}:{}", owner, group)
            } else if !owner.is_empty() {
                owner.to_string()
            } else {
                format!(":{}", group)
            };

            run_cmd("chown", [&ownership, &file.path])?;
        }
    }

    Ok(())
}

fn run_scripts(scripts: &[String]) -> Result<()> {
    if scripts.is_empty() {
        return Ok(());
    }

    println!("Running post-apply scripts...");

    for script in scripts {
        println!(
            "  Executing: {}...",
            script.lines().next().unwrap_or("(script)")
        );
        run_cmd("sh", ["-c", script])?;
    }

    Ok(())
}
