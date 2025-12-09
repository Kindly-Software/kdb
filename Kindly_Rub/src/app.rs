use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    audio::AudioCapsule,
    export::ExportCapsule,
    inflection::{detect_inflections_tuned, InflectionTuning},
    overlay::{HudOverlay, OverlayCapsule},
    renderer::{MediaRenderCapsule, RenderOutput},
    sampler::{sample_motion_block, MotionSample},
    timeline::TimelineEntry,
    timeline::TimelineCapsule,
};

#[derive(Debug)]
pub struct PipelineArtifacts {
    pub samples: Vec<MotionSample>,
    pub inflections: Vec<crate::inflection::InflectionEvent>,
    pub hud: HudOverlay,
    pub grains: Vec<crate::audio::AudioGrain>,
    pub funscript_json: String,
    pub funscript_validation: crate::funscript::FunscriptValidation,
    pub funscript_actions: Vec<crate::funscript::FunscriptAction>,
    pub render_plan: crate::export::RenderPlan,
    pub render_output: Option<RenderOutput>,
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct KindlyRubAppCapsule {
    generation: AtomicU64,
    pub timeline: TimelineCapsule,
    pub audio: AudioCapsule,
    pub overlay: OverlayCapsule,
    pub export: ExportCapsule,
    pub renderer: MediaRenderCapsule,
    pub inflection_tuning: InflectionTuning,
    pub last_calibration: Option<crate::inflection::CalibrationReport>,
    pub last_export_report: Option<crate::export::ExportReport>,
    pub last_export_paths: Option<ExportPaths>,
}

#[derive(Debug, Clone)]
pub struct LivePreviewState {
    pub cursor_ms: u64,
    pub hud_frame: Option<crate::overlay::HudFrame>,
    pub next_indicator: Option<crate::overlay::ImpactIndicator>,
    pub preview_click: Option<crate::audio::AudioGrain>,
}

#[derive(Debug, Clone)]
pub struct UiPreviewBundle {
    pub live: LivePreviewState,
    pub hover_grains: Vec<crate::audio::AudioGrain>,
    pub duration_ms: u64,
    pub tempo_bpm: u32,
}

#[derive(Debug, Clone)]
pub struct UiDiagnostics {
    pub timeline_hint: crate::timeline::TimelineUiHint,
    pub preview: UiPreviewBundle,
    pub last_calibration: Option<crate::inflection::CalibrationReport>,
    pub last_export_report: Option<crate::export::ExportReport>,
    pub last_export_paths: Option<ExportPaths>,
}

#[derive(Debug, Clone)]
pub struct ExportPaths {
    pub report_path: std::path::PathBuf,
    pub funscript_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExportSummaryView {
    pub report: crate::export::ExportReport,
    pub report_path: std::path::PathBuf,
    pub funscript_path: Option<std::path::PathBuf>,
}

/// UI-friendly overlay snapshot: grid lines + duration/tempo for timeline rendering.
#[derive(Debug, Clone)]
pub struct TimelineOverlayView {
    pub grid_lines_ms: Vec<u64>,
    pub duration_ms: u64,
    pub tempo_bpm: Option<u32>,
}

/// Export panel data for UI consumption (paths + report JSON).
#[derive(Debug, Clone)]
pub struct ExportPanelData {
    pub report: crate::export::ExportReport,
    pub report_json: String,
    pub report_path: std::path::PathBuf,
    pub funscript_path: Option<std::path::PathBuf>,
    pub funscript_size: u64,
    pub validation: crate::export::ExportReportValidation,
}

#[derive(Debug, Clone)]
pub struct TuningExportSummary {
    pub calibration: crate::inflection::CalibrationReport,
    pub export: ExportPanelData,
}

#[derive(Debug, Clone)]
pub struct UiControls {
    pub buzz_threshold_ms: f32,
    pub vibration_scale: f32,
    pub min_buzz_ms: f32,
    pub max_buzz_ms: f32,
    pub export_bitrate_kbps: Option<u32>,
    pub export_fps: Option<u32>,
    pub export_path: Option<std::path::PathBuf>,
    pub hud_thickness: Option<f32>,
    pub hud_color: Option<(u8, u8, u8, u8)>,
    pub hud_vibration_color: Option<(u8, u8, u8, u8)>,
    pub hud_latency_ms: Option<u32>,
    pub hud_lead_lag_ms: Option<i32>,
    pub preview_gain_db: Option<f32>,
    pub preview_eq_tilt_db: Option<f32>,
    pub audio_gain_override_db: Option<f32>,
    pub audio_eq_override_db: Option<f32>,
}

impl Default for UiControls {
    fn default() -> Self {
        Self {
            buzz_threshold_ms: 120.0,
            vibration_scale: 0.5,
            min_buzz_ms: 40.0,
            max_buzz_ms: 400.0,
            export_bitrate_kbps: None,
            export_fps: None,
            export_path: None,
            hud_thickness: None,
            hud_color: None,
            hud_vibration_color: None,
            hud_latency_ms: None,
            hud_lead_lag_ms: None,
            preview_gain_db: None,
            preview_eq_tilt_db: None,
            audio_gain_override_db: None,
            audio_eq_override_db: None,
        }
    }
}

impl KindlyRubAppCapsule {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            timeline: TimelineCapsule::new(),
            audio: AudioCapsule::new(),
            overlay: OverlayCapsule::new(),
            export: ExportCapsule::new(),
            renderer: {
                let mut r = MediaRenderCapsule::new();
                r.register_internal_mux_handler();
                r
            },
            inflection_tuning: InflectionTuning::default(),
            last_calibration: None,
            last_export_report: None,
            last_export_paths: None,
        }
    }

    pub fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_inflection_tuning(&mut self, tuning: InflectionTuning) {
        self.inflection_tuning = tuning;
        self.bump_generation();
    }

    pub fn set_inflection_tuning_from_ui(
        &mut self,
        base_buzz_threshold_ms: f32,
        vibration_scale: f32,
        min_ms: f32,
        max_ms: f32,
    ) {
        self.inflection_tuning = InflectionTuning {
            base_buzz_threshold_ms: base_buzz_threshold_ms.clamp(20.0, 500.0),
            vibration_scale: vibration_scale.clamp(0.2, 1.5),
            min_ms: min_ms.clamp(5.0, 500.0),
            max_ms: max_ms.clamp(50.0, 1000.0),
        };
        self.bump_generation();
    }

    /// Let the frontend register the real in-tree media muxer. This stays pure Rust
    /// and must not call external ffmpeg.
    pub fn set_media_mux_handler<F>(&mut self, handler: F)
    where
        F: Fn(
            &crate::renderer::MediaBackendRequest,
            &[crate::rasterizer::RasterizedFrame],
            &[crate::audio::AudioGrain],
            &[u8],
        ) -> std::io::Result<crate::renderer::MediaMuxResult>
            + Send
            + Sync
            + 'static,
    {
        self.renderer.set_mux_handler(handler);
    }

    /// Use the default internal muxer (no external ffmpeg).
    pub fn use_internal_muxer(&mut self) {
        self.renderer.use_internal_backend();
    }

    /// Apply UI controls (sliders + export options) and return export options derived from them.
    pub fn apply_ui_controls(&mut self, controls: &UiControls) -> crate::export::ExportUiOptions {
        self.set_inflection_tuning_from_ui(
            controls.buzz_threshold_ms,
            controls.vibration_scale,
            controls.min_buzz_ms,
            controls.max_buzz_ms,
        );
        if let Some(thick) = controls.hud_thickness {
            let color = controls
                .hud_color
                .unwrap_or(self.overlay.style().color);
            let vib_color = controls
                .hud_vibration_color
                .unwrap_or(self.overlay.style().vibration_color);
            let latency = controls.hud_latency_ms.unwrap_or(self.overlay.style().latency_indicator_ms);
            let lead_lag = controls.hud_lead_lag_ms.unwrap_or(self.overlay.style().lead_lag_ms);
            self.overlay
                .configure_visuals(thick, color, vib_color, latency, lead_lag);
        }
        if controls.preview_gain_db.is_some() || controls.preview_eq_tilt_db.is_some() {
            self.audio.set_preview_tone(
                controls.preview_gain_db.unwrap_or(-6.0),
                controls.preview_eq_tilt_db.unwrap_or(0.0),
            );
        }
        if controls.audio_gain_override_db.is_some() || controls.audio_eq_override_db.is_some() {
            self.audio.set_live_overrides(
                controls.audio_gain_override_db,
                controls.audio_eq_override_db,
            );
        }
        crate::export::ExportUiOptions {
            bitrate_kbps: controls.export_bitrate_kbps,
            fps: controls.export_fps,
            output_path: controls.export_path.clone(),
        }
    }

    /// Convenience: apply UI controls, then render with resulting export options.
    pub fn run_and_render_from_ui(
        &mut self,
        entry: &TimelineEntry,
        fps: u32,
        output_dir: &std::path::Path,
        controls: &UiControls,
    ) -> Result<PipelineArtifacts, std::io::Error> {
        let opts = self.apply_ui_controls(controls);
        self.run_and_render_with_options(entry, fps, output_dir, &opts)
    }

    pub fn run_pipeline(&self, entry: &TimelineEntry, fps: u32) -> PipelineArtifacts {
        self.run_pipeline_with_profile(entry, fps, crate::export::ExportProfile::Simple, None)
    }

    pub fn run_pipeline_with_profile(
        &self,
        entry: &TimelineEntry,
        fps: u32,
        profile: crate::export::ExportProfile,
        output_path: Option<std::path::PathBuf>,
    ) -> PipelineArtifacts {
        let samples = sample_motion_block(entry, fps);
        let inflections = detect_inflections_tuned(
            &samples,
            &self.inflection_tuning,
            entry.block.range(),
            entry.block.tempo(),
        );
        let hud = self
            .overlay
            .generate_hud(&samples, &inflections, entry.block.range(), entry.start_ms);
        let grains = self
            .audio
            .grains_from_inflections(&inflections, entry.preset_meta.as_ref());
        let funscript_positions: Vec<(u64, f32)> =
            samples.iter().map(|s| (s.time_ms, s.position_pct)).collect();
        let funscript = self.export.render_funscript(&funscript_positions);
        let render_plan = self.export.plan_internal_render(
            &hud.frames,
            &grains,
            &inflections,
            funscript.validation,
            profile,
            output_path,
            fps,
        );

        PipelineArtifacts {
            samples,
            inflections,
            hud,
            grains,
            funscript_json: funscript.json,
            funscript_validation: funscript.validation,
            funscript_actions: funscript.actions,
            render_plan,
            render_output: None,
        }
    }

    pub fn run_and_render(
        &mut self,
        entry: &TimelineEntry,
        fps: u32,
        output_dir: &std::path::Path,
    ) -> Result<PipelineArtifacts, std::io::Error> {
        let mut artifacts = self.run_pipeline(entry, fps);
        let output = self.renderer.render(
            &artifacts.render_plan,
            &artifacts.hud,
            &artifacts.grains,
            &artifacts.funscript_json,
            output_dir,
        )?;
        self.last_export_report = Some(output.report.clone());
        self.last_export_paths = Some(ExportPaths {
            report_path: output.report_path.clone(),
            funscript_path: output.funscript_path.clone(),
        });
        artifacts.render_output = Some(output);
        Ok(artifacts)
    }

    pub fn run_and_render_with_profile(
        &mut self,
        entry: &TimelineEntry,
        fps: u32,
        output_dir: &std::path::Path,
        profile: crate::export::ExportProfile,
        output_path: Option<std::path::PathBuf>,
    ) -> Result<PipelineArtifacts, std::io::Error> {
        let mut artifacts = self.run_pipeline_with_profile(entry, fps, profile, output_path);
        let output = self.renderer.render(
            &artifacts.render_plan,
            &artifacts.hud,
            &artifacts.grains,
            &artifacts.funscript_json,
            output_dir,
        )?;
        self.last_export_report = Some(output.report.clone());
        self.last_export_paths = Some(ExportPaths {
            report_path: output.report_path.clone(),
            funscript_path: output.funscript_path.clone(),
        });
        artifacts.render_output = Some(output);
        Ok(artifacts)
    }

    pub fn run_and_render_with_options(
        &mut self,
        entry: &TimelineEntry,
        fps: u32,
        output_dir: &std::path::Path,
        options: &crate::export::ExportUiOptions,
    ) -> Result<PipelineArtifacts, std::io::Error> {
        let validation = self.export.validate_ui_options(options, fps);
        if !validation.ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "invalid export options (bitrate_ok={}, fps_ok={}, path_ok={})",
                    validation.bitrate_ok, validation.fps_ok, validation.path_ok
                ),
            ));
        }
        let (profile, output_path) = options.to_profile(fps);
        self.run_and_render_with_profile(entry, fps, output_dir, profile, output_path)
    }

    pub fn preview_hover(&self, entry: &TimelineEntry, fps: u32) -> Vec<crate::audio::AudioGrain> {
        self.audio
            .preview_grains_for_hover(entry, fps, &self.inflection_tuning)
    }

    /// Combine live preview + hover audio + timeline UI hint for a selected entry index.
    pub fn timeline_ui_snapshot(
        &mut self,
        selected_idx: Option<usize>,
        fps: u32,
        cursor_ms: u64,
    ) -> Option<(crate::timeline::TimelineUiHint, UiPreviewBundle)> {
        let entry = selected_idx.and_then(|i| self.timeline.entries().get(i)).cloned()?;
        self.timeline.refresh_hint_for(&entry);
        let hint = self
            .timeline
            .ui_hint_with_viewport(selected_idx, Some((entry.start_ms, entry.end_ms())));
        let bundle = self.ui_preview_bundle(&entry, fps, cursor_ms);
        Some((hint, bundle))
    }

    /// Full UI diagnostics bundle: timeline hint + preview + last calibration/export reports.
    pub fn ui_state_bundle(
        &mut self,
        selected_idx: Option<usize>,
        fps: u32,
        cursor_ms: u64,
    ) -> Option<UiDiagnostics> {
        let (hint, preview) = self.timeline_ui_snapshot(selected_idx, fps, cursor_ms)?;
        Some(UiDiagnostics {
            timeline_hint: hint,
            preview,
            last_calibration: self.last_calibration,
            last_export_report: self.last_export_report.clone(),
            last_export_paths: self.last_export_paths.clone(),
        })
    }

    /// Overlay-only data for lightweight timeline redraws (grid + duration/tempo).
    pub fn timeline_overlay_view(
        &mut self,
        selected_idx: Option<usize>,
        fps: u32,
        cursor_ms: u64,
    ) -> Option<TimelineOverlayView> {
        let (hint, _) = self.timeline_ui_snapshot(selected_idx, fps, cursor_ms)?;
        Some(TimelineOverlayView {
            grid_lines_ms: hint.grid_lines_ms.clone(),
            duration_ms: hint.duration_ms,
            tempo_bpm: hint.tempo_bpm,
        })
    }

    /// Update timeline UI controls (zoom/snap/grid/handles).
    pub fn configure_timeline_ui(
        &mut self,
        zoom: Option<f32>,
        snap: Option<crate::timeline::SnapGrid>,
        snap_tolerance_ms: Option<u32>,
        show_stretch_handles: Option<bool>,
    ) {
        if let Some(z) = zoom {
            self.timeline.set_zoom(z);
        }
        if let Some(s) = snap {
            self.timeline.set_snap(Some(s));
        }
        if let Some(tol) = snap_tolerance_ms {
            self.timeline.set_snap_tolerance_ms(tol);
        }
        if let Some(show) = show_stretch_handles {
            self.timeline.set_show_stretch_handles(show);
        }
        self.bump_generation();
    }

    /// Apply drag/stretch to a timeline entry with snapping, returning updated hint + entry.
    pub fn drag_or_stretch_entry(
        &mut self,
        index: usize,
        new_start_ms: Option<u64>,
        new_duration_ms: Option<u64>,
    ) -> Option<(crate::timeline::TimelineEntry, crate::timeline::TimelineUiHint)> {
        let mut updated = None;
        if let Some(start) = new_start_ms {
            updated = self.timeline.drag_entry(index, start);
        }
        if let Some(dur) = new_duration_ms {
            updated = self.timeline.stretch_entry(index, dur);
        }
        if updated.is_none() {
            updated = self.timeline.entries().get(index).cloned();
        }
        let entry = updated?;
        let hint =
            self.timeline
                .ui_hint_with_viewport(Some(index), Some((entry.start_ms, entry.end_ms())));
        Some((entry, hint))
    }

    /// Auto-calibrate inflection tuning from sampled footage; sets tuning in-place.
    pub fn calibrate_inflections_from_samples(
        &mut self,
        samples: &[crate::sampler::MotionSample],
        target_buzz_per_sec: f32,
    ) -> crate::inflection::InflectionTuning {
        let report = self.calibrate_inflections_and_report(samples, target_buzz_per_sec);
        report.tuned
    }

    pub fn calibrate_inflections_and_report(
        &mut self,
        samples: &[crate::sampler::MotionSample],
        target_buzz_per_sec: f32,
    ) -> crate::inflection::CalibrationReport {
        let report = self
            .inflection_tuning
            .calibrate_with_report(samples, target_buzz_per_sec);
        self.inflection_tuning = report.tuned;
        self.last_calibration = Some(report);
        self.bump_generation();
        report
    }

    pub fn last_calibration_report(&self) -> Option<crate::inflection::CalibrationReport> {
        self.last_calibration
    }

    pub fn last_export_report(&self) -> Option<&crate::export::ExportReport> {
        self.last_export_report.as_ref()
    }

    pub fn last_export_paths(&self) -> Option<&ExportPaths> {
        self.last_export_paths.as_ref()
    }

    /// UI-friendly export summary (report + file paths) if a render has been completed.
    pub fn ui_export_summary(&self) -> Option<ExportSummaryView> {
        let report = self.last_export_report.clone()?;
        let paths = self.last_export_paths.clone()?;
        Some(ExportSummaryView {
            report,
            report_path: paths.report_path,
            funscript_path: paths.funscript_path,
        })
    }

    /// Return export artifacts as strings/paths for UI panels (safe if called before render).
    pub fn export_artifacts_for_ui(&self) -> Option<(ExportSummaryView, String)> {
        let summary = self.ui_export_summary()?;
        let report_contents = std::fs::read_to_string(&summary.report_path).unwrap_or_default();
        Some((summary, report_contents))
    }

    /// Aggregate export panel data (report + JSON content + funscript size) for UI display.
    pub fn export_panel_data(&self) -> Option<ExportPanelData> {
        let (summary, report_json) = self.export_artifacts_for_ui()?;
        let funscript_size = summary
            .funscript_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);
        let validation = self
            .export
            .validate_export_report(&report_json, summary.funscript_path.as_ref().and_then(|p| {
                std::fs::read_to_string(p).ok()
            }).as_deref());
        Some(ExportPanelData {
            report: summary.report,
            report_json,
            report_path: summary.report_path,
            funscript_path: summary.funscript_path,
            funscript_size,
            validation,
        })
    }

    /// Calibrate from samples, then render with UI controls, returning both calibration and export summary.
    pub fn calibrate_and_export_summary(
        &mut self,
        entry: &TimelineEntry,
        fps: u32,
        output_dir: &std::path::Path,
        controls: &UiControls,
        samples: &[crate::sampler::MotionSample],
        target_buzz_per_sec: f32,
    ) -> Result<TuningExportSummary, std::io::Error> {
        let calibration = self.calibrate_inflections_and_report(samples, target_buzz_per_sec);
        let _artifacts = self.run_and_render_from_ui(entry, fps, output_dir, controls)?;
        let export = self
            .export_panel_data()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "missing export summary"))?;
        Ok(TuningExportSummary { calibration, export })
    }

    pub fn live_preview(
        &self,
        entry: &TimelineEntry,
        fps: u32,
        cursor_ms: u64,
    ) -> LivePreviewState {
        let samples = sample_motion_block(entry, fps);
        let inflections = detect_inflections_tuned(
            &samples,
            &self.inflection_tuning,
            entry.block.range(),
            entry.block.tempo(),
        );
        let hud = self
            .overlay
            .generate_hud(&samples, &inflections, entry.block.range(), cursor_ms);
        let hud_frame = hud
            .frames
            .iter()
            .min_by_key(|f| f.time_ms.abs_diff(cursor_ms))
            .cloned();
        let next_indicator = hud
            .ghost_indicator
            .clone()
            .or_else(|| {
                hud.indicators
                    .iter()
                    .filter(|i| i.time_ms >= cursor_ms)
                    .min_by_key(|i| i.time_ms)
                    .cloned()
            });
        let preview_click = Some(crate::audio::AudioGrain {
            time_ms: cursor_ms,
            kind: crate::audio::AudioGrainKind::PreviewClick,
            gain_db: -6.0,
            eq_tilt_db: 0.0,
            preview: true,
        });

        LivePreviewState {
            cursor_ms,
            hud_frame,
            next_indicator,
            preview_click,
        }
    }

    pub fn ui_preview_bundle(
        &self,
        entry: &TimelineEntry,
        fps: u32,
        cursor_ms: u64,
    ) -> UiPreviewBundle {
        let live = self.live_preview(entry, fps, cursor_ms);
        let hover_grains = self.preview_hover(entry, fps);
        UiPreviewBundle {
            live,
            hover_grains,
            duration_ms: entry.effective_duration_ms(),
            tempo_bpm: entry.effective_tempo_bpm(),
        }
    }

    /// Compute aggregated funscript across multiple timeline entries (offset-aware).
    pub fn funscript_for_timeline(
        &self,
        entries: &[TimelineEntry],
        fps: u32,
    ) -> (Vec<crate::funscript::FunscriptAction>, crate::funscript::FunscriptValidation) {
        let mut positions = Vec::new();
        for entry in entries {
            let samples = sample_motion_block(entry, fps);
            for s in samples {
                positions.push((entry.start_ms.saturating_add(s.time_ms), s.position_pct));
            }
        }
        positions.sort_by_key(|(t, _)| *t);
        let funscript = self.export.render_funscript(&positions);
        (funscript.actions, funscript.validation)
    }
}

impl Default for KindlyRubAppCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{MotionBlockCapsule, MotionPattern, MotionTempo};
    use crate::timeline::TimelineEntry;
    use crate::export::ExportUiOptions;
    use crate::timeline::SnapGrid;

    #[test]
    fn live_preview_returns_click_and_hud() {
        let app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(30, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let preview = app.live_preview(&entry, 60, 120);
        assert!(preview.hud_frame.is_some());
        assert!(preview.preview_click.is_some());
    }

    #[test]
    fn ui_preview_bundle_contains_hover_audio() {
        let app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(32, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let bundle = app.ui_preview_bundle(&entry, 60, 50);
        assert!(!bundle.hover_grains.is_empty());
        assert!(bundle.duration_ms > 0);
        assert!(bundle.tempo_bpm > 0);
    }

    #[test]
    fn pipeline_aligns_hud_and_audio() {
        let app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(31, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let artifacts = app.run_pipeline(&entry, 60);

        for grain in &artifacts.grains {
            assert!(artifacts
                .inflections
                .iter()
                .any(|e| e.time_ms == grain.time_ms));
        }
        for indicator in &artifacts.hud.indicators {
            assert!(artifacts
                .inflections
                .iter()
                .any(|e| e.time_ms == indicator.time_ms));
        }
        assert!(artifacts.funscript_validation.monotonic);
    }

    #[test]
    fn export_options_choose_advanced_profile() {
        let opts = ExportUiOptions {
            bitrate_kbps: Some(4500),
            fps: Some(48),
            output_path: Some(std::path::PathBuf::from("/tmp/out.mp4")),
        };
        let (profile, path) = opts.to_profile(60);
        match profile {
            crate::export::ExportProfile::Advanced { bitrate_kbps, fps } => {
                assert_eq!(bitrate_kbps, 4500);
                assert_eq!(fps, 48);
            }
            _ => panic!("expected advanced profile"),
        }
        assert!(path.is_some());
    }

    #[test]
    fn ui_tuning_applies_clamps() {
        let mut app = KindlyRubAppCapsule::new();
        app.set_inflection_tuning_from_ui(10.0, 2.0, 1.0, 1500.0);
        let tuning = app.inflection_tuning;
        assert!(tuning.base_buzz_threshold_ms >= 20.0);
        assert!(tuning.vibration_scale <= 1.5);
        assert!(tuning.min_ms >= 5.0);
        assert!(tuning.max_ms <= 1000.0);
    }

    #[test]
    fn timeline_ui_snapshot_returns_hint_and_preview() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(60, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        app.timeline.push(entry);
        app.configure_timeline_ui(Some(1.5), Some(SnapGrid::Milliseconds(50)), Some(20), Some(true));

        let snapshot = app.timeline_ui_snapshot(Some(0), 60, 100).expect("snapshot");
        let hint = snapshot.0;
        let preview = snapshot.1;
        assert!(hint.stretch_handles.is_some());
        assert!(!hint.grid_lines_ms.is_empty());
        assert!(preview.hover_grains.len() > 0);
        assert!(preview.duration_ms > 0);
    }

    #[test]
    fn ui_state_bundle_includes_reports() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(62, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        app.timeline.push(entry);
        app.calibrate_inflections_and_report(&[], 8.0); // will no-op but set last_calibration to None
        // create a calibration with data
        let samples = vec![
            crate::sampler::MotionSample { time_ms: 0, position_pct: 0.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            crate::sampler::MotionSample { time_ms: 30, position_pct: 50.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            crate::sampler::MotionSample { time_ms: 60, position_pct: 100.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
        ];
        let report = app.calibrate_inflections_and_report(&samples, 10.0);
        assert!(report.measured_buzz_per_sec > 0.0);
        let bundle = app.ui_state_bundle(Some(0), 60, 100).expect("bundle");
        assert!(bundle.timeline_hint.grid_lines_ms.len() > 0);
        assert!(bundle.last_calibration.is_some());
        // export report is None before render
        assert!(bundle.last_export_report.is_none());
    }

    #[test]
    fn drag_or_stretch_updates_hint() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(61, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 500);
        app.timeline.push(TimelineEntry::new(10, 500, block));
        app.configure_timeline_ui(None, Some(SnapGrid::Milliseconds(25)), Some(10), None);

        let (entry, hint) = app
            .drag_or_stretch_entry(0, Some(37), Some(420))
            .expect("drag/stretch");
        assert_eq!(entry.start_ms, 25); // snapped
        assert!(hint.duration_ms > 0);
        assert!(!hint.grid_lines_ms.is_empty());
    }

    #[test]
    fn calibrate_from_samples_updates_tuning() {
        let mut app = KindlyRubAppCapsule::new();
        let samples = vec![
            crate::sampler::MotionSample {
                time_ms: 0,
                position_pct: 0.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
            crate::sampler::MotionSample {
                time_ms: 30,
                position_pct: 100.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
            crate::sampler::MotionSample {
                time_ms: 60,
                position_pct: 0.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
        ];
        let before = app.inflection_tuning.base_buzz_threshold_ms;
        let tuned = app.calibrate_inflections_from_samples(&samples, 8.0);
        assert!(tuned.base_buzz_threshold_ms <= before);
    }

    #[test]
    fn calibration_report_is_returned() {
        let mut app = KindlyRubAppCapsule::new();
        let samples = vec![
            crate::sampler::MotionSample {
                time_ms: 0,
                position_pct: 0.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
            crate::sampler::MotionSample {
                time_ms: 30,
                position_pct: 50.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
            crate::sampler::MotionSample {
                time_ms: 60,
                position_pct: 100.0,
                velocity_pct_per_ms: 1.0,
                acceleration_pct_per_ms2: 0.0,
            },
        ];
        let report = app.calibrate_inflections_and_report(&samples, 10.0);
        assert!(report.tuned.base_buzz_threshold_ms >= app.inflection_tuning.min_ms);
        assert!(report.measured_buzz_per_sec > 0.0);
        assert_eq!(report.sample_count, samples.len());
    }

    #[test]
    fn forwards_mux_handler() {
        let mut app = KindlyRubAppCapsule::new();
        app.set_media_mux_handler(|req, _, _, _| {
            Ok(crate::renderer::MediaMuxResult {
                output_path: req.target_path.clone(),
                fps: req.fps,
                bitrate_kbps: req.bitrate_kbps,
                video_frames: 0,
                audio_frames: 0,
            })
        });
        // rely on renderer tests for full invocation; just ensure compilation path exists
    }

    #[test]
    fn apply_ui_controls_returns_export_opts() {
        let mut app = KindlyRubAppCapsule::new();
        let controls = UiControls {
            buzz_threshold_ms: 30.0,
            vibration_scale: 1.2,
            min_buzz_ms: 10.0,
            max_buzz_ms: 600.0,
            export_bitrate_kbps: Some(6000),
            export_fps: Some(72),
            export_path: Some(std::path::PathBuf::from("/tmp/out.mp4")),
            hud_thickness: Some(4.0),
            hud_color: Some((10, 10, 10, 200)),
            hud_vibration_color: Some((200, 50, 50, 200)),
            hud_latency_ms: Some(5),
            hud_lead_lag_ms: Some(-3),
            preview_gain_db: Some(-3.0),
            preview_eq_tilt_db: Some(1.0),
            audio_gain_override_db: Some(2.5),
            audio_eq_override_db: Some(-0.5),
        };
        let opts = app.apply_ui_controls(&controls);
        assert_eq!(opts.bitrate_kbps, Some(6000));
        assert_eq!(opts.fps, Some(72));
        assert!(opts.output_path.is_some());
        assert!(app.inflection_tuning.base_buzz_threshold_ms >= 20.0);
        // Ensure overlay and audio overrides applied
        assert_eq!(app.overlay.style().thickness, 4.0);
        assert_eq!(app.overlay.style().latency_indicator_ms, 5);
        let preview = app.audio.preview_grains_for_hover(
            &TimelineEntry::new(
                0,
                800,
                MotionBlockCapsule::new(90, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800),
            ),
            60,
            &app.inflection_tuning,
        );
        assert!(preview.iter().any(|g| (g.gain_db - -3.0).abs() < 0.1));
    }

    #[test]
    fn run_and_render_from_ui_accepts_defaults() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(34, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let controls = UiControls::default();
        // We only check that it builds the render plan with simple profile and does not panic.
        let plan = app.run_pipeline_with_profile(
            &entry,
            60,
            crate::export::ExportProfile::Simple,
            None,
        );
        assert!(plan.render_plan.video_frames > 0);
        let out_dir = std::env::temp_dir();
        app.use_internal_muxer();
        let result = app.run_and_render_from_ui(&entry, 60, &out_dir, &controls);
        assert!(result.is_ok());
    }

    #[test]
    fn render_emits_export_report_file() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(95, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let out_dir = {
            let mut dir = std::env::temp_dir();
            dir.push("kindly_rub_report_smoke");
            let _ = std::fs::create_dir_all(&dir);
            dir
        };
        app.use_internal_muxer();
        let artifacts = app
            .run_and_render(&entry, 60, &out_dir)
            .expect("render ok");
        let output = artifacts.render_output.expect("render output");
        let report_path = output.report_path;
        let report_str = std::fs::read_to_string(&report_path).expect("report read");
        assert!(report_str.contains("\"monotonic\":true"));
        assert!(report_str.contains("\"impact_high\""));
        let script_str =
            std::fs::read_to_string(output.funscript_path.clone().unwrap()).expect("script");
        assert!(script_str.contains("\"actions\""));
        assert!(app.last_export_report().is_some());
        let summary = app.ui_export_summary().expect("summary");
        assert_eq!(summary.report.duration_ms, output.report.duration_ms);
        assert_eq!(summary.report_path, report_path);
        assert_eq!(summary.funscript_path, output.funscript_path);

        let panel = app.export_panel_data().expect("panel");
        assert!(!panel.report_json.is_empty());
        assert!(panel.funscript_size > 0);
        assert!(panel.validation.ok);
    }

    #[test]
    fn calibrate_and_export_summary_combines_steps() {
        let mut app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(120, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let samples = vec![
            crate::sampler::MotionSample { time_ms: 0, position_pct: 0.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            crate::sampler::MotionSample { time_ms: 40, position_pct: 50.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
            crate::sampler::MotionSample { time_ms: 80, position_pct: 100.0, velocity_pct_per_ms: 1.0, acceleration_pct_per_ms2: 0.0 },
        ];
        let controls = UiControls::default();
        let out_dir = std::env::temp_dir().join("kindly_rub_tuning_summary");
        let _ = std::fs::create_dir_all(&out_dir);
        let summary = app
            .calibrate_and_export_summary(&entry, 60, &out_dir, &controls, &samples, 10.0)
            .expect("tuning+export");
        assert!(summary.calibration.measured_buzz_per_sec > 0.0);
        assert!(summary.export.validation.ok);
        assert!(summary.export.funscript_size > 0);
    }

    #[test]
    fn funscript_multi_entry_is_monotone_and_dense() {
        let app = KindlyRubAppCapsule::new();
        let block_a =
            MotionBlockCapsule::new(40, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let block_b =
            MotionBlockCapsule::new(41, MotionPattern::Vibration, 70, 100, MotionTempo::Rapide, 400);
        let entry_a = TimelineEntry::new(0, 800, block_a);
        let entry_b = TimelineEntry::new(900, 400, block_b);
        let entry_b = entry_b.with_stretch_ppm(1_200_000);
        let (actions, validation) = app.funscript_for_timeline(&[entry_a, entry_b], 90);

        assert!(validation.monotonic);
        assert!(validation.clamped);
        assert!(validation.density_ok);
        assert!(actions.windows(2).all(|w| w[1].at > w[0].at));
        assert!(actions.first().unwrap().at <= 10);
        assert!(actions.last().unwrap().at >= 900);
    }

    #[test]
    fn internal_mux_handler_is_registered_and_aligns_counts() {
        let app = KindlyRubAppCapsule::new();
        let block =
            MotionBlockCapsule::new(35, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let entry = TimelineEntry::new(0, 800, block);
        let mut artifacts = app.run_pipeline(&entry, 60);
        let out_dir = std::env::temp_dir();
        let output = app
            .renderer
            .render(
                &artifacts.render_plan,
                &artifacts.hud,
                &artifacts.grains,
                &artifacts.funscript_json,
                &out_dir,
            )
            .expect("render with internal handler");
        assert!(output.media_output.is_some());
        assert_eq!(output.video_frames, artifacts.hud.frames.len());
        assert_eq!(output.audio_grains, artifacts.grains.len());
        artifacts.render_output = Some(output);
    }

    #[test]
    fn multi_entry_hud_audio_funscript_align() {
        let app = KindlyRubAppCapsule::new();
        let block_a =
            MotionBlockCapsule::new(50, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let block_b =
            MotionBlockCapsule::new(51, MotionPattern::Vibration, 70, 100, MotionTempo::Rapide, 400);
        let entry_a = TimelineEntry::new(0, 800, block_a);
        let entry_b = TimelineEntry::new(900, 400, block_b);

        let art_a = app.run_pipeline(&entry_a, 90);
        let art_b = app.run_pipeline(&entry_b, 120);

        // Within each entry, grains align to inflections
        for (art, entry) in [(&art_a, &entry_a), (&art_b, &entry_b)] {
            for grain in &art.grains {
                assert!(
                    art.inflections.iter().any(|e| e.time_ms == grain.time_ms),
                    "grain not aligned for entry starting at {}",
                    entry.start_ms
                );
            }
            for ind in &art.hud.indicators {
                assert!(
                    art.inflections.iter().any(|e| e.time_ms == ind.time_ms),
                    "indicator not aligned for entry starting at {}",
                    entry.start_ms
                );
            }
        }

        // Combined funscript across entries stays monotone and dense-valid
        let (actions, validation) = app.funscript_for_timeline(&[entry_a, entry_b], 90);
        assert!(validation.monotonic);
        assert!(validation.clamped);
        assert!(validation.density_ok);
        assert!(actions.first().unwrap().at <= 5);
        assert!(actions.last().unwrap().at >= 900);
    }

    #[test]
    fn stretched_variants_keep_alignment() {
        let app = KindlyRubAppCapsule::new();
        let base_block =
            MotionBlockCapsule::new(75, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 800);
        let stretch_ppms = [500_000u32, 800_000, 1_000_000, 1_300_000];

        for (idx, ppm) in stretch_ppms.iter().enumerate() {
            let entry =
                TimelineEntry::new((idx as u64) * 120, 800, base_block.clone()).with_stretch_ppm(*ppm);
            let artifacts = app.run_pipeline(&entry, 90);
            assert!(artifacts.funscript_validation.monotonic);
            for inf in &artifacts.inflections {
                assert!(
                    artifacts.hud.indicators.iter().any(|i| i.time_ms == inf.time_ms),
                    "indicator missing for inflection at {}",
                    inf.time_ms
                );
                assert!(
                    artifacts.grains.iter().any(|g| g.time_ms == inf.time_ms),
                    "grain missing for inflection at {}",
                    inf.time_ms
                );
            }
        }
    }
}
