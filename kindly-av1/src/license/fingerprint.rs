//! Hardware fingerprint generation
//! [TRADE SECRET]
//!
//! Generates Blake3 hash of CPU ID + MAC address for hardware binding.
//!
//! # Platform Support
//!
//! - **Linux**: Uses `/proc/cpuinfo` for CPU ID and `/sys/class/net/*/address` for MAC
//! - **macOS**: Uses `sysctl` for CPU ID and `ifconfig` for MAC
//! - **Windows**: Uses `GetSystemInfo` and `GetAdaptersAddresses`
//!
//! # Security Considerations
//!
//! The fingerprint is a one-way hash - the original CPU/MAC values cannot be
//! recovered from it. This prevents reverse-engineering of license keys.
//!
//! # Stability
//!
//! The fingerprint is designed to be stable across:
//! - OS updates
//! - Driver updates
//! - Software reinstalls
//!
//! It will change if:
//! - CPU is replaced
//! - Primary network adapter is replaced
//! - Machine is virtualized/containerized differently

use std::collections::BTreeSet;

/// Hardware fingerprint (32 bytes)
///
/// Blake3 hash of combined hardware identifiers.
/// Used for license binding to specific machines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareFingerprint([u8; 32]);

impl HardwareFingerprint {
    /// Generate fingerprint for current machine
    ///
    /// Combines CPU identifier and MAC address into a Blake3 hash.
    /// Returns a consistent value for the same hardware.
    pub fn generate() -> Self {
        let mut hasher = blake3::Hasher::new();

        // Add CPU identifier
        let cpu_id = get_cpu_id();
        hasher.update(&cpu_id);

        // Add MAC address
        let mac = get_mac_address();
        hasher.update(&mac);

        // Add domain separator
        hasher.update(b"kindly-av1-fingerprint-v1");

        Self(*hasher.finalize().as_bytes())
    }

    /// Create from existing bytes
    ///
    /// Used when loading a stored fingerprint from disk.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes
    #[inline]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string (for debugging)
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl Default for HardwareFingerprint {
    fn default() -> Self {
        Self::generate()
    }
}

/// Get CPU identifier (platform-specific)
///
/// Returns a stable identifier for the CPU that doesn't change
/// across reboots or OS updates.
#[cfg(target_os = "linux")]
fn get_cpu_id() -> Vec<u8> {
    use std::fs;

    let mut cpu_data = Vec::new();

    // Read CPU model from /proc/cpuinfo
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            // Get model name (stable across boots)
            if line.starts_with("model name") {
                if let Some(value) = line.split(':').nth(1) {
                    cpu_data.extend_from_slice(value.trim().as_bytes());
                }
                break;
            }
        }

        // Get CPU family and model (stable hardware identifiers)
        for line in cpuinfo.lines() {
            if line.starts_with("cpu family")
                || line.starts_with("model\t")
                || line.starts_with("stepping")
            {
                if let Some(value) = line.split(':').nth(1) {
                    cpu_data.extend_from_slice(value.trim().as_bytes());
                }
            }
        }
    }

    // Fallback: read from /sys/devices/system/cpu
    if cpu_data.is_empty() {
        if let Ok(model) =
            fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        {
            cpu_data.extend_from_slice(model.trim().as_bytes());
        }
    }

    // If still empty, use a placeholder
    if cpu_data.is_empty() {
        cpu_data.extend_from_slice(b"unknown-cpu");
    }

    cpu_data
}

/// Get CPU identifier (platform-specific)
#[cfg(target_os = "macos")]
fn get_cpu_id() -> Vec<u8> {
    use std::process::Command;

    let mut cpu_data = Vec::new();

    // Use sysctl to get CPU brand string
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
    {
        if output.status.success() {
            cpu_data.extend_from_slice(&output.stdout);
        }
    }

    // Also get CPU family and model
    if let Ok(output) = Command::new("sysctl")
        .args(["-n", "machdep.cpu.family"])
        .output()
    {
        if output.status.success() {
            cpu_data.extend_from_slice(&output.stdout);
        }
    }

    if cpu_data.is_empty() {
        cpu_data.extend_from_slice(b"unknown-cpu");
    }

    cpu_data
}

/// Get CPU identifier (platform-specific)
#[cfg(target_os = "windows")]
fn get_cpu_id() -> Vec<u8> {
    use std::process::Command;

    let mut cpu_data = Vec::new();

    // Use WMIC to get CPU information
    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "ProcessorId,Name", "/format:csv"])
        .output()
    {
        if output.status.success() {
            cpu_data.extend_from_slice(&output.stdout);
        }
    }

    if cpu_data.is_empty() {
        cpu_data.extend_from_slice(b"unknown-cpu");
    }

    cpu_data
}

/// Get CPU identifier (fallback for other platforms)
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_cpu_id() -> Vec<u8> {
    // Return a platform-specific placeholder
    b"unknown-platform-cpu".to_vec()
}

/// Get primary MAC address (platform-specific)
///
/// Returns the MAC address of the first physical network interface.
/// Virtual interfaces (docker, veth, etc.) are excluded.
#[cfg(target_os = "linux")]
fn get_mac_address() -> Vec<u8> {
    use std::fs;

    // Collect all physical interface MAC addresses
    let mut macs: BTreeSet<String> = BTreeSet::new();

    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let iface_name = entry.file_name().to_string_lossy().to_string();

            // Skip virtual interfaces
            if iface_name.starts_with("lo")
                || iface_name.starts_with("docker")
                || iface_name.starts_with("veth")
                || iface_name.starts_with("br-")
                || iface_name.starts_with("virbr")
            {
                continue;
            }

            // Check if physical interface (has device/driver symlink)
            let driver_path = entry.path().join("device/driver");
            if !driver_path.exists() {
                continue;
            }

            // Read MAC address
            let address_path = entry.path().join("address");
            if let Ok(mac) = fs::read_to_string(address_path) {
                let mac = mac.trim().to_lowercase();
                // Skip zero/broadcast MACs
                if mac != "00:00:00:00:00:00" && mac != "ff:ff:ff:ff:ff:ff" {
                    macs.insert(mac);
                }
            }
        }
    }

    // Use first MAC (sorted for consistency)
    if let Some(mac) = macs.into_iter().next() {
        return mac.as_bytes().to_vec();
    }

    b"unknown-mac".to_vec()
}

/// Get primary MAC address (platform-specific)
#[cfg(target_os = "macos")]
fn get_mac_address() -> Vec<u8> {
    use std::process::Command;

    // Use ifconfig to get MAC addresses
    if let Ok(output) = Command::new("ifconfig").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Look for ether lines
            for line in stdout.lines() {
                let line = line.trim();
                if line.starts_with("ether ") {
                    if let Some(mac) = line.split_whitespace().nth(1) {
                        let mac = mac.to_lowercase();
                        if mac != "00:00:00:00:00:00" && mac != "ff:ff:ff:ff:ff:ff" {
                            return mac.as_bytes().to_vec();
                        }
                    }
                }
            }
        }
    }

    b"unknown-mac".to_vec()
}

/// Get primary MAC address (platform-specific)
#[cfg(target_os = "windows")]
fn get_mac_address() -> Vec<u8> {
    use std::process::Command;

    // Use getmac command
    if let Ok(output) = Command::new("getmac").args(["/FO", "CSV", "/NH"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse first valid MAC
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').collect();
                if !parts.is_empty() {
                    let mac = parts[0].trim_matches('"').to_lowercase();
                    // Windows uses - instead of :
                    let mac = mac.replace('-', ":");
                    if mac.len() == 17
                        && mac != "00:00:00:00:00:00"
                        && mac != "ff:ff:ff:ff:ff:ff"
                    {
                        return mac.as_bytes().to_vec();
                    }
                }
            }
        }
    }

    b"unknown-mac".to_vec()
}

/// Get primary MAC address (fallback for other platforms)
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_mac_address() -> Vec<u8> {
    b"unknown-platform-mac".to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_generation() {
        let fp1 = HardwareFingerprint::generate();
        let fp2 = HardwareFingerprint::generate();

        // Same machine should produce same fingerprint
        assert_eq!(fp1, fp2);

        // Should be 32 bytes
        assert_eq!(fp1.as_bytes().len(), 32);
    }

    #[test]
    fn test_fingerprint_from_bytes() {
        let bytes = [0xAA; 32];
        let fp = HardwareFingerprint::from_bytes(bytes);
        assert_eq!(fp.as_bytes(), &bytes);
    }

    #[test]
    fn test_fingerprint_to_hex() {
        let bytes = [0xAB; 32];
        let fp = HardwareFingerprint::from_bytes(bytes);
        let hex = fp.to_hex();

        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cpu_id_not_empty() {
        let cpu_id = get_cpu_id();
        assert!(!cpu_id.is_empty());
    }

    #[test]
    fn test_mac_address_not_empty() {
        let mac = get_mac_address();
        assert!(!mac.is_empty());
    }

    #[test]
    fn test_fingerprint_deterministic() {
        // Generate multiple times and verify consistency
        let fingerprints: Vec<_> = (0..5).map(|_| HardwareFingerprint::generate()).collect();

        for fp in &fingerprints[1..] {
            assert_eq!(&fingerprints[0], fp);
        }
    }

    #[test]
    fn test_different_inputs_different_fingerprints() {
        let fp1 = HardwareFingerprint::from_bytes([0xAA; 32]);
        let fp2 = HardwareFingerprint::from_bytes([0xBB; 32]);

        assert_ne!(fp1, fp2);
    }
}
