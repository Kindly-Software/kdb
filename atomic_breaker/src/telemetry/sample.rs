//! Generic telemetry primitives shared by collectors.

/// Normalised telemetry snapshot produced by a [`TelemetrySource`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetrySample {
    /// Normalised mean metric (ratio against budget).
    pub mu_norm: f32,
    /// Normalised jitter metric (ratio against budget).
    pub sg_norm: f32,
    /// Increment to apply to the breaker error counter.
    pub err_inc: u16,
    /// Cause bits suggested by the telemetry source.
    pub cause: u8,
    /// Optional backoff hint (0-63). Ignored on compact layouts.
    pub backoff_hint: Option<u8>,
}

impl TelemetrySample {
    /// Create an empty sample.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            mu_norm: 0.0,
            sg_norm: 0.0,
            err_inc: 0,
            cause: 0,
            backoff_hint: None,
        }
    }

    /// Clamp ratios into a sane range to protect fixed-point packing.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            mu_norm: self.mu_norm.clamp(0.0, 256.0),
            sg_norm: self.sg_norm.clamp(0.0, 256.0),
            err_inc: self.err_inc,
            cause: self.cause,
            backoff_hint: self.backoff_hint.map(|hint| hint.min(63)),
        }
    }

    /// Merge another sample by taking maximum ratios and summed errors.
    #[must_use]
    pub fn merged(self, other: Self) -> Self {
        Self {
            mu_norm: self.mu_norm.max(other.mu_norm),
            sg_norm: self.sg_norm.max(other.sg_norm),
            err_inc: self.err_inc.saturating_add(other.err_inc),
            cause: self.cause | other.cause,
            backoff_hint: self.backoff_hint.or(other.backoff_hint),
        }
    }
}

impl Default for TelemetrySample {
    fn default() -> Self {
        Self::zero()
    }
}

/// Common trait for telemetry providers.
pub trait TelemetrySource {
    /// Acquire the latest telemetry sample.
    fn poll(&mut self) -> TelemetrySample;
}

/// Convenience telemetry source producing predefined samples.
#[derive(Clone, Debug)]
pub struct MockSource {
    samples: Vec<TelemetrySample>,
    next: usize,
}

impl MockSource {
    /// Construct a mock source from a sequence of samples.
    #[must_use]
    pub fn new(samples: Vec<TelemetrySample>) -> Self {
        Self { samples, next: 0 }
    }
}

impl TelemetrySource for MockSource {
    fn poll(&mut self) -> TelemetrySample {
        if self.samples.is_empty() {
            return TelemetrySample::zero();
        }
        let sample = self.samples[self.next];
        self.next = (self.next + 1) % self.samples.len();
        sample
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::cause;

    #[test]
    fn mock_source_cycles_samples() {
        let samples = vec![
            TelemetrySample {
                mu_norm: 1.0,
                sg_norm: 0.5,
                err_inc: 1,
                cause: cause::CPU,
                backoff_hint: Some(3),
            },
            TelemetrySample::zero(),
        ];
        let mut mock = MockSource::new(samples.clone());
        assert_eq!(mock.poll(), samples[0]);
        assert_eq!(mock.poll(), samples[1]);
        assert_eq!(mock.poll(), samples[0]);
    }

    #[test]
    fn merging_respects_maxima() {
        let a = TelemetrySample {
            mu_norm: 2.0,
            sg_norm: 1.0,
            err_inc: 3,
            cause: cause::LAT,
            backoff_hint: Some(5),
        };
        let b = TelemetrySample {
            mu_norm: 1.0,
            sg_norm: 2.5,
            err_inc: 4,
            cause: cause::CPU,
            backoff_hint: None,
        };
        let merged = a.merged(b);
        assert_eq!(merged.mu_norm, 2.0);
        assert_eq!(merged.sg_norm, 2.5);
        assert_eq!(merged.err_inc, 7);
        assert_eq!(merged.cause, cause::LAT | cause::CPU);
        assert_eq!(merged.backoff_hint, Some(5));
    }
}
