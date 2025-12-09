use atomic_capsule_derive::ComputationalCapsule;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audio::{AudioCapsule, AudioGrain};
use crate::export::{ExportReport, RenderPlan};
use crate::overlay::HudOverlay;
use crate::rasterizer::{HudRasterizerCapsule, RasterizedFrame};

#[derive(Debug, Clone)]
pub struct MediaBackendRequest {
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub target_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub sample_rate_hz: u32,
    pub audio_channels: u8,
}

#[derive(Debug)]
pub struct RenderOutput {
    pub video_frames: usize,
    pub audio_grains: usize,
    pub funscript_bytes: usize,
    pub funscript_path: Option<PathBuf>,
    pub report_path: PathBuf,
    pub report_bytes: usize,
    pub report: ExportReport,
    pub backend_request: MediaBackendRequest,
    pub media_output: Option<MediaMuxResult>,
    pub audio_bytes: usize,
}

#[derive(Debug)]
pub struct MediaMuxResult {
    pub output_path: PathBuf,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub video_frames: usize,
    pub audio_frames: usize,
}

/// Internal FFmpeg-alternative media muxer. This is a stub replacement for the
/// proprietary in-tree implementation and must not call external binaries.
/// It writes a minimal WebM header via atomic_capsule muxer; if that fails,
/// it falls back to a manifest of HUD/audio counts to disk to prove invocation.
fn internal_media_mux(
    request: &MediaBackendRequest,
    rasters: &[RasterizedFrame],
    grains: &[AudioGrain],
    pcm: &[u8],
) -> io::Result<MediaMuxResult> {
    let adapter = crate::media_adapter::WebmMuxAdapterCapsule::new();
    adapter.mux(request, rasters, grains, pcm)
}

fn escape_json_str(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_report_json(report: &ExportReport) -> String {
    let profile = match &report.profile {
        crate::export::ExportProfile::Simple => "simple".to_string(),
        crate::export::ExportProfile::Advanced { bitrate_kbps, fps } => {
            format!("advanced:{}kbps@{}fps", bitrate_kbps, fps)
        }
    };
    let output_path = report
        .output_path
        .as_ref()
        .map(|p| format!("\"{}\"", escape_json_str(&p.to_string_lossy())))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"duration_ms\":{},\"impact_high\":{},\"impact_low\":{},\"buzz\":{},\
\"funscript\":{{\"monotonic\":{},\"clamped\":{},\"density_ok\":{},\"total_actions\":{},\"duration_ms\":{}}},\
\"profile\":\"{}\",\"output_path\":{}}}",
        report.duration_ms,
        report.impact_high,
        report.impact_low,
        report.buzz,
        report.funscript.monotonic,
        report.funscript.clamped,
        report.funscript.density_ok,
        report.funscript.total_actions,
        report.funscript.duration_ms,
        profile,
        output_path
    )
}

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct MediaRenderCapsule {
    generation: AtomicU64,
    /// Default in-tree backend function (no external ffmpeg).
    backend:
        fn(
            &MediaBackendRequest,
            &[RasterizedFrame],
            &[AudioGrain],
            &[u8],
        ) -> io::Result<MediaMuxResult>,
    custom_mux: Option<
        Box<
            dyn Fn(
                    &MediaBackendRequest,
                    &[RasterizedFrame],
                    &[AudioGrain],
                    &[u8],
                ) -> io::Result<MediaMuxResult>
                + Send
                + Sync,
        >,
    >,
    rasterizer: HudRasterizerCapsule,
    audio: AudioCapsule,
}

impl std::fmt::Debug for MediaRenderCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaRenderCapsule")
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

impl Default for MediaRenderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaRenderCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            backend: internal_media_mux,
            custom_mux: None,
            rasterizer: HudRasterizerCapsule::new(),
            audio: AudioCapsule::new(),
        }
    }

    /// Force using the built-in internal backend even when a handler was previously set.
    pub fn use_internal_backend(&mut self) {
        self.custom_mux = None;
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Allow callers to inject the real in-tree media muxer. This remains pure Rust
    /// and must not call external ffmpeg.
    pub fn set_mux_handler<F>(&mut self, handler: F)
    where
        F: Fn(
                &MediaBackendRequest,
                &[RasterizedFrame],
                &[AudioGrain],
                &[u8],
            ) -> io::Result<MediaMuxResult>
            + Send
            + Sync
            + 'static,
    {
        self.custom_mux = Some(Box::new(handler));
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Register the built-in in-tree muxer as a handler (useful for bootstrap paths
    /// that prefer the handler route).
    pub fn register_internal_mux_handler(&mut self) {
        self.set_mux_handler(move |req, rasters, grains, pcm| {
            internal_media_mux(req, rasters, grains, pcm)
        });
    }

    /// Render using the internal pipeline (placeholder): writes funscript JSON to disk
    /// and returns counts for HUD frames and audio grains. Video/audio muxing is
    /// expected to use the in-tree media backend (not external ffmpeg).
    pub fn render(
        &self,
        render_plan: &RenderPlan,
        hud: &HudOverlay,
        grains: &[AudioGrain],
        funscript_json: &str,
        output_dir: &Path,
    ) -> io::Result<RenderOutput> {
        let validation = &render_plan.report.funscript;
        if !(validation.monotonic && validation.clamped && validation.density_ok) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "funscript validation failed",
            ));
        }
        let funscript_path = if let Some(path) = render_plan.output_path.clone() {
            path
        } else {
            let mut path = output_dir.to_path_buf();
            path.push("script.funscript");
            path
        };
        let mut file = File::create(&funscript_path)?;
        file.write_all(funscript_json.as_bytes())?;
        file.sync_all()?;
        let mut report_path = output_dir.to_path_buf();
        report_path.push("export_report.json");
        let report_json = render_report_json(&render_plan.report);
        fs::write(&report_path, report_json.as_bytes())?;

        // Rasterize HUD into RGBA frames (CPU).
        let rasters = self
            .rasterizer
            .render(hud, render_plan.width, render_plan.height);
        let pcm = self
            .audio
            .synthesize_pcm(grains, render_plan.report.duration_ms, render_plan.sample_rate_hz);

        // Prepare internal media backend request (muxing to happen in in-tree pipeline).
        let mut media_path = output_dir.to_path_buf();
        let output_name = match render_plan.profile {
            crate::export::ExportProfile::Advanced { .. } => "output_advanced.mp4",
            crate::export::ExportProfile::Simple => "output.mp4",
        };
        media_path.push(output_name);
        let backend_request = MediaBackendRequest {
            fps: render_plan.fps,
            bitrate_kbps: render_plan.bitrate_kbps,
            target_path: media_path,
            width: render_plan.width,
            height: render_plan.height,
            sample_rate_hz: render_plan.sample_rate_hz,
            audio_channels: render_plan.audio_channels,
        };

        let media_output = if let Some(handler) = &self.custom_mux {
            handler(&backend_request, &rasters, grains, &pcm).ok()
        } else {
            (self.backend)(&backend_request, &rasters, grains, &pcm).ok()
        };

        self.generation.fetch_add(1, Ordering::Relaxed);

        Ok(RenderOutput {
            video_frames: render_plan.video_frames.min(hud.frames.len()),
            audio_grains: render_plan.audio_grains.min(grains.len()),
            funscript_bytes: funscript_json.len(),
            funscript_path: Some(funscript_path),
            report_path,
            report_bytes: report_json.len(),
            report: render_plan.report.clone(),
            backend_request,
            media_output,
            audio_bytes: pcm.len(),
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::{HudFrame, HudOverlay, HudTheme, HudZone, ImpactIndicator};
    use crate::audio::{AudioGrain, AudioGrainKind};
    use crate::export::{ExportProfile, ExportReport, RenderPlan};
    use crate::funscript::FunscriptValidation;

    fn sample_hud() -> HudOverlay {
        let frames = vec![HudFrame {
            time_ms: 0,
            position_pct: 50.0,
            zone: HudZone::Top,
            jitter: false,
        }];
        HudOverlay {
            frames,
            indicators: vec![ImpactIndicator {
                time_ms: 0,
                kind: crate::inflection::InflectionKind::ImpactHigh,
                zone: HudZone::Top,
                is_ghost: false,
                color: (255, 255, 255, 200),
            }],
            ghost_indicator: None,
            latency_indicator_ms: 0,
            lead_lag_ms: 0,
            theme: HudTheme::Dark,
            thickness: 2.0,
            color: (255, 255, 255, 200),
            vibration_color: (255, 80, 80, 200),
        }
    }

    #[test]
    fn writes_funscript_and_counts() {
        let renderer = MediaRenderCapsule::new();
        let hud = sample_hud();
        let grains = vec![AudioGrain {
            time_ms: 0,
            kind: AudioGrainKind::ImpactHigh,
            gain_db: 0.0,
            eq_tilt_db: 0.0,
            preview: false,
        }];
        let plan = RenderPlan {
            video_frames: 1,
            audio_grains: 1,
            profile: ExportProfile::Simple,
            output_path: None,
            bitrate_kbps: 4000,
            fps: 60,
            width: 960,
            height: 540,
            sample_rate_hz: 48_000,
            audio_channels: 1,
            report: ExportReport {
                duration_ms: 0,
                impact_high: 0,
                impact_low: 0,
                buzz: 0,
                funscript: FunscriptValidation {
                    monotonic: true,
                    clamped: true,
                    density_ok: true,
                    total_actions: 0,
                    duration_ms: 0,
                },
                output_path: None,
                profile: ExportProfile::Simple,
            },
        };
        let out_dir = std::env::temp_dir();
        let result = renderer
            .render(&plan, &hud, &grains, r#"{"version":"1.0","actions":[]}"#, &out_dir)
            .expect("render ok");

        assert_eq!(result.video_frames, 1);
        assert_eq!(result.audio_grains, 1);
        assert!(result.funscript_bytes > 0);
        assert!(result.funscript_path.is_some());
        assert!(result.report_bytes > 0);
        assert!(std::fs::metadata(&result.report_path).is_ok());
        assert!(result.report.funscript.monotonic);
        assert_eq!(result.backend_request.fps, 60);
        assert_eq!(result.backend_request.bitrate_kbps, 4000);
        assert!(result.media_output.is_some());
    }

    #[test]
    fn custom_mux_handler_is_used() {
        let mut renderer = MediaRenderCapsule::new();
        renderer.set_mux_handler(|req, rasters, grains, pcm| {
            Ok(MediaMuxResult {
                output_path: req.target_path.clone(),
                fps: req.fps,
                bitrate_kbps: req.bitrate_kbps,
                video_frames: rasters.len() + grains.len(),
                audio_frames: if pcm.is_empty() { 0 } else { 1 },
            })
        });
        let hud = sample_hud();
        let grains = vec![AudioGrain {
            time_ms: 0,
            kind: AudioGrainKind::ImpactHigh,
            gain_db: 0.0,
            eq_tilt_db: 0.0,
            preview: false,
        }];
        let plan = RenderPlan {
            video_frames: 1,
            audio_grains: 1,
            profile: ExportProfile::Advanced { bitrate_kbps: 5000, fps: 48 },
            output_path: None,
            bitrate_kbps: 5000,
            fps: 48,
            width: 1280,
            height: 720,
            sample_rate_hz: 48_000,
            audio_channels: 1,
            report: ExportReport {
                duration_ms: 0,
                impact_high: 0,
                impact_low: 0,
                buzz: 0,
                funscript: FunscriptValidation {
                    monotonic: true,
                    clamped: true,
                    density_ok: true,
                    total_actions: 0,
                    duration_ms: 0,
                },
                output_path: None,
                profile: ExportProfile::Advanced { bitrate_kbps: 5000, fps: 48 },
            },
        };
        let out_dir = std::env::temp_dir();
        let result = renderer
            .render(&plan, &hud, &grains, r#"{"version":"1.0","actions":[]}"#, &out_dir)
            .expect("render ok");

        assert!(result.media_output.is_some());
        assert_eq!(result.media_output.unwrap().video_frames, 2);
        assert_eq!(result.backend_request.fps, 48);
        assert!(result.report_bytes > 0);
    }
}
