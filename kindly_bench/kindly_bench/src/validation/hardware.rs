//! Hardware validation for B32 compliance
//!
//! Collects 27 hardware/software checks for reproducibility

use std::fs;

/// Hardware information for B32 validation
#[derive(Debug, Clone)]
pub struct HardwareInfo {
    /// CPU model name
    pub cpu_model: String,
    /// CPU microarchitecture (best effort)
    pub microarchitecture: String,
    /// Total CPU cores
    pub cores_total: usize,
    /// Performance cores (P-cores)
    pub cores_p: usize,
    /// Efficiency cores (E-cores)
    pub cores_e: Option<usize>,
    /// Low-power cores (LP-cores)
    pub cores_lp: Option<usize>,
    /// Base frequency (MHz)
    pub frequency_base: Option<u32>,
    /// Boost frequency (MHz)
    pub frequency_boost: Option<u32>,
    /// Turbo boost enabled
    pub turbo_enabled: Option<bool>,
    /// Hyperthreading enabled
    pub hyperthreading_enabled: bool,
    /// CPU frequency scaling governor
    pub frequency_scaling_governor: Option<String>,
    /// Total memory size (GB)
    pub memory_size_gb: Option<u32>,
    /// Memory type (DDR4, DDR5, etc.)
    pub memory_type: Option<String>,
    /// Memory speed (MHz)
    pub memory_speed_mhz: Option<u32>,
    /// Memory channels
    pub memory_channels: Option<u32>,
    /// Memory bandwidth (GB/s)
    pub memory_bandwidth_gbps: Option<f64>,
    /// L1 data cache size (KB)
    pub l1_data_kb: Option<u32>,
    /// L1 instruction cache size (KB)
    pub l1_instruction_kb: Option<u32>,
    /// L2 cache size (KB)
    pub l2_kb: Option<u32>,
    /// L3 cache size (KB)
    pub l3_kb: Option<u32>,
    /// Cache line size (bytes)
    pub cache_line_bytes: u32,
    /// NUMA nodes
    pub numa_nodes: Option<usize>,
    /// NUMA enabled
    pub numa_enabled: bool,
    /// OS name
    pub os: String,
    /// Kernel version
    pub kernel: String,
    /// Rust version
    pub rust_version: String,
}

impl HardwareInfo {
    /// Collect hardware information for current system
    pub fn collect() -> Self {
        Self {
            cpu_model: Self::get_cpu_model(),
            microarchitecture: Self::get_microarchitecture(),
            cores_total: Self::get_cores_total(),
            cores_p: Self::get_cores_p(),
            cores_e: Self::get_cores_e(),
            cores_lp: Self::get_cores_lp(),
            frequency_base: Self::get_frequency_base(),
            frequency_boost: Self::get_frequency_boost(),
            turbo_enabled: Self::get_turbo_enabled(),
            hyperthreading_enabled: Self::get_hyperthreading_enabled(),
            frequency_scaling_governor: Self::get_frequency_scaling_governor(),
            memory_size_gb: Self::get_memory_size_gb(),
            memory_type: Self::get_memory_type(),
            memory_speed_mhz: Self::get_memory_speed_mhz(),
            memory_channels: Self::get_memory_channels(),
            memory_bandwidth_gbps: Self::get_memory_bandwidth_gbps(),
            l1_data_kb: Self::get_l1_data_kb(),
            l1_instruction_kb: Self::get_l1_instruction_kb(),
            l2_kb: Self::get_l2_kb(),
            l3_kb: Self::get_l3_kb(),
            cache_line_bytes: Self::get_cache_line_bytes(),
            numa_nodes: Self::get_numa_nodes(),
            numa_enabled: Self::get_numa_enabled(),
            os: Self::get_os(),
            kernel: Self::get_kernel(),
            rust_version: Self::get_rust_version(),
        }
    }

    /// Get CPU model name
    fn get_cpu_model() -> String {
        #[cfg(target_os = "linux")]
        {
            if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
                for line in cpuinfo.lines() {
                    if line.starts_with("model name") {
                        if let Some(model) = line.split(':').nth(1) {
                            return model.trim().to_string();
                        }
                    }
                }
            }
        }

        "Unknown".to_string()
    }

    /// Get CPU microarchitecture (best effort)
    fn get_microarchitecture() -> String {
        let model = Self::get_cpu_model();

        // Heuristic detection based on model name
        if model.contains("Ultra") {
            "Meteor Lake".to_string()
        } else if model.contains("13th Gen") || model.contains("14th Gen") {
            "Raptor Lake".to_string()
        } else if model.contains("12th Gen") {
            "Alder Lake".to_string()
        } else if model.contains("Ryzen 9 6900") {
            "Zen 3+".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    /// Get total CPU cores
    fn get_cores_total() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    /// Get performance cores (P-cores)
    fn get_cores_p() -> usize {
        // For now, assume all cores are P-cores
        // TODO: Parse /proc/cpuinfo or lscpu for hybrid architectures
        Self::get_cores_total()
    }

    /// Get efficiency cores (E-cores)
    fn get_cores_e() -> Option<usize> {
        // TODO: Detect E-cores for hybrid architectures
        None
    }

    /// Get low-power cores (LP-cores)
    fn get_cores_lp() -> Option<usize> {
        // TODO: Detect LP-cores for Meteor Lake
        None
    }

    /// Get base frequency (MHz)
    fn get_frequency_base() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(freq) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/base_frequency") {
                if let Ok(khz) = freq.trim().parse::<u32>() {
                    return Some(khz / 1000); // Convert kHz to MHz
                }
            }
        }

        None
    }

    /// Get boost frequency (MHz)
    fn get_frequency_boost() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(freq) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq") {
                if let Ok(khz) = freq.trim().parse::<u32>() {
                    return Some(khz / 1000); // Convert kHz to MHz
                }
            }
        }

        None
    }

    /// Get turbo boost status
    fn get_turbo_enabled() -> Option<bool> {
        #[cfg(target_os = "linux")]
        {
            // Intel: check /sys/devices/system/cpu/intel_pstate/no_turbo
            if let Ok(no_turbo) = fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo") {
                return Some(no_turbo.trim() == "0");
            }

            // AMD: check /sys/devices/system/cpu/cpufreq/boost
            if let Ok(boost) = fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost") {
                return Some(boost.trim() == "1");
            }
        }

        None
    }

    /// Get hyperthreading status
    fn get_hyperthreading_enabled() -> bool {
        #[cfg(target_os = "linux")]
        {
            if let Ok(siblings) = fs::read_to_string("/sys/devices/system/cpu/cpu0/topology/thread_siblings_list") {
                // If siblings list has more than 1 entry, HT is enabled
                return siblings.split(',').count() > 1;
            }
        }

        false
    }

    /// Get CPU frequency scaling governor
    fn get_frequency_scaling_governor() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(governor) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor") {
                return Some(governor.trim().to_string());
            }
        }

        None
    }

    /// Get total memory size (GB)
    fn get_memory_size_gb() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                return Some((kb / 1024 / 1024) as u32); // Convert KB to GB
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Get memory type (DDR4, DDR5, etc.)
    fn get_memory_type() -> Option<String> {
        // TODO: Parse dmidecode output (requires root)
        None
    }

    /// Get memory speed (MHz)
    fn get_memory_speed_mhz() -> Option<u32> {
        // TODO: Parse dmidecode output (requires root)
        None
    }

    /// Get memory channels
    fn get_memory_channels() -> Option<u32> {
        // TODO: Parse dmidecode output (requires root)
        None
    }

    /// Get memory bandwidth (GB/s)
    fn get_memory_bandwidth_gbps() -> Option<f64> {
        // TODO: Calculate from memory speed and channels
        None
    }

    /// Get L1 data cache size (KB)
    fn get_l1_data_kb() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(size) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index0/size") {
                return Self::parse_cache_size(&size);
            }
        }

        None
    }

    /// Get L1 instruction cache size (KB)
    fn get_l1_instruction_kb() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(size) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index1/size") {
                return Self::parse_cache_size(&size);
            }
        }

        None
    }

    /// Get L2 cache size (KB)
    fn get_l2_kb() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(size) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index2/size") {
                return Self::parse_cache_size(&size);
            }
        }

        None
    }

    /// Get L3 cache size (KB)
    fn get_l3_kb() -> Option<u32> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(size) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index3/size") {
                return Self::parse_cache_size(&size);
            }
        }

        None
    }

    /// Parse cache size string (e.g., "32K", "256K")
    fn parse_cache_size(size_str: &str) -> Option<u32> {
        let trimmed = size_str.trim();
        if trimmed.ends_with('K') {
            trimmed.trim_end_matches('K').parse::<u32>().ok()
        } else if trimmed.ends_with('M') {
            trimmed.trim_end_matches('M').parse::<u32>().ok().map(|m| m * 1024)
        } else {
            trimmed.parse::<u32>().ok()
        }
    }

    /// Get cache line size (bytes)
    fn get_cache_line_bytes() -> u32 {
        #[cfg(target_os = "linux")]
        {
            if let Ok(size) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index0/coherency_line_size") {
                if let Ok(bytes) = size.trim().parse::<u32>() {
                    return bytes;
                }
            }
        }

        64 // Default assumption
    }

    /// Get number of NUMA nodes
    fn get_numa_nodes() -> Option<usize> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = fs::read_dir("/sys/devices/system/node") {
                let count = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("node"))
                    .count();
                if count > 0 {
                    return Some(count);
                }
            }
        }

        None
    }

    /// Get NUMA enabled status
    fn get_numa_enabled() -> bool {
        Self::get_numa_nodes().map(|n| n > 1).unwrap_or(false)
    }

    /// Get OS name
    fn get_os() -> String {
        std::env::consts::OS.to_string()
    }

    /// Get kernel version
    fn get_kernel() -> String {
        #[cfg(target_os = "linux")]
        {
            if let Ok(version) = fs::read_to_string("/proc/version") {
                return version.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            }
        }

        "Unknown".to_string()
    }

    /// Get Rust version
    fn get_rust_version() -> String {
        env!("CARGO_PKG_RUST_VERSION").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_info_collection() {
        let info = HardwareInfo::collect();

        // Basic sanity checks
        assert!(!info.cpu_model.is_empty());
        assert!(info.cores_total > 0);
        assert_eq!(info.cache_line_bytes, 64); // Most systems
        assert!(!info.os.is_empty());
    }
}
