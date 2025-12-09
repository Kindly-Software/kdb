use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::AtomicU64;

use crate::inflection::{InflectionEvent, InflectionKind};
use crate::sampler::MotionSample;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudZone {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct HudFrame {
    pub time_ms: u64,
    pub position_pct: f32,
    pub zone: HudZone,
    pub jitter: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ImpactIndicator {
    pub time_ms: u64,
    pub kind: InflectionKind,
    pub zone: HudZone,
    pub is_ghost: bool,
    pub color: (u8, u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTheme {
    Light,
    Dark,
    Neon,
}

#[derive(Debug, Clone, Copy)]
pub struct HudStyle {
    pub theme: HudTheme,
    pub thickness: f32,
    pub color: (u8, u8, u8, u8), // RGBA
    pub jitter_on_vibration: bool,
    pub ghost_next_impact: bool,
    pub latency_indicator_ms: u32,
    pub vibration_color: (u8, u8, u8, u8),
    pub lead_lag_ms: i32,
}

#[derive(Debug, Clone)]
pub struct HudOverlay {
    pub frames: Vec<HudFrame>,
    pub indicators: Vec<ImpactIndicator>,
    pub ghost_indicator: Option<ImpactIndicator>,
    pub latency_indicator_ms: u32,
    pub lead_lag_ms: i32,
    pub theme: HudTheme,
    pub thickness: f32,
    pub color: (u8, u8, u8, u8),
    pub vibration_color: (u8, u8, u8, u8),
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct OverlayCapsule {
    generation: AtomicU64,
    style: HudStyle,
}

impl OverlayCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            style: HudStyle {
                theme: HudTheme::Dark,
                thickness: 2.0,
                color: (255, 255, 255, 200),
                jitter_on_vibration: true,
                ghost_next_impact: true,
                latency_indicator_ms: 0,
                vibration_color: (255, 80, 80, 200),
                lead_lag_ms: 0,
            },
        }
    }

    pub fn generate_hud(
        &self,
        samples: &[MotionSample],
        events: &[InflectionEvent],
        range: (u8, u8),
        cursor_ms: u64,
    ) -> HudOverlay {
        let zone = zone_for_range(range);
        let buzz_windows: Vec<u64> = events
            .iter()
            .filter(|e| e.kind == InflectionKind::Buzz)
            .map(|e| e.time_ms)
            .collect();
        let frames = samples
            .iter()
            .map(|s| HudFrame {
                time_ms: s.time_ms,
                position_pct: s.position_pct,
                zone,
                jitter: self.style.jitter_on_vibration && is_near_buzz(s.time_ms, &buzz_windows),
            })
            .collect();
        let indicators = events
            .iter()
            .map(|e| ImpactIndicator {
                time_ms: e.time_ms,
                kind: e.kind,
                zone,
                is_ghost: false,
                color: match e.kind {
                    InflectionKind::Buzz => self.style.vibration_color,
                    _ => self.style.color,
                },
            })
            .collect();
        let ghost_indicator = if self.style.ghost_next_impact {
            next_event_after(cursor_ms, events).map(|e| ImpactIndicator {
                time_ms: e.time_ms,
                kind: e.kind,
                zone,
                is_ghost: true,
                color: match e.kind {
                    InflectionKind::Buzz => self.style.vibration_color,
                    _ => self.style.color,
                },
            })
        } else {
            None
        };

        HudOverlay {
            frames,
            indicators,
            ghost_indicator,
            latency_indicator_ms: self.style.latency_indicator_ms,
            lead_lag_ms: self.style.lead_lag_ms,
            theme: self.style.theme,
            thickness: self.style.thickness,
            color: self.style.color,
            vibration_color: self.style.vibration_color,
        }
    }

    pub fn style(&self) -> HudStyle {
        self.style
    }

    pub fn set_style(&mut self, style: HudStyle) {
        self.style = style;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn apply_theme(&mut self, theme: HudTheme) {
        self.style.theme = theme;
        let (color, vib) = match theme {
            HudTheme::Light => ((30, 30, 30, 220), (200, 60, 60, 220)),
            HudTheme::Dark => ((255, 255, 255, 200), (255, 120, 120, 220)),
            HudTheme::Neon => ((50, 220, 255, 220), (255, 50, 200, 220)),
        };
        self.style.color = color;
        self.style.vibration_color = vib;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn configure_visuals(
        &mut self,
        thickness: f32,
        color: (u8, u8, u8, u8),
        vibration_color: (u8, u8, u8, u8),
        latency_ms: u32,
        lead_lag_ms: i32,
    ) {
        self.style.thickness = thickness.clamp(0.5, 20.0);
        self.style.color = color;
        self.style.vibration_color = vibration_color;
        self.style.latency_indicator_ms = latency_ms;
        self.style.lead_lag_ms = lead_lag_ms;
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for OverlayCapsule {
    fn default() -> Self {
        Self::new()
    }
}

fn zone_for_range(range: (u8, u8)) -> HudZone {
    let mid = (range.0 as u16 + range.1 as u16) / 2;
    if mid >= 50 { HudZone::Top } else { HudZone::Bottom }
}

fn is_near_buzz(time_ms: u64, buzzes: &[u64]) -> bool {
    buzzes
        .iter()
        .any(|t| time_ms.abs_diff(*t) <= 30)
}

fn next_event_after(cursor_ms: u64, events: &[InflectionEvent]) -> Option<&InflectionEvent> {
    events
        .iter()
        .filter(|e| e.time_ms >= cursor_ms)
        .min_by_key(|e| e.time_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflection::{detect_inflections, InflectionKind};
    use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};
    use crate::sampler::sample_motion_block;
    use crate::timeline::TimelineEntry;

    #[test]
    fn overlay_matches_samples_and_events() {
        let block =
            MotionBlockCapsule::new(20, MotionPattern::Linear, 75, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block.clone());
        let samples = sample_motion_block(&entry, 60);
        let events = detect_inflections(&samples, 100.0);

        let overlay = OverlayCapsule::new();
        let hud = overlay.generate_hud(&samples, &events, block.range(), entry.start_ms);

        assert_eq!(hud.frames.len(), samples.len());
        assert_eq!(hud.indicators.len(), events.len());
        assert!(hud
            .indicators
            .iter()
            .any(|i| i.kind == InflectionKind::ImpactHigh));
        assert!(hud.latency_indicator_ms <= overlay.style().latency_indicator_ms);
        assert!(hud.ghost_indicator.is_some());
        assert_eq!(hud.lead_lag_ms, overlay.style().lead_lag_ms);
    }
}
