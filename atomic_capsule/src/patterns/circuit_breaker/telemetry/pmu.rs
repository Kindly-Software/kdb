//! Linux perf-event backed telemetry collector.

use std::time::{Duration, Instant};

use perfcnt::linux::{HardwareEventType, PerfCounter, PerfCounterBuilderLinux};
use perfcnt::AbstractPerfCounter;

use super::{TelemetrySample, TelemetrySource};
use crate::patterns::circuit_breaker::cause;

/// Abstraction over hardware performance counters.
pub trait Counter {
    /// Start counting.
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    /// Read the current counter value.
    fn read(&mut self) -> Result<u64, Box<dyn std::error::Error>>;
}

impl Counter for PerfCounter {
    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        AbstractPerfCounter::start(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn read(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        AbstractPerfCounter::read(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

/// Configuration for [`PmuCollector`].
#[derive(Clone, Debug)]
pub struct PmuConfig {
    /// Budget for mean utilisation (cycles) used to normalise `mu_norm`.
    pub mu_budget_cycles: f32,
    /// Budget for jitter (last level cache misses) used to normalise `sg_norm`.
    pub sg_budget_misses: f32,
    /// Number of cache-miss deltas that map to a single error increment.
    pub miss_err_window: u64,
    /// Threshold ratio (misses/cycles) that triggers the `cause::IO` flag.
    pub miss_ratio_cause: f32,
    /// Optional backoff hint sequence applied when ratio exceeds multiples.
    pub backoff_levels: [u8; 3],
    /// Polling interval used for normalising deltas.
    pub interval: Duration,
}

impl Default for PmuConfig {
    fn default() -> Self {
        Self {
            mu_budget_cycles: 50_000.0,
            sg_budget_misses: 5_000.0,
            miss_err_window: 1_000,
            miss_ratio_cause: 0.02,
            backoff_levels: [1, 2, 3],
            interval: Duration::from_millis(50),
        }
    }
}

struct Snapshot {
    cycles: u64,
    misses: u64,
    last: Instant,
}

/// Collector reading hardware performance counters and mapping them to breaker telemetry.
pub struct PmuCollector<C = PerfCounter, M = PerfCounter>
where
    C: Counter,
    M: Counter,
{
    cycles: C,
    misses: M,
    config: PmuConfig,
    snapshot: Snapshot,
}

impl PmuCollector<PerfCounter, PerfCounter> {
    /// Create a collector for the current CPU using hardware perf events.
    pub fn new(config: PmuConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let cycles =
            PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CPUCycles).finish()?;
        let misses = PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CacheMisses)
            .finish()?;

        cycles.start()?;
        misses.start()?;

        Ok(Self::with_counters(config, cycles, misses)?)
    }
}

impl<C: Counter, M: Counter> PmuCollector<C, M> {
    /// Construct a collector from custom counters (primarily for testing).
    pub fn with_counters(
        config: PmuConfig,
        mut cycles: C,
        mut misses: M,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        cycles.start()?;
        misses.start()?;

        let now = Instant::now();
        let snapshot = Snapshot {
            cycles: cycles.read()?,
            misses: misses.read()?,
            last: now,
        };

        Ok(Self {
            cycles,
            misses,
            config,
            snapshot,
        })
    }

    /// Access the collector configuration.
    #[must_use]
    pub fn config(&self) -> &PmuConfig {
        &self.config
    }

    fn compute_sample(
        &mut self,
        now: Instant,
    ) -> Result<TelemetrySample, Box<dyn std::error::Error>> {
        let current_cycles = self.cycles.read()?;
        let current_misses = self.misses.read()?;

        let delta_cycles = current_cycles.saturating_sub(self.snapshot.cycles);
        let delta_misses = current_misses.saturating_sub(self.snapshot.misses);
        let elapsed = now.saturating_duration_since(self.snapshot.last);

        self.snapshot = Snapshot {
            cycles: current_cycles,
            misses: current_misses,
            last: now,
        };

        if elapsed < self.config.interval {
            return Ok(TelemetrySample::zero());
        }

        let mu_norm = if self.config.mu_budget_cycles > 0.0 {
            (delta_cycles as f32) / self.config.mu_budget_cycles
        } else {
            0.0
        };

        let sg_norm = if self.config.sg_budget_misses > 0.0 {
            (delta_misses as f32) / self.config.sg_budget_misses
        } else {
            0.0
        };

        let err_inc = if self.config.miss_err_window == 0 {
            0
        } else {
            (delta_misses / self.config.miss_err_window).min(u16::MAX as u64) as u16
        };

        let miss_ratio = if delta_cycles == 0 {
            0.0
        } else {
            (delta_misses as f32) / (delta_cycles as f32)
        };

        let mut cause = 0u8;
        if miss_ratio >= self.config.miss_ratio_cause {
            cause |= cause::IO;
        }

        let backoff_hint = if miss_ratio >= self.config.miss_ratio_cause * 3.0 {
            Some(self.config.backoff_levels[2].min(63))
        } else if miss_ratio >= self.config.miss_ratio_cause * 2.0 {
            Some(self.config.backoff_levels[1].min(63))
        } else if miss_ratio >= self.config.miss_ratio_cause {
            Some(self.config.backoff_levels[0].min(63))
        } else {
            None
        };

        Ok(TelemetrySample {
            mu_norm,
            sg_norm,
            err_inc,
            cause,
            backoff_hint,
        }
        .clamped())
    }
}

impl<C: Counter, M: Counter> TelemetrySource for PmuCollector<C, M> {
    fn poll(&mut self) -> TelemetrySample {
        let now = Instant::now();
        match self.compute_sample(now) {
            Ok(sample) => sample,
            Err(_) => TelemetrySample::zero(),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    struct MockCounter {
        values: Vec<u64>,
        idx: usize,
    }

    impl MockCounter {
        fn new(values: Vec<u64>) -> Self {
            Self { values, idx: 0 }
        }
    }

    impl Counter for MockCounter {
        fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }

        fn read(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
            let value = self
                .values
                .get(self.idx)
                .copied()
                .unwrap_or(*self.values.last().unwrap_or(&0));
            if self.idx + 1 < self.values.len() {
                self.idx += 1;
            }
            Ok(value)
        }
    }

    #[test]
    fn pmu_collector_normalises_values() {
        let config = PmuConfig {
            mu_budget_cycles: 100.0,
            sg_budget_misses: 10.0,
            miss_err_window: 5,
            miss_ratio_cause: 0.2,
            backoff_levels: [2, 4, 6],
            interval: Duration::from_millis(1),
        };
        let mut collector = PmuCollector::with_counters(
            config,
            MockCounter::new(vec![0, 200]),
            MockCounter::new(vec![0, 40]),
        )
        .expect("build collector");

        let sample = collector.poll();
        assert!(sample.mu_norm >= 2.0);
        assert!(sample.sg_norm >= 4.0);
        assert!(sample.err_inc >= 8);
        assert!(sample.cause & cause::IO != 0);
        assert_eq!(sample.backoff_hint, Some(6));
    }
}
