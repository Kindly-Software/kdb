use crate::motion::MotionTempo;
use crate::timeline::TimelineEntry;

#[derive(Debug, Clone, Copy)]
pub struct MotionSample {
    pub time_ms: u64,
    pub position_pct: f32,
    pub velocity_pct_per_ms: f32,
    pub acceleration_pct_per_ms2: f32,
}

pub fn sample_motion_block(entry: &TimelineEntry, fps: u32) -> Vec<MotionSample> {
    let effective_duration_ms = entry.effective_duration_ms();
    if effective_duration_ms == 0 || fps == 0 {
        return Vec::new();
    }

    let range = entry.block.range();
    let (start_pct, end_pct) = (range.0 as f32, range.1 as f32);
    let step_ms = 1000.0 / fps.max(1) as f32;

    let stretch_factor = entry.stretch_ppm as f32 / 1_000_000.0;
    let period_ms = tempo_period_ms(entry.block.tempo(), stretch_factor);

    let mut samples = Vec::new();
    let total_steps = ((effective_duration_ms as f32) / step_ms).ceil() as u64;
    for step in 0..=total_steps {
        let t_ms = (step as f32 * step_ms).min(effective_duration_ms as f32);
        let phase = (t_ms % period_ms) / period_ms;
        let (position_pct, velocity, accel) = hermite_wave(start_pct, end_pct, period_ms, phase);
        let velocity = velocity / 1000.0; // derivative is per second; convert to per ms
        let accel = accel / 1_000_000.0; // per ms^2
        samples.push(MotionSample {
            time_ms: t_ms.round() as u64,
            position_pct,
            velocity_pct_per_ms: velocity,
            acceleration_pct_per_ms2: accel,
        });
    }

    samples
}

fn tempo_period_ms(tempo: MotionTempo, stretch_factor: f32) -> f32 {
    let base: f32 = match tempo {
        MotionTempo::Lent => 1800.0,
        MotionTempo::Moyen => 900.0,
        MotionTempo::Rapide => 300.0,
    };
    let stretch = stretch_factor.max(0.25).min(4.0);
    (base * stretch).clamp(50.0, 4000.0)
}

fn hermite_wave(start: f32, end: f32, period_ms: f32, phase: f32) -> (f32, f32, f32) {
    // Symmetric stroke: ease-in-out forward then ease-in-out backward using cubic Hermite (smoothstep)
    let amplitude = end - start;
    let (dir, local_t) = if phase < 0.5 {
        (1.0, phase * 2.0)
    } else {
        (-1.0, (phase - 0.5) * 2.0)
    };

    // Smoothstep Hermite basis: p(t) = 3t^2 - 2t^3
    let t = local_t;
    let p = 3.0 * t * t - 2.0 * t * t * t;
    let dp = 6.0 * t - 6.0 * t * t; // derivative w.r.t t
    let ddp = 6.0 - 12.0 * t; // second derivative w.r.t t

    let position = if dir > 0.0 {
        start + amplitude * p
    } else {
        end - amplitude * p
    };
    let velocity = dir * amplitude * dp * (2.0 / period_ms); // chain rule: local_t spans half-period
    let accel = dir * amplitude * ddp * (4.0 / (period_ms * period_ms));

    (position, velocity, accel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};
    use crate::timeline::TimelineEntry;

    #[test]
    fn samples_follow_range_and_duration() {
        let block =
            MotionBlockCapsule::new(1, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 1000);
        let entry = TimelineEntry::new(0, 1000, block);
        let samples = sample_motion_block(&entry, 60);

        assert!(!samples.is_empty());
        assert_eq!(samples.first().unwrap().time_ms, 0);
        assert!(samples.last().unwrap().time_ms >= 1000);

        let min_pos = samples
            .iter()
            .map(|s| s.position_pct)
            .fold(f32::INFINITY, f32::min);
        let max_pos = samples
            .iter()
            .map(|s| s.position_pct)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(min_pos >= -0.01, "min_pos {}", min_pos);
        assert!(max_pos <= 100.01, "max_pos {}", max_pos);
    }

    #[test]
    fn stretch_slows_frequency() {
        let block =
            MotionBlockCapsule::new(2, MotionPattern::Linear, 0, 100, MotionTempo::Rapide, 500);
        let stretched_entry = TimelineEntry::new(0, 500, block.clone()).with_stretch_ppm(2_000_000);
        let fast_entry = TimelineEntry::new(0, 500, block);

        let stretched_samples = sample_motion_block(&stretched_entry, 60);
        let fast_samples = sample_motion_block(&fast_entry, 60);

        let stretched_high = count_inflections(&stretched_samples);
        let fast_high = count_inflections(&fast_samples);

        let stretched_rate =
            stretched_high as f32 * 1000.0 / stretched_entry.effective_duration_ms().max(1) as f32;
        let fast_rate = fast_high as f32 * 1000.0 / fast_entry.effective_duration_ms().max(1) as f32;

        assert!(stretched_rate < fast_rate);
    }

    fn count_inflections(samples: &[MotionSample]) -> usize {
        let mut count = 0;
        let mut last_sign = 0i8;
        for pair in samples.windows(2) {
            let v = pair[1].velocity_pct_per_ms;
            let sign = if v > 0.0 { 1 } else if v < 0.0 { -1 } else { 0 };
            if sign != 0 && sign != last_sign && last_sign != 0 {
                count += 1;
            }
            if sign != 0 {
                last_sign = sign;
            }
        }
        count
    }

    #[test]
    fn stretched_shape_matches_base() {
        let block =
            MotionBlockCapsule::new(5, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 1000);
        let base = TimelineEntry::new(0, 1000, block.clone());
        let stretched = TimelineEntry::new(0, 1000, block).with_stretch_ppm(1_500_000);

        let base_samples = sample_motion_block(&base, 120);
        let stretched_samples = sample_motion_block(&stretched, 120);
        let base_duration = base_samples.last().unwrap().time_ms as f32;
        let stretched_duration = stretched_samples.last().unwrap().time_ms as f32;

        let fetch_pos = |samples: &[MotionSample], duration: f32, norm: f32| {
            let target = duration * norm;
            samples
                .iter()
                .min_by_key(|s| s.time_ms.abs_diff(target as u64))
                .map(|s| s.position_pct)
                .unwrap_or(0.0)
        };

        for step in 0..=10 {
            let norm = step as f32 / 10.0;
            let base_pos = fetch_pos(&base_samples, base_duration, norm);
            let stretched_pos = fetch_pos(&stretched_samples, stretched_duration, norm);
            assert!(
                (base_pos - stretched_pos).abs() < 1.5,
                "mismatch at norm {} ({} vs {})",
                norm,
                base_pos,
                stretched_pos
            );
        }
    }
}
