//! Audio setup using PipeWire stack

use anyhow::Result;
use std::path::Path;

use crate::distro::Distro;
use crate::init::ServiceSpec;
use crate::manifest::AudioConfig;

/// Set up audio (PipeWire + WirePlumber) based on configuration
pub fn setup_audio(root: &Path, config: &AudioConfig, distro: &dyn Distro) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    install_audio_packages(root, config, distro)?;
    setup_user_audio_services(root, config, distro)?;
    Ok(())
}

/// Install PipeWire audio stack packages based on configuration
fn install_audio_packages(root: &Path, config: &AudioConfig, distro: &dyn Distro) -> Result<()> {
    // Core packages (always installed when audio is enabled)
    let mut packages = vec!["pipewire", "wireplumber"];

    // Optional compatibility layers
    if config.pulseaudio_compat {
        packages.push("pipewire-pulse");
    }
    if config.alsa_compat {
        packages.push("pipewire-alsa");
    }
    if config.jack_compat {
        packages.push("pipewire-jack");
    }

    distro.install_packages(root, &packages)
}

/// Create user service templates for audio services
fn setup_user_audio_services(root: &Path, config: &AudioConfig, distro: &dyn Distro) -> Result<()> {
    let init = distro.init_system();

    // PipeWire service (always created)
    let pipewire = ServiceSpec::longrun("pipewire", "pipewire");
    init.create_user_service(root, &pipewire)?;

    // WirePlumber service (always created, waits for pipewire)
    let wireplumber = ServiceSpec::longrun("wireplumber", "wireplumber")
        .wait_for("${XDG_RUNTIME_DIR}/pipewire-0");
    init.create_user_service(root, &wireplumber)?;

    // pipewire-pulse service (optional, waits for pipewire)
    if config.pulseaudio_compat {
        let pipewire_pulse = ServiceSpec::longrun("pipewire-pulse", "pipewire-pulse")
            .wait_for("${XDG_RUNTIME_DIR}/pipewire-0");
        init.create_user_service(root, &pipewire_pulse)?;
    }

    Ok(())
}
