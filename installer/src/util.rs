use std::fs;

/// CPU vendor enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

impl CpuVendor {
    /// Get the microcode package name for this CPU vendor
    pub fn microcode_package(&self) -> Option<&'static str> {
        match self {
            CpuVendor::Intel => Some("intel-ucode"),
            CpuVendor::Amd => Some("amd-ucode"),
            CpuVendor::Unknown => None,
        }
    }

    /// Get a human-readable name for this vendor
    pub fn name(&self) -> &'static str {
        match self {
            CpuVendor::Intel => "Intel",
            CpuVendor::Amd => "AMD",
            CpuVendor::Unknown => "Unknown",
        }
    }
}

/// Detect the CPU vendor from /proc/cpuinfo
pub fn detect_cpu_vendor() -> CpuVendor {
    let cpuinfo = match fs::read_to_string("/proc/cpuinfo") {
        Ok(content) => content,
        Err(_) => return CpuVendor::Unknown,
    };

    if cpuinfo.contains("GenuineIntel") {
        CpuVendor::Intel
    } else if cpuinfo.contains("AuthenticAMD") {
        CpuVendor::Amd
    } else {
        CpuVendor::Unknown
    }
}
