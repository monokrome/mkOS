//! Network services setup (mDNS, SSH, Eternal Terminal)

use anyhow::{Context, Result};
use std::path::Path;

use crate::distro::Distro;
use crate::manifest::{EtConfig, NetworkConfig};

/// Set up network services based on configuration
pub fn setup_network(root: &Path, config: &NetworkConfig, distro: &dyn Distro) -> Result<()> {
    if config.mdns {
        println!("  Setting up mDNS (Avahi)...");
        setup_mdns(root, distro)?;
    }

    if let Some(ssh) = &config.ssh {
        if ssh.enabled {
            println!("  Setting up SSH server...");
            setup_ssh(root, distro)?;
        }
    }

    if let Some(et) = &config.eternalterminal {
        if et.enabled {
            println!("  Setting up Eternal Terminal...");
            setup_eternalterminal(root, et, distro)?;
        }
    }

    Ok(())
}

/// Install and enable Avahi for mDNS (.local hostname resolution)
fn setup_mdns(root: &Path, distro: &dyn Distro) -> Result<()> {
    // Install packages
    distro.install_packages(root, &["avahi", "nss-mdns"])?;

    // Enable avahi service
    let service = distro.map_service("avahi");
    distro
        .init_system()
        .enable_service(root, &service)
        .context("Failed to enable avahi service")?;

    Ok(())
}

/// Install and enable SSH server
fn setup_ssh(root: &Path, distro: &dyn Distro) -> Result<()> {
    // Install openssh package
    distro.install_packages(root, &["openssh"])?;

    // Enable sshd service
    let service = distro.map_service("sshd");
    distro
        .init_system()
        .enable_service(root, &service)
        .context("Failed to enable sshd service")?;

    Ok(())
}

/// Install and configure Eternal Terminal
fn setup_eternalterminal(root: &Path, config: &EtConfig, distro: &dyn Distro) -> Result<()> {
    // Install eternalterminal package
    distro.install_packages(root, &["eternalterminal"])?;

    // Write configuration file
    let et_config = format!(
        "[Networking]\n\
         Port = {}\n",
        config.port
    );

    std::fs::write(root.join("etc/et.cfg"), et_config)
        .context("Failed to write /etc/et.cfg")?;

    // Enable etserver service
    let service = distro.map_service("etserver");
    distro
        .init_system()
        .enable_service(root, &service)
        .context("Failed to enable etserver service")?;

    Ok(())
}

/// Check if any network services are enabled
pub fn has_network_services(config: &NetworkConfig) -> bool {
    config.mdns
        || config.ssh.as_ref().map(|s| s.enabled).unwrap_or(false)
        || config
            .eternalterminal
            .as_ref()
            .map(|e| e.enabled)
            .unwrap_or(false)
}
