use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::AtomicU64;

use crate::inflection::{InflectionEvent, InflectionKind, InflectionTuning, detect_inflections_tuned};
use crate::presets::PresetMeta;

#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    pub attack_ms: u32,
    pub decay_ms: u32,
    pub gain_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioGrainKind {
    ImpactHigh,
    ImpactLow,
    Buzz,
    PreviewClick,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioGrain {
    pub time_ms: u64,
    pub kind: AudioGrainKind,
    pub gain_db: f32,
    pub eq_tilt_db: f32,
    pub preview: bool,
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct AudioCapsule {
    generation: AtomicU64,
    impact_env: Envelope,
    buzz_env: Envelope,
    max_density_per_sec: u32,
    preview_gain_db: f32,
    preview_eq_tilt_db: f32,
    live_gain_override_db: Option<f32>,
    live_eq_override_db: Option<f32>,
    sample_rate_hz: u32,
}

impl AudioCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            impact_env: Envelope {
                attack_ms: 2,
                decay_ms: 80,
                gain_db: 0.0,
            },
            buzz_env: Envelope {
                attack_ms: 0,
                decay_ms: 20,
                gain_db: -6.0,
            },
            max_density_per_sec: 120,
            preview_gain_db: -6.0,
            preview_eq_tilt_db: 0.0,
            live_gain_override_db: None,
            live_eq_override_db: None,
            sample_rate_hz: 48_000,
        }
    }

    pub fn set_envelopes(&mut self, impact: Envelope, buzz: Envelope) {
        self.impact_env = impact;
        self.buzz_env = buzz;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_max_density(&mut self, max_per_sec: u32) {
        self.max_density_per_sec = max_per_sec.max(1);
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_preview_tone(&mut self, gain_db: f32, eq_tilt_db: f32) {
        self.preview_gain_db = gain_db;
        self.preview_eq_tilt_db = eq_tilt_db;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_live_overrides(&mut self, gain_db: Option<f32>, eq_tilt_db: Option<f32>) {
        self.live_gain_override_db = gain_db;
        self.live_eq_override_db = eq_tilt_db;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Synthesize a simple mono PCM16 waveform from grains using attack/decay envelopes.
    /// This is intentionally minimal (clicks/short buzz tones) to keep CPU usage low.
    pub fn synthesize_pcm(&self, grains: &[AudioGrain], duration_ms: u64, sample_rate: u32) -> Vec<u8> {
        let sr = sample_rate.max(8_000).min(96_000);
        let total_samples = ((duration_ms as f64 / 1000.0) * sr as f64).ceil() as usize + sr as usize; // extra tail
        let mut pcm = vec![0i32; total_samples.max(1)];

        for grain in grains {
            let env = self.envelope_for(grain.kind);
            let attack_samples = ms_to_samples(env.attack_ms, sr);
            let decay_samples = ms_to_samples(env.decay_ms, sr);
            let peak = db_to_lin(grain.gain_db) * (if grain.kind == AudioGrainKind::ImpactLow { -1.0 } else { 1.0 });
            let start = ms_to_samples(grain.time_ms as u32, sr);
            let end = (start + attack_samples + decay_samples).min(pcm.len().saturating_sub(1));

            // Attack
            for i in 0..attack_samples {
                let idx = start + i;
                if idx >= pcm.len() {
                    break;
                }
                let t = i as f32 / attack_samples.max(1) as f32;
                pcm[idx] = (pcm[idx] as f32 + peak * t * i16::MAX as f32) as i32;
            }
            // Decay
            for i in 0..decay_samples {
                let idx = start + attack_samples + i;
                if idx >= pcm.len() || idx >= end {
                    break;
                }
                let t = 1.0 - (i as f32 / decay_samples.max(1) as f32);
                pcm[idx] = (pcm[idx] as f32 + peak * t * i16::MAX as f32) as i32;
            }
        }

        // Clamp to i16
        let mut out = Vec::with_capacity(pcm.len() * 2);
        for s in pcm {
            let clamped = s.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            out.extend_from_slice(&clamped.to_le_bytes());
        }
        out
    }

    pub fn grains_from_inflections(
        &self,
        events: &[InflectionEvent],
        preset: Option<&PresetMeta>,
    ) -> Vec<AudioGrain> {
        let (preset_gain, eq_tilt) = preset
            .map(|m| (m.audio_gain_db, m.eq_tilt_db))
            .unwrap_or((0.0, 0.0));
        let gain_override = self.live_gain_override_db.unwrap_or(0.0);
        let eq_override = self.live_eq_override_db.unwrap_or(0.0);
        let mut grains = Vec::new();
        let mut last_ms = None;
        for e in events {
            if let Some(prev) = last_ms {
                let delta = e.time_ms.saturating_sub(prev);
                if delta > 0 {
                    let density = 1000 / delta.max(1);
                    if density as u32 > self.max_density_per_sec {
                        continue;
                    }
                }
            }
            let kind = match e.kind {
                InflectionKind::ImpactHigh => AudioGrainKind::ImpactHigh,
                InflectionKind::ImpactLow => AudioGrainKind::ImpactLow,
                InflectionKind::Buzz => AudioGrainKind::Buzz,
            };
            grains.push(AudioGrain {
                time_ms: e.time_ms,
                kind,
                gain_db: preset_gain
                    + match kind {
                        AudioGrainKind::Buzz => self.buzz_env.gain_db,
                        _ => self.impact_env.gain_db,
                    }
                    + gain_override,
                eq_tilt_db: eq_tilt + eq_override,
                preview: false,
            });
            last_ms = Some(e.time_ms);
        }
        grains
    }

    pub fn envelope_for(&self, kind: AudioGrainKind) -> Envelope {
        match kind {
            AudioGrainKind::ImpactHigh | AudioGrainKind::ImpactLow => self.impact_env,
            AudioGrainKind::Buzz => self.buzz_env,
            AudioGrainKind::PreviewClick => self.impact_env,
        }
    }

    pub fn preview_grains_for_hover(
        &self,
        entry: &crate::timeline::TimelineEntry,
        fps: u32,
        tuning: &InflectionTuning,
    ) -> Vec<AudioGrain> {
        use crate::sampler::sample_motion_block;

        let samples = sample_motion_block(entry, fps.max(24));
        let threshold = tuning.threshold_for(entry.block.range(), entry.block.tempo());
        let events = detect_inflections_tuned(&samples, tuning, entry.block.range(), entry.block.tempo());
        let mut grains = self.grains_from_inflections(&events, entry.preset_meta.as_ref());
        if let Some(first) = events.first() {
            grains.push(AudioGrain {
                time_ms: first.time_ms,
                kind: AudioGrainKind::PreviewClick,
                gain_db: self.preview_gain_db,
                eq_tilt_db: self.preview_eq_tilt_db,
                preview: true,
            });
        } else {
            grains.push(AudioGrain {
                time_ms: threshold as u64,
                kind: AudioGrainKind::PreviewClick,
                gain_db: self.preview_gain_db,
                eq_tilt_db: self.preview_eq_tilt_db,
                preview: true,
            });
        }
        grains
    }
}

impl Default for AudioCapsule {
    fn default() -> Self {
        Self::new()
    }
}

fn ms_to_samples(ms: u32, sample_rate: u32) -> usize {
    (((ms as f64 / 1000.0) * sample_rate as f64).ceil() as usize).max(1)
}

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflection::InflectionKind;

    #[test]
    fn builds_grains() {
        let audio = AudioCapsule::new();
        let events = &[
            InflectionEvent {
                time_ms: 10,
                position_pct: 100.0,
                kind: InflectionKind::ImpactHigh,
            },
            InflectionEvent {
                time_ms: 30,
                position_pct: 0.0,
                kind: InflectionKind::ImpactLow,
            },
            InflectionEvent {
                time_ms: 40,
                position_pct: 50.0,
                kind: InflectionKind::Buzz,
            },
        ];

        let grains = audio.grains_from_inflections(events, None);
        assert_eq!(grains.len(), 3);
        assert_eq!(grains[0].kind, AudioGrainKind::ImpactHigh);
        assert_eq!(grains[2].kind, AudioGrainKind::Buzz);
        assert!(grains.iter().all(|g| !g.preview));
    }

    #[test]
    fn synthesizes_pcm() {
        let audio = AudioCapsule::new();
        let events = &[
            InflectionEvent {
                time_ms: 0,
                position_pct: 100.0,
                kind: InflectionKind::ImpactHigh,
            },
            InflectionEvent {
                time_ms: 30,
                position_pct: 0.0,
                kind: InflectionKind::ImpactLow,
            },
        ];
        let grains = audio.grains_from_inflections(events, None);
        let pcm = audio.synthesize_pcm(&grains, 100, 48_000);
        assert!(!pcm.is_empty());
        assert!(pcm.len() > 100); // some samples
    }

    #[test]
    fn preview_uses_tuning() {
        let audio = AudioCapsule::new();
        let block =
            crate::motion::MotionBlockCapsule::new(9, crate::motion::MotionPattern::Vibration, 60, 90, crate::motion::MotionTempo::Rapide, 400);
        let entry = crate::timeline::TimelineEntry::new(0, 400, block);
        let tuning = crate::inflection::InflectionTuning::default();
        let grains = audio.preview_grains_for_hover(&entry, 120, &tuning);
        assert!(!grains.is_empty());
        assert!(grains.iter().any(|g| g.preview));
    }
}
