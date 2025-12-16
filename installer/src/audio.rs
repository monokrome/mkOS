use anyhow::Result;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::distro::Distro;

/// Audio packages to install for PipeWire stack
const AUDIO_PACKAGES: &[&str] = &[
    "pipewire",
    "wireplumber",
];

/// Set up audio (PipeWire + WirePlumber)
pub fn setup_audio(root: &Path, distro: &dyn Distro) -> Result<()> {
    install_audio_packages(root, distro)?;
    setup_user_audio_services(root)?;
    Ok(())
}

/// Install PipeWire audio stack packages
fn install_audio_packages(root: &Path, distro: &dyn Distro) -> Result<()> {
    distro.install_packages(root, AUDIO_PACKAGES)
}

/// Create user service templates in /etc/skel for pipewire and wireplumber
fn setup_user_audio_services(root: &Path) -> Result<()> {
    // Create s6 service directories in /etc/skel
    let skel_sv = root.join("etc/skel/.config/s6/sv");

    // PipeWire service
    let pipewire_dir = skel_sv.join("pipewire");
    std::fs::create_dir_all(&pipewire_dir)?;

    let pipewire_run = r#"#!/bin/sh
exec pipewire
"#;
    let pipewire_run_path = pipewire_dir.join("run");
    std::fs::write(&pipewire_run_path, pipewire_run)?;
    std::fs::set_permissions(&pipewire_run_path, std::fs::Permissions::from_mode(0o755))?;

    // WirePlumber service
    let wireplumber_dir = skel_sv.join("wireplumber");
    std::fs::create_dir_all(&wireplumber_dir)?;

    let wireplumber_run = r#"#!/bin/sh
# Wait for pipewire socket
while [ ! -e "${XDG_RUNTIME_DIR}/pipewire-0" ]; do
    sleep 0.1
done
exec wireplumber
"#;
    let wireplumber_run_path = wireplumber_dir.join("run");
    std::fs::write(&wireplumber_run_path, wireplumber_run)?;
    std::fs::set_permissions(&wireplumber_run_path, std::fs::Permissions::from_mode(0o755))?;

    Ok(())
}
