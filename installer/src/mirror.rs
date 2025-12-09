use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub struct Mirror {
    pub name: String,
    pub url: String,
}

pub fn parse_mirrorlist(path: &str) -> Result<Vec<Mirror>> {
    let content = fs::read_to_string(path).context("Failed to read mirrorlist")?;

    let mut mirrors = Vec::new();
    let mut current_name = String::new();

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('#') {
            // Comment line - might be a server name
            let comment = line.trim_start_matches('#').trim();
            if !comment.is_empty() && !comment.starts_with("Server") {
                current_name = comment.to_string();
            }
        } else if line.starts_with("Server") {
            // Server line: Server = https://...
            if let Some(url) = line.split('=').nth(1) {
                let url = url.trim().to_string();
                let name = if current_name.is_empty() {
                    // Extract domain from URL as name
                    url.split("//")
                        .nth(1)
                        .and_then(|s| s.split('/').next())
                        .unwrap_or("Unknown")
                        .to_string()
                } else {
                    current_name.clone()
                };

                mirrors.push(Mirror { name, url });
                current_name.clear();
            }
        }
    }

    Ok(mirrors)
}

pub fn select_mirror(mirrors: &[Mirror]) -> Result<&Mirror> {
    if mirrors.is_empty() {
        anyhow::bail!("No mirrors available");
    }

    let mut filtered: Vec<(usize, &Mirror)> = mirrors.iter().enumerate().collect();

    loop {
        println!("\nAvailable mirrors:");
        for (idx, (_orig_idx, mirror)) in filtered.iter().enumerate() {
            println!("  [{:2}] {}", idx + 1, mirror.name);
        }

        print!("\nSelect [1-{}], Enter=first, /search: ", filtered.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(filtered[0].1);
        }

        if let Some(filter) = input.strip_prefix('/') {
            let filter_lower = filter.to_lowercase();
            filtered = mirrors
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    m.name.to_lowercase().contains(&filter_lower)
                        || m.url.to_lowercase().contains(&filter_lower)
                })
                .collect();

            if filtered.is_empty() {
                println!("No mirrors match '{}', showing all", filter);
                filtered = mirrors.iter().enumerate().collect();
            }
            continue;
        }

        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= filtered.len() {
                return Ok(filtered[n - 1].1);
            }
        }

        println!("Invalid selection");
    }
}

pub fn write_mirrorlist(path: &str, mirror: &Mirror) -> Result<()> {
    let content = format!(
        "# mkOS selected mirror: {}\nServer = {}\n",
        mirror.name, mirror.url
    );

    fs::write(path, content).context("Failed to write mirrorlist")?;

    Ok(())
}

pub fn setup_mirror() -> Result<()> {
    let mirrorlist_path = "/etc/pacman.d/mirrorlist";

    let mirrors = parse_mirrorlist(mirrorlist_path)?;

    if mirrors.is_empty() {
        println!("No mirrors found in mirrorlist, using default");
        return Ok(());
    }

    let selected = select_mirror(&mirrors)?;
    println!("\nSelected: {}", selected.name);

    write_mirrorlist(mirrorlist_path, selected)?;

    Ok(())
}
