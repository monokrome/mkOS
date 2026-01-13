use super::{InitSystem, ServiceSpec, ServiceType};
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct Runit {
    service_dir: &'static str,
    enablement_dir: &'static str,
    user_service_dir: &'static str,
}

impl Runit {
    /// Create Runit configuration for Void Linux
    pub fn void() -> Self {
        Self {
            service_dir: "etc/sv",
            enablement_dir: "var/service",
            user_service_dir: "service",
        }
    }

    /// Create Runit configuration for Artix runit
    pub fn artix() -> Self {
        Self {
            service_dir: "etc/runit/sv",
            enablement_dir: "etc/runit/runsvdir/default",
            user_service_dir: "service",
        }
    }

    /// Generate a runit run script
    fn generate_run_script(&self, spec: &ServiceSpec) -> String {
        let mut script = String::from("#!/bin/sh\n");

        if let Some(wait_path) = &spec.wait_for {
            script.push_str(&format!(
                "# Wait for {}\nwhile [ ! -e \"{}\" ]; do sleep 0.1; done\n\n",
                wait_path, wait_path
            ));
        }

        for (key, value) in &spec.environment {
            script.push_str(&format!("export {}=\"{}\"\n", key, value));
        }

        if !spec.environment.is_empty() {
            script.push('\n');
        }

        script.push_str(&format!("exec {}\n", spec.command));
        script
    }
}

impl InitSystem for Runit {
    fn name(&self) -> &str {
        "runit"
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_path = root.join(self.service_dir).join(service);
        let enablement_dir = root.join(self.enablement_dir);

        if !service_path.exists() {
            anyhow::bail!(
                "Service {} not found in {}",
                service,
                service_path.display()
            );
        }

        fs::create_dir_all(&enablement_dir)?;

        let link = enablement_dir.join(service);
        if !link.exists() {
            let target = Path::new("/").join(self.service_dir).join(service);
            std::os::unix::fs::symlink(&target, &link)
                .with_context(|| format!("Failed to enable service {}", service))?;
        }

        Ok(())
    }

    fn disable_service(&self, root: &Path, service: &str) -> Result<()> {
        let link = root.join(self.enablement_dir).join(service);

        if link.exists() {
            fs::remove_file(&link)
                .with_context(|| format!("Failed to disable service {}", service))?;
        }

        Ok(())
    }

    fn is_service_enabled(&self, root: &Path, service: &str) -> bool {
        root.join(self.enablement_dir).join(service).exists()
    }

    fn create_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let service_path = root.join(self.service_dir).join(&spec.name);
        fs::create_dir_all(&service_path)?;

        // Create run script
        let run_script = self.generate_run_script(spec);
        let run_path = service_path.join("run");
        fs::write(&run_path, run_script)
            .with_context(|| format!("Failed to create run script for {}", spec.name))?;

        let mut perms = fs::metadata(&run_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&run_path, perms)?;

        // For oneshot services, create a finish script that prevents restart
        if spec.service_type == ServiceType::Oneshot {
            let finish_script = "#!/bin/sh\n# Oneshot service - exit after completion\nexit 0\n";
            let finish_path = service_path.join("finish");
            fs::write(&finish_path, finish_script)?;

            let mut perms = fs::metadata(&finish_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&finish_path, perms)?;

            // Create a down file to prevent automatic restart
            fs::write(service_path.join("down"), "")?;
        }

        Ok(())
    }

    fn user_service_dir(&self) -> &str {
        self.user_service_dir
    }

    fn setup_user_services(&self, root: &Path) -> Result<()> {
        let skel_service = root.join("etc/skel").join(self.user_service_dir);
        fs::create_dir_all(&skel_service)
            .context("Failed to create user service skeleton directory")?;

        // Create a README explaining user service setup
        let readme = "# User Services (runit)\n\n\
            This directory contains your personal runit services.\n\n\
            ## Running User Services\n\n\
            Add this to your shell profile (~/.profile or ~/.bash_profile):\n\n\
            ```sh\n\
            export SVDIR=~/service\n\
            if [ -z \"$RUNSVDIR_PID\" ]; then\n    \
            runsvdir ~/service &\n\
            fi\n\
            ```\n\n\
            Then create services in ~/service/ just like system services.\n";

        fs::write(skel_service.join("README.md"), readme)?;

        Ok(())
    }

    fn create_user_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let skel_service = root.join("etc/skel").join(self.user_service_dir);
        let service_dir = skel_service.join(&spec.name);
        fs::create_dir_all(&service_dir)?;

        // Create run script
        let run_script = self.generate_run_script(spec);
        let run_path = service_dir.join("run");
        fs::write(&run_path, run_script)?;

        let mut perms = fs::metadata(&run_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&run_path, perms)?;

        // For oneshot services
        if spec.service_type == ServiceType::Oneshot {
            let finish_script = "#!/bin/sh\nexit 0\n";
            let finish_path = service_dir.join("finish");
            fs::write(&finish_path, finish_script)?;

            let mut perms = fs::metadata(&finish_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&finish_path, perms)?;

            fs::write(service_dir.join("down"), "")?;
        }

        Ok(())
    }
}
