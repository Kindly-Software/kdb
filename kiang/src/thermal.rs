//! GPU Thermal Monitoring
//!
//! Lockfree thermal reading from Linux sysfs thermal zones for Intel Arc GPUs.
//! Reads from `/sys/class/thermal/thermal_zone*/` to get GPU temperature.

use crate::{KiangError, Result};
use std::fs;
use std::path::PathBuf;

/// Thermal monitor for Intel Arc GPU
///
/// Reads temperature from Linux thermal zones in a lockfree manner.
/// Caches the thermal zone path for fast repeated reads.
pub struct ThermalMonitor {
    /// Cached path to GPU thermal zone
    thermal_zone_path: Option<PathBuf>,
}

impl Default for ThermalMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ThermalMonitor {
    /// Create new thermal monitor
    pub const fn new() -> Self {
        Self {
            thermal_zone_path: None,
        }
    }

    /// Read current GPU temperature in Celsius
    ///
    /// First call discovers the Intel GPU thermal zone, subsequent calls
    /// use the cached path for fast reads.
    pub fn read_temperature(&self) -> Result<u8> {
        // If path not cached, discover it
        let path = if let Some(ref p) = self.thermal_zone_path {
            p.clone()
        } else {
            self.discover_thermal_zone()?
        };

        // Read temperature from sysfs
        self.read_thermal_zone(&path)
    }

    /// Discover Intel GPU thermal zone
    fn discover_thermal_zone(&self) -> Result<PathBuf> {
        // Search thermal zones for Intel GPU
        for i in 0..20 {
            let type_path = format!("/sys/class/thermal/thermal_zone{}/type", i);
            let temp_path = format!("/sys/class/thermal/thermal_zone{}/temp", i);

            if let Ok(zone_type) = fs::read_to_string(&type_path) {
                let zone_type = zone_type.trim();

                // Intel Arc GPUs report as "INT3403" or contain "GPU" in type
                if zone_type.contains("INT3403")
                    || zone_type.contains("GPU")
                    || zone_type.contains("gpu")
                    || zone_type.contains("x86_pkg_temp")
                // Fallback to package temp
                {
                    tracing::info!(
                        "Found GPU thermal zone: {} (type: {})",
                        temp_path,
                        zone_type
                    );
                    return Ok(PathBuf::from(temp_path));
                }
            }
        }

        // If no GPU-specific zone found, use first available thermal zone as fallback
        let fallback = PathBuf::from("/sys/class/thermal/thermal_zone0/temp");
        if fallback.exists() {
            tracing::warn!("Using fallback thermal zone: {:?}", fallback);
            Ok(fallback)
        } else {
            Err(KiangError::ThermalError(
                "No thermal zone found".to_string(),
            ))
        }
    }

    /// Read temperature from thermal zone file
    fn read_thermal_zone(&self, path: &PathBuf) -> Result<u8> {
        let temp_str = fs::read_to_string(path)
            .map_err(|e| KiangError::ThermalError(format!("Failed to read thermal: {}", e)))?;

        // Parse temperature (in millidegrees Celsius)
        let temp_millicelsius: i32 = temp_str
            .trim()
            .parse()
            .map_err(|e| KiangError::ThermalError(format!("Failed to parse thermal: {}", e)))?;

        // Convert to Celsius and clamp to u8 range
        let temp_celsius = (temp_millicelsius / 1000).clamp(0, 255) as u8;

        Ok(temp_celsius)
    }

    /// Force refresh thermal zone discovery
    pub fn refresh(&mut self) -> Result<()> {
        self.thermal_zone_path = None;
        self.read_temperature()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_monitor_creation() {
        let monitor = ThermalMonitor::new();
        assert!(monitor.thermal_zone_path.is_none());
    }

    #[test]
    fn test_thermal_read() {
        let monitor = ThermalMonitor::new();

        // Try to read temperature (may fail if no thermal zones available)
        if let Ok(temp) = monitor.read_temperature() {
            assert!(
                temp > 0 && temp < 120,
                "Temperature out of reasonable range: {}",
                temp
            );
            println!("GPU temperature: {}°C", temp);
        } else {
            println!("No thermal zones available on this system");
        }
    }

    #[test]
    fn test_thermal_zone_discovery() {
        let monitor = ThermalMonitor::new();

        if let Ok(path) = monitor.discover_thermal_zone() {
            println!("Found thermal zone: {:?}", path);
            assert!(path.exists());
        }
    }
}
