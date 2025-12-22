use super::{InitSystem, ServiceSpec, ServiceType};
use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// S6 init system implementation
pub struct S6 {
    /// Directory where service definitions live (e.g., "etc/s6/sv")
    service_dir: &'static str,
    /// Directory where enabled services are symlinked (e.g., "etc/s6/adminsv/default")
    enablement_dir: &'static str,
    /// User service directory relative to home
    user_service_dir: &'static str,
}

impl S6 {
    /// S6 configuration for Artix Linux
    pub fn artix() -> Self {
        Self {
            service_dir: "etc/s6/sv",
            enablement_dir: "etc/s6/adminsv/default",
            user_service_dir: ".config/s6/sv",
        }
    }

    /// S6 configuration for Void Linux
    pub fn void() -> Self {
        Self {
            service_dir: "etc/s6/sv",
            enablement_dir: "etc/s6/rc/default",
            user_service_dir: ".config/s6/sv",
        }
    }

    /// Generate s6 run script content for a service spec
    fn generate_run_script(&self, spec: &ServiceSpec) -> String {
        let mut script = String::from("#!/bin/sh\n");

        // Add environment variables
        for (key, value) in &spec.environment {
            script.push_str(&format!("export {}=\"{}\"\n", key, value));
        }

        // Add wait loop if needed
        if let Some(wait_path) = &spec.wait_for {
            script.push_str(&format!("# Wait for {}\n", wait_path));
            script.push_str(&format!("while [ ! -e \"{}\" ]; do\n", wait_path));
            script.push_str("    sleep 0.1\n");
            script.push_str("done\n");
        }

        // Add the command
        match spec.service_type {
            ServiceType::Longrun => {
                script.push_str(&format!("exec {}\n", spec.command));
            }
            ServiceType::Oneshot => {
                script.push_str(&format!("{}\n", spec.command));
            }
        }

        script
    }

    /// Write a service to a directory
    fn write_service(&self, service_dir: &Path, spec: &ServiceSpec) -> Result<()> {
        std::fs::create_dir_all(service_dir)?;

        // Write run script
        let run_script = self.generate_run_script(spec);
        let run_path = service_dir.join("run");
        std::fs::write(&run_path, run_script)?;
        std::fs::set_permissions(&run_path, std::fs::Permissions::from_mode(0o755))?;

        // For oneshot services, write type and up files
        if spec.service_type == ServiceType::Oneshot {
            std::fs::write(service_dir.join("type"), "oneshot\n")?;
            std::fs::write(service_dir.join("up"), "run\n")?;
        }

        Ok(())
    }
}

impl InitSystem for S6 {
    fn name(&self) -> &str {
        "s6"
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_src = root.join(self.service_dir).join(service);
        let service_dst = root.join(self.enablement_dir).join(service);

        if !service_src.exists() {
            anyhow::bail!(
                "Service '{}' not found at {}. The service package may not be installed.",
                service,
                service_src.display()
            );
        }

        if let Some(parent) = service_dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if service_dst.exists() {
            return Ok(());
        }

        std::os::unix::fs::symlink(&service_src, &service_dst)
            .context(format!("Failed to enable service '{}'", service))
    }

    fn disable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_dst = root.join(self.enablement_dir).join(service);

        if service_dst.exists() || service_dst.is_symlink() {
            std::fs::remove_file(&service_dst)
                .context(format!("Failed to disable service '{}'", service))?;
        }

        Ok(())
    }

    fn is_service_enabled(&self, root: &Path, service: &str) -> bool {
        let service_dst = root.join(self.enablement_dir).join(service);
        service_dst.exists() || service_dst.is_symlink()
    }

    fn create_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let service_dir = root.join(self.service_dir).join(&spec.name);
        self.write_service(&service_dir, spec)
            .context(format!("Failed to create system service '{}'", spec.name))
    }

    fn user_service_dir(&self) -> &str {
        self.user_service_dir
    }

    fn setup_user_services(&self, root: &Path) -> Result<()> {
        let skel_s6 = root.join("etc/skel/.config/s6");

        // Create service directory
        std::fs::create_dir_all(skel_s6.join("sv"))?;

        // Create log directory for s6-svscan
        std::fs::create_dir_all(skel_s6.join("log"))?;

        // Create profile.d script to start user s6-svscan on login
        let profile_d = root.join("etc/profile.d");
        std::fs::create_dir_all(&profile_d)?;

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

    fn create_user_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let skel_sv = root.join("etc/skel").join(self.user_service_dir);
        let service_dir = skel_sv.join(&spec.name);
        self.write_service(&service_dir, spec)
            .context(format!("Failed to create user service '{}'", spec.name))
    }
}
