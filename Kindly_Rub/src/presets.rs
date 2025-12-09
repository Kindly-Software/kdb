use atomic_capsule_derive::ComputationalCapsule;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetTag {
    Full,
    Top,
    Bottom,
    Vibration,
}

#[derive(Debug, Clone)]
pub struct PresetMeta {
    pub tag: PresetTag,
    pub generation: u64,
    pub sparkline: Vec<u8>, // simple preview 0-100 points
    pub audio_gain_db: f32,
    pub eq_tilt_db: f32,
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct PresetLibraryCapsule {
    generation: AtomicU64,
    presets: HashMap<String, MotionBlockCapsule>,
    meta: HashMap<String, PresetMeta>,
}

impl PresetLibraryCapsule {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            presets: HashMap::new(),
            meta: HashMap::new(),
        }
    }

    pub fn upsert(
        &mut self,
        name: impl Into<String>,
        pattern: MotionPattern,
        range_start: u8,
        range_end: u8,
        tempo: MotionTempo,
        nominal_duration_ms: u32,
        meta: PresetMeta,
    ) {
        let name = name.into();
        let id = self.presets.len() as u64 + 1;
        let block =
            MotionBlockCapsule::new(id, pattern, range_start, range_end, tempo, nominal_duration_ms);
        self.presets.insert(name.clone(), block);
        self.meta.insert(name, meta);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn upsert_autofill(
        &mut self,
        name: impl Into<String>,
        pattern: MotionPattern,
        range_start: u8,
        range_end: u8,
        tempo: MotionTempo,
        nominal_duration_ms: u32,
    ) {
        let tag = infer_tag(range_start, range_end, pattern);
        let sparkline = sparkline(range_start, range_end, pattern, 24);
        let generation = self.generation() + 1;
        let meta = PresetMeta {
            tag,
            generation,
            sparkline,
            audio_gain_db: 0.0,
            eq_tilt_db: 0.0,
        };
        self.upsert(
            name,
            pattern,
            range_start,
            range_end,
            tempo,
            nominal_duration_ms,
            meta,
        );
    }

    pub fn get(&self, name: &str) -> Option<MotionBlockCapsule> {
        self.presets.get(name).cloned()
    }

    pub fn meta(&self, name: &str) -> Option<&PresetMeta> {
        self.meta.get(name)
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn describe(&self, name: &str) -> Option<String> {
        let meta = self.meta(name)?;
        let tag = match meta.tag {
            PresetTag::Full => "full",
            PresetTag::Top => "top",
            PresetTag::Bottom => "bottom",
            PresetTag::Vibration => "vibration",
        };
        Some(format!("{} (g{}) [{}]", name, meta.generation, tag))
    }
}

impl Default for PresetLibraryCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut lib = PresetLibraryCapsule::new();
        lib.upsert(
            "full_fast",
            MotionPattern::Linear,
            0,
            100,
            MotionTempo::Rapide,
            800,
            PresetMeta {
                tag: PresetTag::Full,
                generation: 1,
                sparkline: vec![0, 50, 100],
                audio_gain_db: 0.0,
                eq_tilt_db: 0.0,
            },
        );
        assert!(lib.get("full_fast").is_some());
        let meta = lib.meta("full_fast").unwrap();
        assert_eq!(meta.tag, PresetTag::Full);
        assert_eq!(lib.generation(), 1);
    }

    #[test]
    fn auto_generates_sparkline_and_tag() {
        let mut lib = PresetLibraryCapsule::new();
        lib.upsert_autofill(
            "vibe",
            MotionPattern::Vibration,
            70,
            100,
            MotionTempo::Rapide,
            600,
        );
        let meta = lib.meta("vibe").unwrap();
        assert_eq!(meta.tag, PresetTag::Vibration);
        assert!(!meta.sparkline.is_empty());
        assert_eq!(meta.generation, 1);
        let desc = lib.describe("vibe").unwrap();
        assert!(desc.contains("g1"));
    }
}

fn infer_tag(range_start: u8, range_end: u8, pattern: MotionPattern) -> PresetTag {
    if pattern == MotionPattern::Vibration {
        PresetTag::Vibration
    } else if range_start <= 5 && range_end >= 95 {
        PresetTag::Full
    } else if range_end <= 50 {
        PresetTag::Bottom
    } else {
        PresetTag::Top
    }
}

fn sparkline(range_start: u8, range_end: u8, pattern: MotionPattern, points: usize) -> Vec<u8> {
    if points == 0 {
        return Vec::new();
    }
    let steps = points.max(2) as f32 - 1.0;
    let amplitude = range_end as f32 - range_start as f32;
    let cycles = if pattern == MotionPattern::Vibration { 2.0 } else { 1.0 };
    (0..points)
        .map(|i| {
            let t = i as f32 / steps;
            let phase = (t * cycles) % 1.0;
            let (dir, local_t) = if phase < 0.5 {
                (1.0, phase * 2.0)
            } else {
                (-1.0, (phase - 0.5) * 2.0)
            };
            let p = 3.0 * local_t * local_t - 2.0 * local_t * local_t * local_t;
            let pos = if dir > 0.0 {
                range_start as f32 + amplitude * p
            } else {
                range_end as f32 - amplitude * p
            };
            pos.round().clamp(0.0, 100.0) as u8
        })
        .collect()
}
