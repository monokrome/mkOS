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
        self.user_service_dir
    }

    fn setup_user_services(&self, root: &Path) -> Result<()> {
        let skel_sv = root.join("etc/skel").join(self.user_service_dir);
        fs::create_dir_all(&skel_sv)
            .context("Failed to create user service skeleton directory")?;

        // OpenRC doesn't have native user service support, so we create a simple wrapper
        // Users can manage their own services using a personal runsvdir or similar
        let readme = "# User Services\n\n\
            OpenRC does not have built-in user service support.\n\
            This directory is provided for compatibility with mkOS manifests.\n\n\
            To run user services, consider using:\n\
            - runit's runsvdir in your session startup\n\
            - s6-rc as a user\n\
            - supervise-daemon (OpenRC's supervisor)\n";

        fs::write(skel_sv.join("README.md"), readme)?;

        Ok(())
    }

    fn create_user_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let skel_sv = root.join("etc/skel").join(self.user_service_dir);
        let service_dir = skel_sv.join(&spec.name);
        fs::create_dir_all(&service_dir)?;

        // Create a simple run script that can be used with runsvdir
        let mut run_script = String::from("#!/bin/sh\n");

        if let Some(wait_path) = &spec.wait_for {
            run_script.push_str(&format!(
                "# Wait for {}\nwhile [ ! -e \"{}\" ]; do sleep 0.1; done\n\n",
                wait_path, wait_path
            ));
        }

        for (key, value) in &spec.environment {
            run_script.push_str(&format!("export {}=\"{}\"\n", key, value));
        }

        if !spec.environment.is_empty() {
            run_script.push('\n');
        }

        run_script.push_str(&format!("exec {}\n", spec.command));

        let run_path = service_dir.join("run");
        fs::write(&run_path, run_script)?;

        let mut perms = fs::metadata(&run_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&run_path, perms)?;

        if spec.service_type == ServiceType::Oneshot {
            fs::write(service_dir.join("type"), "oneshot\n")?;
        }

        Ok(())
    }
}
