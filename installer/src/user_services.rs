use anyhow::Result;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Set up user-level s6 supervisor infrastructure
pub fn setup_user_s6(root: &Path) -> Result<()> {
    setup_skel_structure(root)?;
    setup_profile_script(root)?;
    Ok(())
}

/// Create the s6 service directory structure in /etc/skel
fn setup_skel_structure(root: &Path) -> Result<()> {
    let skel_s6 = root.join("etc/skel/.config/s6");

    // Create service directory (where individual services live)
    std::fs::create_dir_all(skel_s6.join("sv"))?;

    // Create log directory for s6-svscan
    std::fs::create_dir_all(skel_s6.join("log"))?;

    Ok(())
}

/// Create profile.d script to start user s6-svscan on login
fn setup_profile_script(root: &Path) -> Result<()> {
    let profile_d = root.join("etc/profile.d");
    std::fs::create_dir_all(&profile_d)?;

    // Script to start s6-svscan for the user on login
    let script = r#"#!/bin/sh
# Start user-level s6 supervisor if not already running
# Only run in interactive shells with XDG_RUNTIME_DIR set

if [ -n "$XDG_RUNTIME_DIR" ] && [ -d "$HOME/.config/s6/sv" ]; then
    S6_SCANDIR="$HOME/.config/s6/sv"
    S6_PIDFILE="$XDG_RUNTIME_DIR/s6-svscan.pid"

    # Check if already running
    if [ -f "$S6_PIDFILE" ] && kill -0 "$(cat "$S6_PIDFILE")" 2>/dev/null; then
        return 0
    fi

    # Start s6-svscan in background
    s6-svscan "$S6_SCANDIR" &
    echo $! > "$S6_PIDFILE"
fi
"#;

    let script_path = profile_d.join("50-s6-user.sh");
    std::fs::write(&script_path, script)?;
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;

    Ok(())
}
