/// RAM detection and pipeline selection capsule (T0+T1 tier)
///
/// This module provides:
/// - RamDetectorCapsule: Detect available system RAM
/// - PipelineSelectorCapsule: Select optimal pipeline based on RAM + corpus size
/// - PipelineSelection enum: Result of selection logic (Fast or Streaming)

use std::io;

/// Pipeline selection decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineSelection {
    /// DedupPipeline (O(N) memory, ~136K docs/sec)
    Fast,
    /// StreamingDedupPipeline (O(1) 273 MB, ~30-100K docs/sec target)
    Streaming,
}

impl PipelineSelection {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            PipelineSelection::Fast => "DedupPipeline (Fast)",
            PipelineSelection::Streaming => "StreamingDedupPipeline (Safe)",
        }
    }
}

/// RAM detector capsule
pub struct RamDetectorCapsule;

impl RamDetectorCapsule {
    /// Get available RAM in GB
    pub fn available_ram_gb() -> io::Result<f64> {
        #[cfg(target_os = "linux")]
        {
            Self::available_ram_linux()
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(16.0)
        }
    }

    /// Get available RAM in bytes
    pub fn available_ram_bytes() -> u64 {
        // Multiply by bytes-per-GB first, then convert to avoid truncation when < 1 GB
        let gb = Self::available_ram_gb().unwrap_or(16.0);
        (gb * 1024.0 * 1024.0 * 1024.0) as u64
    }

    #[cfg(target_os = "linux")]
    fn available_ram_linux() -> io::Result<f64> {
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;

        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: f64 = parts[1].parse().unwrap_or(0.0);
                    return Ok(kb / (1024.0 * 1024.0));
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "MemAvailable not found in /proc/meminfo",
        ))
    }
}

/// Pipeline selector capsule
pub struct PipelineSelectorCapsule;

impl PipelineSelectorCapsule {
    /// Select optimal pipeline implementation
    pub fn select(
        num_docs: usize,
        available_ram_gb: Option<f64>,
        force_fast: bool,
        force_streaming: bool,
    ) -> PipelineSelection {
        if force_fast {
            return PipelineSelection::Fast;
        }
        if force_streaming {
            return PipelineSelection::Streaming;
        }

        let available_gb = available_ram_gb
            .or_else(|| RamDetectorCapsule::available_ram_gb().ok())
            .unwrap_or(16.0);

        let required_gb = Self::estimate_memory_gb(num_docs);

        let usable_ram = available_gb * 0.8;
        let required_with_margin = required_gb * 1.25;

        if required_with_margin < usable_ram {
            PipelineSelection::Fast
        } else {
            PipelineSelection::Streaming
        }
    }

    fn estimate_memory_gb(num_docs: usize) -> f64 {
        const BYTES_PER_DOC: f64 = 610.0;
        const SAFETY_FACTOR: f64 = 1.1;
        const OVERHEAD_MB: f64 = 200.0;

        let base_bytes = num_docs as f64 * BYTES_PER_DOC;
        let safe_bytes = base_bytes * SAFETY_FACTOR;
        let total_bytes = safe_bytes + (OVERHEAD_MB * 1024.0 * 1024.0);

        total_bytes / (1024.0 * 1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_detection() {
        let ram = RamDetectorCapsule::available_ram_gb();
        match ram {
            Ok(gb) => {
                assert!(gb > 0.0, "RAM must be positive");
                assert!(gb < 1000.0, "RAM unrealistic (> 1 PB)");
            }
            Err(_) => {
                let fallback_gb = RamDetectorCapsule::available_ram_gb().unwrap_or(16.0);
                assert_eq!(fallback_gb, 16.0);
            }
        }
    }

    #[test]
    fn test_ram_detection_bytes() {
        let bytes = RamDetectorCapsule::available_ram_bytes();
        // Must always return a positive value (either detected RAM or 16 GB fallback)
        assert!(bytes > 0, "RAM bytes must be positive, got: {}", bytes);
        assert!(bytes < 1_000_000_000_000_000, "RAM unrealistic (> 1 PB)");
    }

    #[test]
    fn test_selector_small_corpus() {
        let sel = PipelineSelectorCapsule::select(100_000, Some(16.0), false, false);
        assert_eq!(sel, PipelineSelection::Fast);
    }

    #[test]
    fn test_selector_large_corpus() {
        let sel = PipelineSelectorCapsule::select(1_000_000_000, Some(64.0), false, false);
        assert_eq!(sel, PipelineSelection::Streaming);
    }

    #[test]
    fn test_selector_medium_corpus_ample_ram() {
        let sel = PipelineSelectorCapsule::select(10_000_000, Some(64.0), false, false);
        assert_eq!(sel, PipelineSelection::Fast);
    }

    #[test]
    fn test_selector_medium_corpus_limited_ram() {
        let sel = PipelineSelectorCapsule::select(10_000_000, Some(8.0), false, false);
        assert_eq!(sel, PipelineSelection::Streaming);
    }

    #[test]
    fn test_force_fast() {
        let sel = PipelineSelectorCapsule::select(100_000_000, Some(8.0), true, false);
        assert_eq!(sel, PipelineSelection::Fast);
    }

    #[test]
    fn test_force_streaming() {
        let sel = PipelineSelectorCapsule::select(100_000, Some(64.0), false, true);
        assert_eq!(sel, PipelineSelection::Streaming);
    }

    #[test]
    fn test_force_flags_conflict() {
        let sel = PipelineSelectorCapsule::select(100_000, Some(64.0), true, true);
        assert_eq!(sel, PipelineSelection::Fast);
    }

    #[test]
    fn test_estimate_memory_1m_docs() {
        let memory_gb = PipelineSelectorCapsule::estimate_memory_gb(1_000_000);
        // Formula: ~0.82 GB for 1M docs (256B signature + 32B hash × 20 bands)
        assert!((0.80..0.85).contains(&memory_gb), "Got {}", memory_gb);
    }

    #[test]
    fn test_estimate_memory_10m_docs() {
        let memory_gb = PipelineSelectorCapsule::estimate_memory_gb(10_000_000);
        // Formula: ~6.4 GB for 10M docs
        assert!((6.40..6.50).contains(&memory_gb), "Got {}", memory_gb);
    }

    #[test]
    fn test_estimate_memory_100m_docs() {
        let memory_gb = PipelineSelectorCapsule::estimate_memory_gb(100_000_000);
        // Formula: ~62.7 GB for 100M docs
        assert!((62.50..62.80).contains(&memory_gb), "Got {}", memory_gb);
    }

    #[test]
    fn test_pipeline_selection_name() {
        assert_eq!(PipelineSelection::Fast.name(), "DedupPipeline (Fast)");
        assert_eq!(PipelineSelection::Streaming.name(), "StreamingDedupPipeline (Safe)");
    }
}
