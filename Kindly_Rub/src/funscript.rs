use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunscriptAction {
    pub at: u64,
    pub pos: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunscriptValidation {
    pub monotonic: bool,
    pub clamped: bool,
    pub density_ok: bool,
    pub total_actions: usize,
    pub duration_ms: u64,
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct FunscriptCapsule {
    generation: AtomicU64,
    max_density_per_sec: u32,
}

impl FunscriptCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            max_density_per_sec: 200,
        }
    }

    pub fn set_max_density(&mut self, max_per_sec: u32) {
        self.max_density_per_sec = max_per_sec.max(1);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn from_positions(&self, positions: &[(u64, f32)]) -> (Vec<FunscriptAction>, FunscriptValidation) {
        let mut actions: Vec<FunscriptAction> = positions
            .iter()
            .map(|(t, p)| FunscriptAction {
                at: *t,
                pos: clamp_pos(*p),
            })
            .collect();
        actions.sort_by_key(|a| a.at);
        // Density control and deduplication at identical timestamps
        actions.dedup_by_key(|a| a.at);
        actions = enforce_density(actions, self.max_density_per_sec);
        self.generation.fetch_add(1, Ordering::Relaxed);
        let validation = validate_actions(&actions, self.max_density_per_sec);
        (actions, validation)
    }

    pub fn to_json(&self, actions: &[FunscriptAction]) -> String {
        let mut out = String::new();
        out.push_str(r#"{"version":"1.0","inverted":false,"actions":["#);
        for (idx, action) in actions.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            // Simple JSON object without escaping needed
            out.push_str("{\"at\":");
            out.push_str(&action.at.to_string());
            out.push_str(",\"pos\":");
            out.push_str(&action.pos.to_string());
            out.push('}');
        }
        out.push_str("]}");
        out
    }
}

impl Default for FunscriptCapsule {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp_pos(p: f32) -> u8 {
    if p.is_nan() {
        0
    } else {
        p.clamp(0.0, 100.0).round() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_sorts_actions() {
        let writer = FunscriptCapsule::new();
        let (actions, validation) =
            writer.from_positions(&[(50, 110.0), (10, -5.0), (10, 30.0)]);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].at, 10);
        assert_eq!(actions[0].pos, 0);
        assert_eq!(actions[1].pos, 100);
        assert!(validation.monotonic);
        assert!(validation.clamped);
    }

    #[test]
    fn renders_json() {
        let writer = FunscriptCapsule::new();
        let (actions, _) = writer.from_positions(&[(0, 0.0), (100, 100.0)]);
        let json = writer.to_json(&actions);
        assert!(json.starts_with(r#"{"version":"1.0","inverted":false,"actions":["#));
        assert!(json.contains(r#"{"at":0,"pos":0}"#));
        assert!(json.contains(r#"{"at":100,"pos":100}"#));
        assert!(json.ends_with("]}"));
    }

    #[test]
    fn enforces_density() {
        let mut writer = FunscriptCapsule::new();
        writer.set_max_density(2);
        let positions = vec![(0, 0.0), (10, 10.0), (20, 20.0), (30, 30.0)];
        let (actions, validation) = writer.from_positions(&positions);
        // With max 2 actions/sec and 10ms spacing, we expect thinning
        assert!(actions.len() < positions.len());
        assert!(validation.density_ok);
    }

    #[test]
    fn validation_reports_monotonic() {
        let writer = FunscriptCapsule::new();
        let mut positions = Vec::new();
        for i in 0..40 {
            positions.push((i * 25, (i * 3 % 100) as f32));
        }
        let (actions, validation) = writer.from_positions(&positions);
        assert!(validation.monotonic);
        assert!(validation.clamped);
        assert!(actions.windows(2).all(|w| w[1].at > w[0].at));
    }

    #[test]
    fn stretch_variations_remain_monotone_and_within_density() {
        use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};
        use crate::sampler::sample_motion_block;
        use crate::timeline::TimelineEntry;

        let base_block =
            MotionBlockCapsule::new(99, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let stretch_ppms = [500_000u32, 1_000_000u32, 1_500_000u32];
        let writer = FunscriptCapsule::new();
        for ppm in stretch_ppms {
            let entry = TimelineEntry::new(0, 800, base_block.clone()).with_stretch_ppm(ppm);
            let samples = sample_motion_block(&entry, 120);
            let positions: Vec<(u64, f32)> =
                samples.iter().map(|s| (s.time_ms, s.position_pct)).collect();
            let (actions, validation) = writer.from_positions(&positions);
            assert!(
                validation.monotonic,
                "monotone failed at stretch {}",
                ppm
            );
            assert!(validation.clamped);
            let min_delta = 1000u64
                .saturating_div(writer.max_density_per_sec as u64)
                .max(1);
            assert!(actions.windows(2).all(|w| w[1].at.saturating_sub(w[0].at) >= min_delta));
        }
    }
}

fn enforce_density(mut actions: Vec<FunscriptAction>, max_per_sec: u32) -> Vec<FunscriptAction> {
    if actions.is_empty() || max_per_sec == 0 {
        return actions;
    }
    let mut filtered = Vec::with_capacity(actions.len());
    let mut last_at = None;
    let min_delta = 1000u64.saturating_div(max_per_sec as u64).max(1);
    for action in actions.drain(..) {
        if let Some(prev) = last_at {
            if action.at.saturating_sub(prev) < min_delta {
                continue;
            }
        }
        last_at = Some(action.at);
        filtered.push(action);
    }
    filtered
}

fn validate_actions(actions: &[FunscriptAction], max_per_sec: u32) -> FunscriptValidation {
    let monotonic = actions
        .windows(2)
        .all(|w| w[1].at > w[0].at);
    let clamped = actions.iter().all(|a| a.pos <= 100);
    let min_delta = 1000u64.saturating_div(max_per_sec as u64).max(1);
    let density_ok = actions
        .windows(2)
        .all(|w| w[1].at.saturating_sub(w[0].at) >= min_delta);
    let duration_ms = actions.last().map(|a| a.at).unwrap_or(0);
    FunscriptValidation {
        monotonic,
        clamped,
        density_ok,
        total_actions: actions.len(),
        duration_ms,
    }
}
