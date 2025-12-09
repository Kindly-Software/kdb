#![forbid(unsafe_code)]

use kindly_rub::{
    app::{KindlyRubAppCapsule, UiControls},
    motion::{MotionBlockCapsule, MotionPattern, MotionTempo},
    timeline::TimelineEntry,
};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // Optional CLI: first arg = output dir. Defaults to ./smoke_out.
    let out_dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap().join("smoke_out"));
    std::fs::create_dir_all(&out_dir)?;

    let mut app = KindlyRubAppCapsule::new();
    // Ensure the built-in in-tree muxer is used (no external FFmpeg).
    app.use_internal_muxer();

    // Sample motion: 0→100% linear, medium tempo, 1.2s duration.
    let block =
        MotionBlockCapsule::new(999, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 1200);
    let entry = TimelineEntry::new(0, 1200, block);

    // Basic UI tuning to exercise export knobs.
    let controls = UiControls {
        export_bitrate_kbps: Some(4200),
        export_fps: Some(60),
        export_path: Some(out_dir.join("smoke.funscript")),
        hud_thickness: Some(3.0),
        hud_color: Some((32, 196, 255, 255)),
        hud_vibration_color: Some((255, 96, 64, 255)),
        ..UiControls::default()
    };

    let artifacts = app.run_and_render_from_ui(&entry, 60, &out_dir, &controls)?;
    let output = artifacts
        .render_output
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "missing render output"))?;

    let video_path = output
        .media_output
        .as_ref()
        .map(|m| m.output_path.clone())
        .unwrap_or_else(|| out_dir.join("output.mp4"));
    let funscript_path = output
        .funscript_path
        .clone()
        .unwrap_or_else(|| out_dir.join("script.funscript"));
    let report_path = out_dir.join("export_report.json");
    let report_str = std::fs::read_to_string(&report_path).unwrap_or_default();
    let export_summary = app.ui_export_summary();

    let video_bytes = std::fs::metadata(&video_path).map(|m| m.len()).unwrap_or(0);
    let funscript_bytes = std::fs::metadata(&funscript_path)
        .map(|m| m.len())
        .unwrap_or(0);

    println!(
        "Kindly_Rub smoke OK: video_bytes={} hud_frames={} grains={} funscript_bytes={} video={:?} funscript={:?} report={:?}",
        video_bytes,
        output.video_frames,
        output.audio_grains,
        funscript_bytes,
        video_path,
        funscript_path,
        report_path
    );
    if let Some(summary) = export_summary {
        println!(
            "Export summary: duration_ms={} impacts_high={} impacts_low={} buzz={} report_bytes={} validation_monotonic={}",
            summary.report.duration_ms,
            summary.report.impact_high,
            summary.report.impact_low,
            summary.report.buzz,
            report_str.len(),
            summary.report.funscript.monotonic
        );
    }

    if video_bytes == 0 || funscript_bytes == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "smoke failed: video_bytes={} funscript_bytes={}",
                video_bytes, funscript_bytes
            ),
        ));
    }

    Ok(())
}
