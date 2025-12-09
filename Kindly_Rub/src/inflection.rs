use crate::motion::MotionTempo;
use crate::sampler::MotionSample;

#[derive(Debug, Clone, Copy)]
pub struct InflectionTuning {
    pub base_buzz_threshold_ms: f32,
    pub vibration_scale: f32,
    pub min_ms: f32,
    pub max_ms: f32,
}

impl Default for InflectionTuning {
    fn default() -> Self {
        Self {
            base_buzz_threshold_ms: 120.0,
            vibration_scale: 0.5,
            min_ms: 40.0,
            max_ms: 400.0,
        }
    }
}

impl InflectionTuning {
    pub fn threshold_for(&self, range: (u8, u8), tempo: MotionTempo) -> f32 {
        let span = (range.1.saturating_sub(range.0)).max(1) as f32;
        let span_scale = if span < 30.0 { 0.7 } else if span > 80.0 { 1.1 } else { 1.0 };
        let tempo_scale = match tempo {
            MotionTempo::Lent => 1.0,
            MotionTempo::Moyen => 0.75,
            MotionTempo::Rapide => self.vibration_scale,
        };
        let raw = self.base_buzz_threshold_ms * span_scale * tempo_scale;
        raw.clamp(self.min_ms, self.max_ms)
    }

    pub fn retune_from_samples(&self, samples: &[MotionSample], target_buzz_per_sec: f32) -> Self {
        if samples.len() < 4 || target_buzz_per_sec <= 0.0 {
            return *self;
        }
        let Some(median) = median_delta(samples) else {
            return *self;
        };
        let desired_delta = (1000.0 / target_buzz_per_sec).max(self.min_ms).min(self.max_ms);
        let base_buzz_threshold_ms = (median * 0.6).min(desired_delta).max(self.min_ms);
        let mut tuned = *self;
        tuned.base_buzz_threshold_ms = base_buzz_threshold_ms;
        tuned
    }

    pub fn calibrate_with_report(
        &self,
        samples: &[MotionSample],
        target_buzz_per_sec: f32,
    ) -> CalibrationReport {
        let tuned = self.retune_from_samples(samples, target_buzz_per_sec);
        let median = median_delta(samples).unwrap_or(0.0);
        let measured_buzz_per_sec = if median > 0.0 { 1000.0 / median } else { 0.0 };
        CalibrationReport {
            tuned,
            measured_interval_ms: median,
            measured_buzz_per_sec,
            sample_count: samples.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflectionKind {
    ImpactHigh,
    ImpactLow,
    Buzz,
}

#[derive(Debug, Clone, Copy)]
pub struct InflectionEvent {
    pub time_ms: u64,
    pub position_pct: f32,
    pub kind: InflectionKind,
}
#[derive(Debug, Clone, Copy)]
pub struct CalibrationReport {
    pub tuned: InflectionTuning,
    pub measured_interval_ms: f32,
    pub measured_buzz_per_sec: f32,
    pub sample_count: usize,
}

pub fn detect_inflections(samples: &[MotionSample], buzz_threshold_ms: f32) -> Vec<InflectionEvent> {
    detect_with_threshold(samples, buzz_threshold_ms)
}

pub fn detect_inflections_tuned(
    samples: &[MotionSample],
    tuning: &InflectionTuning,
    range: (u8, u8),
    tempo: MotionTempo,
) -> Vec<InflectionEvent> {
    let threshold = tuning.threshold_for(range, tempo);
    detect_with_threshold(samples, threshold)
}

fn detect_with_threshold(samples: &[MotionSample], buzz_threshold_ms: f32) -> Vec<InflectionEvent> {
    if samples.len() < 3 {
        return Vec::new();
    }
    let mut events = Vec::new();
    let mut last_event_time = None;
    for trio in samples.windows(3) {
        let delta_prev = trio[1].position_pct - trio[0].position_pct;
        let delta_next = trio[2].position_pct - trio[1].position_pct;
        let sign_prev = sign(delta_prev);
        let sign_next = sign(delta_next);

        if sign_prev != 0 && sign_next != 0 && sign_prev != sign_next {
            let position = trio[1].position_pct;
            let time = trio[1].time_ms;
            let mut kind = if sign_prev > 0 && sign_next < 0 {
                InflectionKind::ImpactHigh
            } else {
                InflectionKind::ImpactLow
            };
            if let Some(prev) = last_event_time {
                let delta = (time as f32 - prev as f32).abs();
                if delta <= buzz_threshold_ms {
                    kind = InflectionKind::Buzz;
                }
            }
            last_event_time = Some(time);
            events.push(InflectionEvent {
                time_ms: time,
                position_pct: position,
                kind,
            });
        }
    }
    events
}

fn sign(v: f32) -> i8 {
    if v > 1e-2 {
        1
    } else if v < -1e-2 {
        -1
    } else {
        0
    }
}

fn median_delta(samples: &[MotionSample]) -> Option<f32> {
    let mut deltas = Vec::new();
    for w in samples.windows(2) {
        let dt = (w[1].time_ms as f32 - w[0].time_ms as f32).abs();
        if dt > 0.0 {
            deltas.push(dt);
        }
    }
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(deltas[deltas.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};
    use crate::sampler::sample_motion_block;
    use crate::timeline::TimelineEntry;

    #[test]
    fn detects_high_and_low() {
        let block =
            MotionBlockCapsule::new(10, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 1000);
        let entry = TimelineEntry::new(0, 1000, block);
        let samples = sample_motion_block(&entry, 60);
        let events = detect_inflections(&samples, 80.0);

        assert!(!events.is_empty());
        assert!(events.iter().any(|e| e.kind == InflectionKind::ImpactHigh));
        assert!(events.iter().any(|e| e.kind == InflectionKind::ImpactLow));
    }

    #[test]
    fn fast_cycle_marks_buzz() {
        let block =
            MotionBlockCapsule::new(11, MotionPattern::Vibration, 40, 60, MotionTempo::Rapide, 300);
        let entry = TimelineEntry::new(0, 400, block);
        let samples = sample_motion_block(&entry, 120);
        let events = detect_inflections(&samples, 200.0);
        assert!(events.iter().any(|e| e.kind == InflectionKind::Buzz));
    }

    #[test]
    fn tuning_scales_threshold() {
        let tuning = InflectionTuning::default();
        let fast = tuning.threshold_for((70, 100), MotionTempo::Rapide);
        let slow = tuning.threshold_for((0, 100), MotionTempo::Lent);
        assert!(fast < slow);
        assert!(fast >= tuning.min_ms);
    }

    #[test]
    fn retune_from_samples_tracks_density() {
        let tuning = InflectionTuning::default();
        let samples = vec![
            MotionSample { time_ms: 0, position_pct: 0.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 20, position_pct: 10.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 40, position_pct: 20.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 60, position_pct: 30.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
        ];
        let retuned = tuning.retune_from_samples(&samples, 10.0);
        assert!(retuned.base_buzz_threshold_ms <= tuning.base_buzz_threshold_ms);
    }

    #[test]
    fn calibration_report_returns_stats() {
        let tuning = InflectionTuning::default();
        let samples = vec![
            MotionSample { time_ms: 0, position_pct: 0.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 25, position_pct: 10.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 50, position_pct: 20.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            MotionSample { time_ms: 75, position_pct: 30.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
        ];
        let report = tuning.calibrate_with_report(&samples, 12.0);
        assert!(report.tuned.base_buzz_threshold_ms <= tuning.base_buzz_threshold_ms);
        assert!(report.measured_interval_ms > 0.0);
        assert!(report.measured_buzz_per_sec > 0.0);
        assert_eq!(report.sample_count, samples.len());
    }
}
