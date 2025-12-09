use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audio::AudioGrain;
use crate::funscript::{FunscriptAction, FunscriptCapsule, FunscriptValidation};
use crate::inflection::{InflectionEvent, InflectionKind};
use crate::overlay::HudFrame;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FunscriptRender {
    pub json: String,
    pub validation: FunscriptValidation,
    pub actions: Vec<FunscriptAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPlan {
    pub video_frames: usize,
    pub audio_grains: usize,
    pub profile: ExportProfile,
    pub output_path: Option<PathBuf>,
    pub report: ExportReport,
    pub bitrate_kbps: u32,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub sample_rate_hz: u32,
    pub audio_channels: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportProfile {
    Simple,  // mp4 + hud + funscript defaults
    Advanced { bitrate_kbps: u32, fps: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub duration_ms: u64,
    pub impact_high: usize,
    pub impact_low: usize,
    pub buzz: usize,
    pub funscript: FunscriptValidation,
    pub output_path: Option<PathBuf>,
    pub profile: ExportProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReportValidation {
    pub ok: bool,
    pub monotonic: bool,
    pub clamped: bool,
    pub density_ok: bool,
    pub impacts_total: usize,
    pub buzz: usize,
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct ExportCapsule {
    generation: AtomicU64,
    funscript: FunscriptCapsule,
}

#[derive(Debug, Clone, Default)]
pub struct ExportUiOptions {
    pub bitrate_kbps: Option<u32>,
    pub fps: Option<u32>,
    pub output_path: Option<PathBuf>,
}

impl ExportUiOptions {
    pub fn to_profile(&self, default_fps: u32) -> (ExportProfile, Option<PathBuf>) {
        if self.bitrate_kbps.is_some() || self.fps.is_some() || self.output_path.is_some() {
            let profile = ExportProfile::Advanced {
                bitrate_kbps: self.bitrate_kbps.unwrap_or(4000),
                fps: self.fps.unwrap_or(default_fps),
            };
            (profile, self.output_path.clone())
        } else {
            (ExportProfile::Simple, None)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportValidation {
    pub ok: bool,
    pub bitrate_ok: bool,
    pub fps_ok: bool,
    pub path_ok: bool,
}

impl Default for ExportCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_options_map_to_profile() {
        let opts = ExportUiOptions {
            bitrate_kbps: Some(2500),
            fps: Some(48),
            output_path: Some(PathBuf::from("/tmp/out.funscript")),
        };
        let (profile, path) = opts.to_profile(60);
        match profile {
            ExportProfile::Advanced { bitrate_kbps, fps } => {
                assert_eq!(bitrate_kbps, 2500);
                assert_eq!(fps, 48);
            }
            _ => panic!("expected advanced"),
        }
        assert!(path.is_some());

        let opts_simple = ExportUiOptions::default();
        let (profile, path) = opts_simple.to_profile(60);
        assert!(matches!(profile, ExportProfile::Simple));
        assert!(path.is_none());
    }

    #[test]
    fn validates_ui_options() {
        let caps = ExportCapsule::new();
        let good = ExportUiOptions {
            bitrate_kbps: Some(4000),
            fps: Some(60),
            output_path: None,
        };
        let valid = caps.validate_ui_options(&good, 60);
        assert!(valid.ok);

        let bad = ExportUiOptions {
            bitrate_kbps: Some(10),
            fps: Some(10),
            output_path: Some(PathBuf::from("")),
        };
        let invalid = caps.validate_ui_options(&bad, 60);
        assert!(!invalid.ok);
        assert!(!invalid.bitrate_ok);
        assert!(!invalid.fps_ok);
        assert!(!invalid.path_ok);
    }
}

impl ExportCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            funscript: FunscriptCapsule::new(),
        }
    }

    pub fn render_funscript(&self, positions: &[(u64, f32)]) -> FunscriptRender {
        let (actions, validation) = self.funscript.from_positions(positions);
        self.generation.fetch_add(1, Ordering::Relaxed);
        FunscriptRender {
            json: self.funscript.to_json(&actions),
            validation,
            actions,
        }
    }

    /// Basic UI validation for export knobs to prevent obviously invalid requests.
    pub fn validate_ui_options(&self, opts: &ExportUiOptions, default_fps: u32) -> ExportValidation {
        let bitrate = opts.bitrate_kbps.unwrap_or(4000);
        let fps = opts.fps.unwrap_or(default_fps);
        let path_ok = opts.output_path.as_ref().map(|p| !p.as_os_str().is_empty()).unwrap_or(true);
        let bitrate_ok = (500..=50_000).contains(&bitrate);
        let fps_ok = (24..=240).contains(&fps);
        let ok = bitrate_ok && fps_ok && path_ok;
        ExportValidation {
            ok,
            bitrate_ok,
            fps_ok,
            path_ok,
        }
    }

    /// Prepare an internal render plan (overlay + audio) for the in-tree media pipeline.
    /// This deliberately avoids external ffmpeg; the actual mux/render should call the
    /// internal media backend.
    pub fn plan_internal_render(
        &self,
        hud_frames: &[HudFrame],
        grains: &[AudioGrain],
        events: &[InflectionEvent],
        validation: FunscriptValidation,
        profile: ExportProfile,
        output_path: Option<PathBuf>,
        default_fps: u32,
    ) -> RenderPlan {
        let mut impact_high = 0;
        let mut impact_low = 0;
        let mut buzz = 0;
        for e in events {
            match e.kind {
                InflectionKind::ImpactHigh => impact_high += 1,
                InflectionKind::ImpactLow => impact_low += 1,
                InflectionKind::Buzz => buzz += 1,
            }
        }
        let duration_ms = hud_frames.last().map(|f| f.time_ms).unwrap_or(0);
        let (bitrate_kbps, fps) = match profile {
            ExportProfile::Simple => (4000, default_fps),
            ExportProfile::Advanced { bitrate_kbps, fps } => (bitrate_kbps, fps),
        };
        let (width, height) = match profile {
            ExportProfile::Simple => (960, 540),
            ExportProfile::Advanced { .. } => (1280, 720),
        };
        let (sample_rate_hz, audio_channels) = (48_000, 1);
        self.generation.fetch_add(1, Ordering::Relaxed);
        RenderPlan {
            video_frames: hud_frames.len(),
            audio_grains: grains.len(),
            profile: profile.clone(),
            output_path: output_path.clone(),
            bitrate_kbps,
            fps,
            width,
            height,
            sample_rate_hz,
            audio_channels,
            report: ExportReport {
                duration_ms,
                impact_high,
                impact_low,
                buzz,
                funscript: validation,
                output_path,
                profile,
            },
        }
    }

    /// Lightweight validation helper for UI: parse report JSON + optional funscript JSON.
    pub fn validate_export_report(
        &self,
        report_json: &str,
        funscript_json: Option<&str>,
    ) -> ExportReportValidation {
        // Naive JSON parsing (no external deps): look for booleans/integers via substring search.
        let monotonic = report_json.contains("\"monotonic\":true");
        let clamped = report_json.contains("\"clamped\":true");
        let density_ok = report_json.contains("\"density_ok\":true");
        let impact_high = extract_int(report_json, "\"impact_high\":").unwrap_or(0);
        let impact_low = extract_int(report_json, "\"impact_low\":").unwrap_or(0);
        let buzz = extract_int(report_json, "\"buzz\":").unwrap_or(0);
        // Optional cross-check: ensure funscript has actions array if provided.
        let funscript_present = funscript_json.map(|s| s.contains("\"actions\"")).unwrap_or(true);
        let ok = monotonic && clamped && density_ok && funscript_present;
        ExportReportValidation {
            ok,
            monotonic,
            clamped,
            density_ok,
            impacts_total: impact_high.saturating_add(impact_low),
            buzz,
        }
    }
}

fn extract_int(input: &str, key: &str) -> Option<usize> {
    input.find(key).and_then(|idx| {
        let rest = &input[idx + key.len()..];
        let digits: String = rest
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse::<usize>().ok()
    })
}
