use super::{InitSystem, ServiceSpec, ServiceType};
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct OpenRC {
    service_dir: &'static str,
    runlevel_dir: &'static str,
    user_service_dir: &'static str,
}

impl OpenRC {
    /// Create OpenRC configuration for Alpine Linux
    pub fn alpine() -> Self {
        Self {
            service_dir: "etc/init.d",
            runlevel_dir: "etc/runlevels/default",
            user_service_dir: ".config/openrc/sv",
        }
    }

    /// Create OpenRC configuration for Artix OpenRC
    pub fn artix() -> Self {
        Self {
            service_dir: "etc/init.d",
            runlevel_dir: "etc/runlevels/default",
            user_service_dir: ".config/openrc/sv",
        }
    }

    /// Create OpenRC configuration for Gentoo
    pub fn gentoo() -> Self {
        Self {
            service_dir: "etc/init.d",
            runlevel_dir: "etc/runlevels/default",
            user_service_dir: ".config/openrc/sv",
        }
    }

    /// Generate an OpenRC service script
    fn generate_service_script(&self, spec: &ServiceSpec) -> String {
        let depends = if let Some(wait_path) = &spec.wait_for {
            format!(
                "depend() {{\n    need localmount\n    after {}\n}}\n\n",
                wait_path
            )
        } else {
            String::new()
        };

        let env_vars = if !spec.environment.is_empty() {
            spec.environment
                .iter()
                .map(|(k, v)| format!("export {}=\"{}\"", k, v))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n\n"
        } else {
            String::new()
        };

        match spec.service_type {
            ServiceType::Longrun => {
                format!(
                    "#!/sbin/openrc-run\n\
                     # mkOS OpenRC service for {}\n\n\
                     {}\
                     command=\"{}\"\n\
                     command_background=true\n\
                     pidfile=\"/run/{}.pid\"\n\n\
                     {}start_pre() {{\n    \
                     {}\
                     }}\n",
                    spec.name,
                    env_vars,
                    spec.command,
                    spec.name,
                    depends,
                    if let Some(wait_path) = &spec.wait_for {
                        format!(
                            "# Wait for {}\n    \
                             while [ ! -e \"{}\" ]; do sleep 0.1; done",
                            wait_path, wait_path
                        )
                    } else {
                        "return 0".to_string()
                    }
                )
            }
            ServiceType::Oneshot => {
                format!(
                    "#!/sbin/openrc-run\n\
                     # mkOS OpenRC oneshot service for {}\n\n\
                     {}\
                     {}\
                     start() {{\n    \
                     {}\n    \
                     {}\n\
                     }}\n",
                    spec.name,
                    env_vars,
                    depends,
                    if let Some(wait_path) = &spec.wait_for {
                        format!(
                            "# Wait for {}\n    \
                             while [ ! -e \"{}\" ]; do sleep 0.1; done",
                            wait_path, wait_path
                        )
                    } else {
                        String::new()
                    },
                    spec.command
                )
            }
        }
    }
}

impl InitSystem for OpenRC {
    fn name(&self) -> &str {
        "OpenRC"
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_path = root.join(self.service_dir).join(service);
        let runlevel_dir = root.join(self.runlevel_dir);

        if !service_path.exists() {
            anyhow::bail!(
                "Service {} not found in {}",
                service,
                service_path.display()
            );
        }

        fs::create_dir_all(&runlevel_dir)?;

        let link = runlevel_dir.join(service);
        if !link.exists() {
            let target = Path::new("/").join(self.service_dir).join(service);
            std::os::unix::fs::symlink(&target, &link).with_context(|| {
                format!("Failed to enable service {} in default runlevel", service)
            })?;
        }

        Ok(())
    }

    fn disable_service(&self, root: &Path, service: &str) -> Result<()> {
        let link = root.join(self.runlevel_dir).join(service);

        if link.exists() {
            fs::remove_file(&link)
                .with_context(|| format!("Failed to disable service {}", service))?;
        }

        Ok(())
    }

    fn is_service_enabled(&self, root: &Path, service: &str) -> bool {
        root.join(self.runlevel_dir).join(service).exists()
    }

    fn create_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let service_dir = root.join(self.service_dir);
        fs::create_dir_all(&service_dir)?;

        let script_content = self.generate_service_script(spec);
        let script_path = service_dir.join(&spec.name);

        fs::write(&script_path, script_content)
            .with_context(|| format!("Failed to create service {}", spec.name))?;

        // Make executable
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;

        Ok(())
    }

    fn user_service_dir(&self) -> &str {
        // OpenRC doesn't support user services
        // Use a separate init system (runit, s6) for user services
        ""
    }

    fn setup_user_services(&self, _root: &Path) -> Result<()> {
        // OpenRC doesn't support user services - this is a no-op
        // User services should be handled by a separate init system (runit, s6)
        Ok(())
    }

    fn create_user_service(&self, _root: &Path, _spec: &ServiceSpec) -> Result<()> {
        // OpenRC doesn't support user services
        // Use a separate init system (runit, s6) for user services
        anyhow::bail!(
            "OpenRC does not support user services. Use runit or s6 for user service management."
        )
    }
}
