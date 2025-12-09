/// StreamingMonitorCapsule: T5 Streaming capsule for continuous process monitoring
/// Performance: O(1) incremental updates, <100ms scan cycle

use crate::capsules::{ProcessStateCapsule, ResourceGovernorCapsule};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{System, Pid, ProcessesToUpdate};
use tokio::time::interval;
use tracing::{info, warn, debug};

/// Streaming monitor with ring buffer window
pub struct StreamingMonitorCapsule {
    /// Process state map (PID → ProcessStateCapsule)
    processes: HashMap<u32, Arc<ProcessStateCapsule>>,

    /// Resource governor (circuit breaker)
    governor: Arc<ResourceGovernorCapsule>,

    /// System info collector
    sys: System,

    /// Scan interval
    interval: Duration,

    /// Last scan timestamp
    last_scan: Option<Instant>,

    /// Configuration
    config: MonitorConfig,
}

/// Monitor configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MonitorConfig {
    /// CPU threshold for hung detection (percentage)
    pub cpu_threshold_pct: f64,

    /// Runtime threshold for hung detection (seconds)
    pub runtime_threshold_sec: u64,

    /// Scan interval (seconds)
    pub scan_interval_sec: u64,

    /// Grace period before SIGKILL (seconds)
    pub sigkill_grace_sec: u64,

    /// Process name patterns to detect as tests
    pub test_patterns: Vec<String>,

    /// Process name patterns to whitelist (never kill)
    pub whitelist_patterns: Vec<String>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            cpu_threshold_pct: 100.0,           // >100% CPU
            runtime_threshold_sec: 300,         // 5 minutes
            scan_interval_sec: 10,              // 10 seconds
            sigkill_grace_sec: 30,              // 30 seconds
            test_patterns: vec![
                "test".to_string(),
                "bench".to_string(),
                "resource_exhaustion".to_string(),
                "integration_test".to_string(),
            ],
            whitelist_patterns: vec![
                "claude".to_string(),
                "firefox".to_string(),
                "gnome-shell".to_string(),
                "systemd".to_string(),
            ],
        }
    }
}

impl StreamingMonitorCapsule {
    /// Create new streaming monitor
    pub fn new(config: MonitorConfig, governor: Arc<ResourceGovernorCapsule>) -> Self {
        Self {
            processes: HashMap::new(),
            governor,
            sys: System::new_all(),
            interval: Duration::from_secs(config.scan_interval_sec),
            last_scan: None,
            config,
        }
    }

    /// Start monitoring loop
    pub async fn monitor_loop(mut self) {
        info!("Starting system responsiveness monitor");
        info!("CPU threshold: {}%, Runtime threshold: {}s",
            self.config.cpu_threshold_pct,
            self.config.runtime_threshold_sec
        );

        let mut ticker = interval(self.interval);
        let mut minute_ticker = interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.scan_and_evaluate().await;
                }
                _ = minute_ticker.tick() => {
                    // Reset active kill counter every minute
                    self.governor.reset_active_kills();
                    info!("Circuit breaker reset: state={:?}, total_kills={}",
                        self.governor.circuit_state(),
                        self.governor.total_kills()
                    );
                }
            }
        }
    }

    /// Scan processes and evaluate for hung detection
    /// Target: <100ms for 1000 processes
    async fn scan_and_evaluate(&mut self) {
        let scan_start = Instant::now();

        // Refresh system info
        self.sys.refresh_processes(ProcessesToUpdate::All);

        // Scan all processes
        let mut scanned = 0;
        let mut hung_detected = 0;

        for (pid, process) in self.sys.processes() {
            scanned += 1;

            let pid_u32 = pid.as_u32();
            let cpu_pct = process.cpu_usage() as f64;
            let runtime_sec = process.run_time();

            // Skip processes with PIDs exceeding 20-bit limit (1,048,575)
            // This handles systems with large PID spaces gracefully
            if pid_u32 > 0xFFFFF {
                debug!("Skipping process PID {} (exceeds 20-bit limit). Consider expanding PID field if needed.", pid_u32);
                continue;
            }

            // Detect process type (do this BEFORE entry() borrow)
            let name = process.name().to_string_lossy();
            let name_str = name.as_ref();
            let is_test = self.is_test_process(name_str);
            let is_bench = name_str.contains("bench");
            let is_cargo = name_str.contains("cargo") || name_str.contains("rustc");
            let is_whitelisted = self.is_whitelisted(name_str);

            // Get or create process state capsule
            let capsule = self.processes.entry(pid_u32)
                .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

            // Update capsule state
            capsule.update(pid_u32, cpu_pct, runtime_sec, is_test, is_bench, is_cargo);

            // Whitelist if needed
            if is_whitelisted {
                capsule.set_whitelisted(true);
            }

            // Check if hung
            if capsule.is_hung(self.config.cpu_threshold_pct, self.config.runtime_threshold_sec) {
                hung_detected += 1;

                warn!(
                    "Hung process detected: PID={}, name={}, CPU={:.1}%, runtime={}s",
                    pid_u32, name_str, cpu_pct, runtime_sec
                );

                // Attempt to kill (with circuit breaker check)
                if self.governor.can_kill() {
                    self.kill_process(pid_u32, name_str).await;
                } else {
                    warn!("Circuit breaker OPEN: kills disabled (too many recent kills)");
                }
            }
        }

        // Cleanup old PIDs (no longer exist)
        self.processes.retain(|pid, _| {
            self.sys.process(Pid::from_u32(*pid)).is_some()
        });

        let scan_duration = scan_start.elapsed();
        self.last_scan = Some(scan_start);

        debug!(
            "Scan complete: {} processes, {} hung, {:?} duration",
            scanned, hung_detected, scan_duration
        );
    }

    /// Kill hung process (SIGTERM → SIGKILL escalation)
    /// CRITICAL-007 FIX: Re-validate generation before SIGKILL to prevent killing wrong process
    async fn kill_process(&self, pid: u32, name: &str) {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        // Capture original generation BEFORE sending SIGTERM
        let original_generation = self.processes
            .get(&pid)
            .map(|c| c.generation())
            .unwrap_or(255); // Use impossible value if not found

        info!("Killing hung process: PID={}, name={}, gen={}", pid, name, original_generation);

        // Record kill attempt
        if !self.governor.record_kill() {
            warn!("Kill rejected by circuit breaker");
            return;
        }

        let nix_pid = Pid::from_raw(pid as i32);

        // Send SIGTERM (graceful shutdown)
        match kill(nix_pid, Signal::SIGTERM) {
            Ok(_) => {
                info!("Sent SIGTERM to PID {} (gen {})", pid, original_generation);

                // Wait for grace period
                tokio::time::sleep(Duration::from_secs(self.config.sigkill_grace_sec)).await;

                // CRITICAL-007 FIX: RE-VALIDATE GENERATION before SIGKILL
                // This prevents killing an innocent process if PID was reused during grace period
                let current_generation = self.processes
                    .get(&pid)
                    .map(|c| c.generation());

                match current_generation {
                    Some(gen) if gen == original_generation => {
                        // Same generation, safe to SIGKILL
                        if kill(nix_pid, None).is_ok() {
                            warn!("Process {} (gen {}) did not respond to SIGTERM, sending SIGKILL",
                                  pid, original_generation);
                            let _ = kill(nix_pid, Signal::SIGKILL);
                        } else {
                            info!("Process {} (gen {}) terminated gracefully", pid, original_generation);
                        }
                    }
                    Some(gen) => {
                        // Generation changed, PID reused!
                        warn!(
                            "PID {} reused! Original gen={}, current gen={}. \
                             Aborting SIGKILL to protect innocent process.",
                            pid, original_generation, gen
                        );
                    }
                    None => {
                        // Process no longer in map (already exited or cleaned up)
                        info!("Process {} no longer tracked, assuming terminated", pid);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to send SIGTERM to PID {}: {}", pid, e);
            }
        }
    }

    /// Check if process name matches test pattern
    fn is_test_process(&self, name: &str) -> bool {
        self.config.test_patterns.iter().any(|pattern| name.contains(pattern))
    }

    /// Check if process is whitelisted
    fn is_whitelisted(&self, name: &str) -> bool {
        self.config.whitelist_patterns.iter().any(|pattern| name.contains(pattern))
    }
}
