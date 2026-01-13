use super::{InitSystem, ServiceSpec, ServiceType};
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct SysVinit {
    service_dir: &'static str,
    runlevel_dirs: &'static [&'static str],
    user_service_dir: &'static str,
}

impl SysVinit {
    /// Create SysVinit configuration for Devuan
    pub fn devuan() -> Self {
        Self {
            service_dir: "etc/init.d",
            runlevel_dirs: &["etc/rc2.d", "etc/rc3.d", "etc/rc4.d", "etc/rc5.d"],
            user_service_dir: ".config/init/sv",
        }
    }

    /// Create SysVinit configuration for legacy Debian
    pub fn debian() -> Self {
        Self {
            service_dir: "etc/init.d",
            runlevel_dirs: &["etc/rc2.d", "etc/rc3.d", "etc/rc4.d", "etc/rc5.d"],
            user_service_dir: ".config/init/sv",
        }
    }

    /// Generate a SysVinit service script
    fn generate_service_script(&self, spec: &ServiceSpec) -> String {
        let description = format!("mkOS service for {}", spec.name);

        let env_vars = if !spec.environment.is_empty() {
            spec.environment
                .iter()
                .map(|(k, v)| format!("export {}=\"{}\"", k, v))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        } else {
            String::new()
        };

        let wait_code = if let Some(wait_path) = &spec.wait_for {
            format!(
                "    # Wait for {}\n    \
                 while [ ! -e \"{}\" ]; do sleep 0.1; done\n",
                wait_path, wait_path
            )
        } else {
            String::new()
        };

        match spec.service_type {
            ServiceType::Longrun => {
                format!(
                    "#!/bin/sh\n\
                     ### BEGIN INIT INFO\n\
                     # Provides:          {name}\n\
                     # Required-Start:    $local_fs $remote_fs $network\n\
                     # Required-Stop:     $local_fs $remote_fs $network\n\
                     # Default-Start:     2 3 4 5\n\
                     # Default-Stop:      0 1 6\n\
                     # Short-Description: {desc}\n\
                     ### END INIT INFO\n\n\
                     {env}\
                     PIDFILE=/var/run/{name}.pid\n\n\
                     case \"$1\" in\n  \
                     start)\n    \
                     echo \"Starting {name}...\"\n\
                     {wait}\
                     start-stop-daemon --start --background --make-pidfile \\\n      \
                     --pidfile $PIDFILE --exec {cmd}\n    \
                     ;;\n  \
                     stop)\n    \
                     echo \"Stopping {name}...\"\n    \
                     start-stop-daemon --stop --pidfile $PIDFILE\n    \
                     rm -f $PIDFILE\n    \
                     ;;\n  \
                     restart)\n    \
                     $0 stop\n    \
                     sleep 1\n    \
                     $0 start\n    \
                     ;;\n  \
                     status)\n    \
                     if [ -f $PIDFILE ]; then\n      \
                     echo \"{name} is running (PID $(cat $PIDFILE))\"\n      \
                     exit 0\n    \
                     else\n      \
                     echo \"{name} is not running\"\n      \
                     exit 1\n    \
                     fi\n    \
                     ;;\n  \
                     *)\n    \
                     echo \"Usage: $0 {{start|stop|restart|status}}\"\n    \
                     exit 1\n    \
                     ;;\n\
                     esac\n\n\
                     exit 0\n",
                    name = spec.name,
                    desc = description,
                    env = env_vars,
                    wait = wait_code,
                    cmd = spec.command
                )
            }
            ServiceType::Oneshot => {
                format!(
                    "#!/bin/sh\n\
                     ### BEGIN INIT INFO\n\
                     # Provides:          {name}\n\
                     # Required-Start:    $local_fs $remote_fs\n\
                     # Required-Stop:\n\
                     # Default-Start:     2 3 4 5\n\
                     # Default-Stop:\n\
                     # Short-Description: {desc}\n\
                     ### END INIT INFO\n\n\
                     {env}\
                     case \"$1\" in\n  \
                     start)\n    \
                     echo \"Running {name}...\"\n\
                     {wait}\
                     {cmd}\n    \
                     ;;\n  \
                     stop|restart|status)\n    \
                     # Oneshot service, nothing to do\n    \
                     exit 0\n    \
                     ;;\n  \
                     *)\n    \
                     echo \"Usage: $0 {{start|stop|restart|status}}\"\n    \
                     exit 1\n    \
                     ;;\n\
                     esac\n\n\
                     exit 0\n",
                    name = spec.name,
                    desc = description,
                    env = env_vars,
                    wait = wait_code,
                    cmd = spec.command
                )
            }
        }
    }
}

impl InitSystem for SysVinit {
    fn name(&self) -> &str {
        "SysVinit"
    }

    fn enable_service(&self, root: &Path, service: &str) -> Result<()> {
        let service_path = root.join(self.service_dir).join(service);

        if !service_path.exists() {
            anyhow::bail!(
                "Service {} not found in {}",
                service,
                service_path.display()
            );
        }

        // Create symlinks in all default runlevel directories
        // S20 = start priority 20 (after basic services)
        let target = Path::new("/").join(self.service_dir).join(service);

        for runlevel_dir in self.runlevel_dirs {
            let dir = root.join(runlevel_dir);
            fs::create_dir_all(&dir)?;

            let link = dir.join(format!("S20{}", service));
            if !link.exists() {
                std::os::unix::fs::symlink(&target, &link).with_context(|| {
                    format!("Failed to enable service {} in {}", service, runlevel_dir)
                })?;
            }
        }

        // Also create stop links in runlevels 0, 1, 6
        let stop_runlevels = ["etc/rc0.d", "etc/rc1.d", "etc/rc6.d"];
        for runlevel_dir in &stop_runlevels {
            let dir = root.join(runlevel_dir);
            fs::create_dir_all(&dir)?;

            let link = dir.join(format!("K80{}", service));
            if !link.exists() {
                std::os::unix::fs::symlink(&target, &link)?;
            }
        }

        Ok(())
    }

    fn disable_service(&self, root: &Path, service: &str) -> Result<()> {
        // Remove all symlinks from all runlevel directories
        let all_runlevels = [
            "etc/rc0.d",
            "etc/rc1.d",
            "etc/rc2.d",
            "etc/rc3.d",
            "etc/rc4.d",
            "etc/rc5.d",
            "etc/rc6.d",
        ];

        for runlevel_dir in &all_runlevels {
            let dir = root.join(runlevel_dir);
            if !dir.exists() {
                continue;
            }

            // Remove both S* and K* links
            for prefix in &["S", "K"] {
                let pattern = format!("{}{}", prefix, service);
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.contains(&pattern) {
                                fs::remove_file(entry.path())?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn is_service_enabled(&self, root: &Path, service: &str) -> bool {
        // Check if service is enabled in any default runlevel
        for runlevel_dir in self.runlevel_dirs {
            let dir = root.join(runlevel_dir);
            if !dir.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with('S') && name.contains(service) {
                            return true;
                        }
                    }
                }
            }
        }

        false
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

        // SysVinit doesn't have user service support, provide guidance
        let readme = "# User Services\n\n\
            SysVinit does not have built-in user service support.\n\
            This directory is provided for compatibility with mkOS manifests.\n\n\
            To run user services, consider using:\n\
            - runit's runsvdir in your session startup\n\
            - s6 as a user\n\
            - Add commands to your shell profile (~/.profile)\n\
            - Use systemd --user (if migrating to a systemd distro later)\n";

        fs::write(skel_sv.join("README.md"), readme)?;

        Ok(())
    }

    fn create_user_service(&self, root: &Path, spec: &ServiceSpec) -> Result<()> {
        let skel_sv = root.join("etc/skel").join(self.user_service_dir);
        let service_dir = skel_sv.join(&spec.name);
        fs::create_dir_all(&service_dir)?;

        // Create a simple shell script wrapper that can be sourced
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

        script.push_str(&format!("{} &\n", spec.command));
        script.push_str(&format!("echo $! > ~/.{}.pid\n", spec.name));

        let script_path = service_dir.join("run.sh");
        fs::write(&script_path, script)?;

        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;

        if spec.service_type == ServiceType::Oneshot {
            fs::write(service_dir.join("type"), "oneshot\n")?;
        }

        Ok(())
    }
}
