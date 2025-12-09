#![forbid(unsafe_code)]

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::overlay::{HudOverlay, HudZone, ImpactIndicator};

#[derive(Debug, Clone)]
pub struct RasterizedFrame {
    pub time_ms: u64,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct HudRasterizerCapsule {
    generation: AtomicU64,
}

impl Default for HudRasterizerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl HudRasterizerCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
        }
    }

    pub fn render(&self, hud: &HudOverlay, width: u32, height: u32) -> Vec<RasterizedFrame> {
        let mut out = Vec::with_capacity(hud.frames.len());
        for frame in &hud.frames {
            let mut buf = vec![0u8; width as usize * height as usize * 4];
            // Background tint per theme (subtle).
            fill_background(&mut buf, width, height, hud.theme);

            // Ribbon and cursor.
            let y = match frame.zone {
                HudZone::Top => ((height as f32) * 0.15) as u32,
                HudZone::Bottom => ((height as f32) * 0.85) as u32,
            };
            let x = ((frame.position_pct.clamp(0.0, 100.0) / 100.0) * ((width - 1) as f32)) as u32;
            let thickness = hud.thickness.max(1.0).min(20.0) as u32;
            let jitter = frame.jitter;

            draw_horizontal_ribbon(
                &mut buf,
                width,
                height,
                y,
                thickness,
                if jitter { hud.vibration_color } else { hud.color },
            );
            draw_cursor(
                &mut buf,
                width,
                height,
                x,
                thickness,
                if jitter { hud.vibration_color } else { hud.color },
            );

            // Impact indicators and optional ghost.
            for indicator in hud.indicators.iter().chain(hud.ghost_indicator.iter()) {
                draw_indicator(&mut buf, width, height, indicator, frame.time_ms, hud.thickness);
            }

            out.push(RasterizedFrame {
                time_ms: frame.time_ms,
                width,
                height,
                data: buf,
            });
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
        out
    }
}

fn fill_background(buf: &mut [u8], width: u32, height: u32, theme: crate::overlay::HudTheme) {
    let (r, g, b, a) = match theme {
        crate::overlay::HudTheme::Light => (240, 240, 240, 12),
        crate::overlay::HudTheme::Dark => (10, 12, 18, 16),
        crate::overlay::HudTheme::Neon => (2, 10, 18, 18),
    };
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            buf[idx] = r;
            buf[idx + 1] = g;
            buf[idx + 2] = b;
            buf[idx + 3] = a;
        }
    }
}

fn draw_horizontal_ribbon(
    buf: &mut [u8],
    width: u32,
    height: u32,
    y: u32,
    thickness: u32,
    color: (u8, u8, u8, u8),
) {
    let y_start = y.saturating_sub(thickness / 2).min(height.saturating_sub(1));
    let y_end = (y + thickness / 2).min(height.saturating_sub(1));
    for yy in y_start..=y_end {
        for x in 0..width {
            set_px(buf, width, x, yy, color);
        }
    }
}

fn draw_cursor(
    buf: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    thickness: u32,
    color: (u8, u8, u8, u8),
) {
    let x_start = x.saturating_sub(thickness / 2).min(width.saturating_sub(1));
    let x_end = (x + thickness / 2).min(width.saturating_sub(1));
    for xx in x_start..=x_end {
        for y in 0..height {
            set_px(buf, width, xx, y, color);
        }
    }
}

fn draw_indicator(
    buf: &mut [u8],
    width: u32,
    height: u32,
    indicator: &ImpactIndicator,
    frame_time_ms: u64,
    thickness: f32,
) {
    let dt = frame_time_ms.abs_diff(indicator.time_ms);
    // Fade/scale over a 250ms window.
    if dt > 250 {
        return;
    }
    let factor = 1.0 - (dt as f32 / 250.0);
    let base_radius = ((width.min(height)) as f32 * 0.05).max(thickness);
    let radius = (base_radius * factor).max(4.0) as i32;
    let alpha_scale = if indicator.is_ghost { 0.4 } else { 1.0 };
    let color = (
        indicator.color.0,
        indicator.color.1,
        indicator.color.2,
        ((indicator.color.3 as f32) * factor * alpha_scale) as u8,
    );
    let y = match indicator.zone {
        HudZone::Top => ((height as f32) * 0.15) as i32,
        HudZone::Bottom => ((height as f32) * 0.85) as i32,
    };
    let x = (width as f32 * 0.5) as i32;
    draw_filled_circle(buf, width, height, x, y, radius, color);
}

fn draw_filled_circle(
    buf: &mut [u8],
    width: u32,
    height: u32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: (u8, u8, u8, u8),
) {
    let r2 = radius * radius;
    let x_start = (cx - radius).max(0);
    let y_start = (cy - radius).max(0);
    let x_end = (cx + radius).min(width as i32 - 1);
    let y_end = (cy + radius).min(height as i32 - 1);
    for y in y_start..=y_end {
        for x in x_start..=x_end {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                set_px(buf, width, x as u32, y as u32, color);
            }
        }
    }
}

fn set_px(buf: &mut [u8], width: u32, x: u32, y: u32, color: (u8, u8, u8, u8)) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= buf.len() {
        return;
    }
    // Simple overwrite (no alpha blend) since background is already faint.
    buf[idx] = color.0;
    buf[idx + 1] = color.1;
    buf[idx + 2] = color.2;
    buf[idx + 3] = color.3;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflection::InflectionKind;
    use crate::overlay::{HudFrame, HudOverlay, HudTheme, HudZone, ImpactIndicator};

    fn sample_overlay() -> HudOverlay {
        HudOverlay {
            frames: vec![HudFrame {
                time_ms: 0,
                position_pct: 50.0,
                zone: HudZone::Top,
                jitter: false,
            }],
            indicators: vec![ImpactIndicator {
                time_ms: 0,
                kind: InflectionKind::ImpactHigh,
                zone: HudZone::Top,
                is_ghost: false,
                color: (255, 255, 255, 220),
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
    fn rasterizer_outputs_frame_data() {
        let raster = HudRasterizerCapsule::new();
        let hud = sample_overlay();
        let frames = raster.render(&hud, 320, 180);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].width, 320);
        assert_eq!(frames[0].height, 180);
        assert_eq!(frames[0].data.len(), 320 * 180 * 4);
        // Expect non-zero pixels (indicator drawn).
        assert!(frames[0].data.iter().any(|&b| b != 0));
    }
}
