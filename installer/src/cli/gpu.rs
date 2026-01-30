use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum GpuVendor {
    Nvidia,
    AmdDiscrete,
    Intel,
}

pub fn detect_gpus() -> Vec<GpuVendor> {
    let output = match Command::new("lspci").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(_) => {
            eprintln!("Warning: lspci returned an error, GPU detection skipped");
            return Vec::new();
        }
        Err(_) => {
            eprintln!("Warning: lspci not found, GPU detection skipped");
            return Vec::new();
        }
    };

    let mut gpus = Vec::new();

    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("vga") || line_lower.contains("3d controller") {
            if line_lower.contains("nvidia") {
                gpus.push(GpuVendor::Nvidia);
            } else if line_lower.contains("amd") || line_lower.contains("ati") {
                if line_lower.contains("radeon")
                    || line_lower.contains("navi")
                    || line_lower.contains("vega")
                {
                    gpus.push(GpuVendor::AmdDiscrete);
                }
            } else if line_lower.contains("intel") {
                gpus.push(GpuVendor::Intel);
            }
        }
    }

    gpus
}

pub fn get_nvidia_packages() -> Vec<String> {
    vec![
        "nvidia".into(),
        "nvidia-utils".into(),
        "nvidia-prime".into(),
        "lib32-nvidia-utils".into(),
    ]
}
