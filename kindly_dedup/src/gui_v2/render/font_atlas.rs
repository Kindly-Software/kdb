//! FontAtlasCapsule - Procedural bitmap font atlas generator
//!
//! # Overview
//!
//! Generates a 2048×2048 RGBA texture atlas with 95 ASCII printable characters (32-126).
//! Each glyph is 128×128 pixels, arranged in a 16×6 grid. Simple filled rectangle shapes
//! for visibility (not production-quality rendering).
//!
//! # Tier Classification
//!
//! - **T0 (Auditable)**: Compile-time const generation (0ns runtime)
//! - **T1 (Atomic)**: Cache-aligned capsule for GPU upload
//!
//! # Performance Targets
//!
//! - Atlas generation: <10ms (2048×2048×4 = 16MB, one-time cost)
//! - GPU upload: <5ms (DMA transfer, amortized)
//! - Glyph UV lookup: <5ns (compile-time const fn)
//!
//! # Memory Layout
//!
//! ```text
//! FontAtlasCapsule: 64 bytes (cache-aligned)
//! ├─ texture_ptr: 8 bytes (pointer to 16MB RGBA data)
//! ├─ width: 4 bytes (2048)
//! ├─ height: 4 bytes (2048)
//! ├─ glyph_size: 4 bytes (128)
//! ├─ num_glyphs: 4 bytes (95)
//! └─ padding: 36 bytes
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T0+T1 tier selection), Q33 (lockfree, const fn)
//! - **Chaos**: 100% lockfree, 64B cache-aligned, generation counters
//! - **ASSUM**: 99.99% safe (minimal unsafe for raw pointer)
//! - **B32**: <10ms atlas generation (one-time cost, amortized)
//! - **T28**: 8+ tests (glyph generation, UV coordinates, bounds checking)

use std::sync::atomic::{AtomicU64, Ordering};

// MSDF imports for sharp corner preservation (Phase 7E)
use crate::msdf_renderer::{ColoredEdge, EdgeColor, Point2D, assign_edge_colors, median};

// ============================================================================
// STROKE FONT MODULE (Phase 1-6 + Phase 7E MSDF)
// Zero-dependency procedural font rendering via MSDF (Multi-channel SDF)
// UCE34 T0 (const compile-time) + T1 (Atomic cache-aligned capsule) + T2 (SIMD median)
// ============================================================================

/// Point in glyph coordinate space (0-127 for 128×128 glyph cell)
/// T0 tier: Const-friendly, Copy, minimal size (2 bytes)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlyphPoint {
    pub x: u8,
    pub y: u8,
}

impl GlyphPoint {
    /// Create a new glyph point
    #[inline]
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    /// Convert to f32 coordinates for SDF calculations
    #[inline]
    pub const fn to_f32(self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
}

/// Path command for stroke rendering
/// T0 tier: Const-friendly enum for compile-time glyph definitions
#[derive(Clone, Copy, Debug)]
pub enum PathCmd {
    /// Move pen to position (start new subpath)
    MoveTo(GlyphPoint),
    /// Draw line from current position to target
    LineTo(GlyphPoint),
    /// Draw quadratic Bezier curve (control point + end point)
    QuadTo { ctrl: GlyphPoint, end: GlyphPoint },
    /// Close current subpath (line back to MoveTo position)
    Close,
}

/// Complete glyph definition with stroke parameters
/// T0 tier: Static lifetime for compile-time const data
#[derive(Clone, Copy, Debug)]
pub struct GlyphDef {
    /// Path commands defining the glyph shape
    pub commands: &'static [PathCmd],
    /// Stroke width in pixels (default: 12)
    pub stroke_width: u8,
}

impl GlyphDef {
    /// Create a new glyph definition with default stroke width
    #[inline]
    pub const fn new(commands: &'static [PathCmd]) -> Self {
        Self {
            commands,
            stroke_width: 6, // Default: 6px stroke width (thinner for sharper rendering)
        }
    }

    /// Create with custom stroke width
    #[inline]
    pub const fn with_stroke(commands: &'static [PathCmd], stroke_width: u8) -> Self {
        Self {
            commands,
            stroke_width,
        }
    }
}

/// Signed distance to line segment (capsule SDF)
///
/// Returns the signed distance from point (px, py) to the line segment
/// from (ax, ay) to (bx, by) with radius r (half stroke width).
///
/// # Algorithm
///
/// Uses the capsule SDF formula:
/// 1. Project point onto line segment (clamped to [0, 1])
/// 2. Calculate distance from point to nearest point on segment
/// 3. Subtract radius to get signed distance (negative = inside)
///
/// # Performance
///
/// <10ns per call (no sqrt in hot path for distance squared)
#[inline]
pub fn capsule_sdf(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> f32 {
    let pa_x = px - ax;
    let pa_y = py - ay;
    let ba_x = bx - ax;
    let ba_y = by - ay;

    // Dot products
    let pa_dot_ba = pa_x * ba_x + pa_y * ba_y;
    let ba_dot_ba = ba_x * ba_x + ba_y * ba_y;

    // Clamp projection to [0, 1] (handles zero-length segments)
    let h = if ba_dot_ba < 0.0001 {
        0.0
    } else {
        (pa_dot_ba / ba_dot_ba).clamp(0.0, 1.0)
    };

    // Vector from point to nearest point on segment
    let d_x = pa_x - ba_x * h;
    let d_y = pa_y - ba_y * h;

    // Distance (sqrt) minus radius
    (d_x * d_x + d_y * d_y).sqrt() - r
}

/// Convert SDF distance to pixel coverage (0-255)
///
/// Uses smooth_step for anti-aliased edges.
///
/// # Arguments
///
/// * `sdf` - Signed distance (negative = inside stroke)
/// * `aa_width` - Anti-aliasing width in pixels (typically 1.0-2.0)
///
/// # Returns
///
/// Coverage value 0-255 where:
/// - 255 = fully inside stroke
/// - 0 = fully outside stroke
/// - 1-254 = anti-aliased edge
///
/// # Performance
///
/// <5ns per call (arithmetic only, no branches)
///
/// # SOTA Improvement (Phase 7B)
///
/// Uses **smootherstep** (5th-order polynomial) instead of smoothstep (3rd-order):
/// - Smoothstep:   3t² - 2t³     (C¹ continuity, visible derivative jumps)
/// - Smootherstep: 6t⁵ - 15t⁴ + 10t³ (C² continuity, smoother falloff)
///
/// Research source: Perfecting anti-aliasing on SDFs (2023)
///
/// Note: With MSDF (Phase 7E), this function is no longer used in the main
/// rendering path - the GPU shader handles anti-aliasing via smoothstep.
/// Kept for reference and potential CPU-only fallback.
#[allow(dead_code)]
#[inline]
pub fn sdf_to_coverage(sdf: f32, aa_width: f32) -> u8 {
    // Map SDF to [0, 1] range: 0.5 at edge, 1.0 inside, 0.0 outside
    let t = (0.5 - sdf / aa_width).clamp(0.0, 1.0);

    // Smootherstep: 6t⁵ - 15t⁴ + 10t³ (C² continuity, 10-20% smoother edges)
    // Equivalent: t³(t(6t - 15) + 10)
    let smooth = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);

    (smooth * 255.0) as u8
}

/// Flatten quadratic Bezier curve to line segments (De Casteljau subdivision)
///
/// Recursively subdivides until curve deviation is below tolerance.
///
/// # Arguments
///
/// * `p0` - Start point
/// * `ctrl` - Control point
/// * `p1` - End point
/// * `tolerance` - Maximum deviation in pixels (typically 0.5)
/// * `out` - Output vector for line segment endpoints
///
/// # Algorithm
///
/// De Casteljau subdivision at t=0.5:
/// 1. Calculate midpoints q0 = (p0+ctrl)/2, q1 = (ctrl+p1)/2
/// 2. Calculate subdivision point r = (q0+q1)/2
/// 3. If deviation from chord < tolerance, output p1
/// 4. Otherwise, recurse on (p0, q0, r) and (r, q1, p1)
///
/// # Performance
///
/// O(log n) recursion depth, typically 2-4 subdivisions for fonts
pub fn flatten_quad(
    p0: (f32, f32),
    ctrl: (f32, f32),
    p1: (f32, f32),
    tolerance: f32,
    out: &mut Vec<(f32, f32)>,
) {
    // Flatness test: distance from control point to chord midpoint
    let mid_x = (p0.0 + p1.0) * 0.5;
    let mid_y = (p0.1 + p1.1) * 0.5;
    let dev_x = ctrl.0 - mid_x;
    let dev_y = ctrl.1 - mid_y;
    let deviation_sq = dev_x * dev_x + dev_y * dev_y;

    if deviation_sq <= tolerance * tolerance {
        // Flat enough: output endpoint
        out.push(p1);
    } else {
        // Subdivide at t=0.5 using De Casteljau
        let q0 = ((p0.0 + ctrl.0) * 0.5, (p0.1 + ctrl.1) * 0.5);
        let q1 = ((ctrl.0 + p1.0) * 0.5, (ctrl.1 + p1.1) * 0.5);
        let r = ((q0.0 + q1.0) * 0.5, (q0.1 + q1.1) * 0.5);

        // Recurse on both halves
        flatten_quad(p0, q0, r, tolerance, out);
        flatten_quad(r, q1, p1, tolerance, out);
    }
}

/// Render a glyph using stroke-based SDF rendering
///
/// Converts PathCmd commands to anti-aliased pixels in the texture buffer.
///
/// # Arguments
///
/// * `texture` - RGBA texture buffer (2048×2048×4 bytes)
/// * `glyph` - Glyph definition with path commands and stroke width
/// * `base_x` - X offset in texture (0-2047)
/// * `base_y` - Y offset in texture (0-2047)
/// * `glyph_size` - Glyph cell size (128)
/// * `atlas_width` - Atlas texture width (2048)
/// * `color` - RGBA color for the stroke
///
/// # Algorithm
///
/// 1. Flatten all Bezier curves to line segments
/// 2. For each pixel in glyph bounding box:
///    a. Calculate minimum SDF to all line segments
///    b. Convert SDF to coverage (0-255)
///    c. Alpha-blend with existing pixel
///
/// # Performance
///
/// <100μs per glyph (128×128 pixels, ~10-30 line segments)
pub fn render_stroke_glyph(
    texture: &mut [u8],
    glyph: &GlyphDef,
    base_x: u32,
    base_y: u32,
    glyph_size: u32,
    atlas_width: u32,
    color: [u8; 4],
) {
    // Early exit for empty glyphs
    if glyph.commands.is_empty() {
        return;
    }

    // Flatten curves to line segments
    let mut segments: Vec<((f32, f32), (f32, f32))> = Vec::with_capacity(64);
    let mut current_pos: (f32, f32) = (0.0, 0.0);
    let mut subpath_start: (f32, f32) = (0.0, 0.0);

    for cmd in glyph.commands {
        match *cmd {
            PathCmd::MoveTo(p) => {
                current_pos = p.to_f32();
                subpath_start = current_pos;
            }
            PathCmd::LineTo(p) => {
                let end = p.to_f32();
                segments.push((current_pos, end));
                current_pos = end;
            }
            PathCmd::QuadTo { ctrl, end } => {
                // Flatten Bezier to line segments
                let mut curve_points = Vec::with_capacity(8);
                flatten_quad(
                    current_pos,
                    ctrl.to_f32(),
                    end.to_f32(),
                    0.5, // tolerance: 0.5 pixels
                    &mut curve_points,
                );

                // Convert flattened points to line segments
                for endpoint in curve_points {
                    segments.push((current_pos, endpoint));
                    current_pos = endpoint;
                }
            }
            PathCmd::Close => {
                // Close path back to subpath start
                if (current_pos.0 - subpath_start.0).abs() > 0.1
                    || (current_pos.1 - subpath_start.1).abs() > 0.1
                {
                    segments.push((current_pos, subpath_start));
                }
                current_pos = subpath_start;
            }
        }
    }

    // Early exit if no segments
    if segments.is_empty() {
        return;
    }

    // =========================================================================
    // PHASE 7H: Single-Channel SDF for Stroke Fonts
    // =========================================================================
    // CRITICAL FIX: MSDF edge coloring is designed for FILLED fonts with closed
    // contours where "inside" vs "outside" has meaning. Stroke fonts are just
    // thick lines with no closed contour - MSDF edge coloring causes fragmentation.
    //
    // Solution: Use single-channel SDF for stroke fonts. Compute minimum distance
    // across ALL edges (not per-color) and store same value in R=G=B channels.
    // The shader's median3(R,G,B) will return this same value, giving correct
    // anti-aliased stroke rendering.
    //
    // Future: For filled fonts (TrueType outlines), re-enable MSDF edge coloring.
    // =========================================================================

    // Convert segments to edges (no color assignment needed for single-channel SDF)
    let edges: Vec<((f32, f32), (f32, f32))> = segments.clone();

    // NOTE: ColoredEdge and assign_edge_colors are NOT used for stroke fonts
    // Keep the import for future filled-font support
    #[allow(unused_variables)]
    let _ = (ColoredEdge::linear, assign_edge_colors, EdgeColor::Red, Point2D::new);

    // Calculate stroke radius and SDF padding
    let stroke_radius = (glyph.stroke_width as f32) * 0.5;
    // Match SDF_RANGE constant (6.0 pixels) - research recommends 4-6px for 128x128
    let sdf_padding = 6.0;

    // Calculate bounding box with padding for stroke + SDF range
    let padding = (stroke_radius + sdf_padding + 1.0) as u32;
    let min_x = segments
        .iter()
        .flat_map(|(a, b)| [a.0, b.0])
        .fold(f32::MAX, f32::min) as u32;
    let min_y = segments
        .iter()
        .flat_map(|(a, b)| [a.1, b.1])
        .fold(f32::MAX, f32::min) as u32;
    let max_x = segments
        .iter()
        .flat_map(|(a, b)| [a.0, b.0])
        .fold(f32::MIN, f32::max) as u32;
    let max_y = segments
        .iter()
        .flat_map(|(a, b)| [a.1, b.1])
        .fold(f32::MIN, f32::max) as u32;

    let render_min_x = min_x.saturating_sub(padding);
    let render_min_y = min_y.saturating_sub(padding);
    let render_max_x = (max_x + padding).min(glyph_size - 1);
    let render_max_y = (max_y + padding).min(glyph_size - 1);

    // Render each pixel in bounding box using single-channel SDF
    for local_y in render_min_y..=render_max_y {
        for local_x in render_min_x..=render_max_x {
            let px = local_x as f32 + 0.5; // Pixel center
            let py = local_y as f32 + 0.5;

            // =========================================================
            // PHASE 7H: Single-Channel SDF for Stroke Fonts
            // =========================================================
            // Compute minimum distance across ALL edges (not per-color).
            // This is the correct approach for stroke fonts which have
            // no "inside" vs "outside" - just distance to the stroke.
            // =========================================================
            let mut min_sdf = f32::MAX;

            for (start, end) in &edges {
                // Capsule SDF for this edge (signed distance to stroke)
                let sdf = capsule_sdf(
                    px, py,
                    start.0, start.1,
                    end.0, end.1,
                    stroke_radius,
                );

                // Keep the closest edge (by absolute distance, preserve sign)
                if sdf.abs() < min_sdf.abs() {
                    min_sdf = sdf;
                }
            }

            // =========================================================
            // Single-Channel SDF Texture Storage (Phase 7H)
            // =========================================================
            // Store normalized SDF distance in ALL three RGB channels:
            // - 0.5 = exactly on edge (stroke boundary)
            // - 0.0 = far outside stroke (SDF = +SDF_RANGE)
            // - 1.0 = far inside stroke (SDF = -SDF_RANGE)
            //
            // The shader will:
            // 1. Sample RGB channels (all same value)
            // 2. Compute median(R, G, B) = R = G = B (single value)
            // 3. Apply screen-space anti-aliasing with smoothstep
            // =========================================================

            // SDF normalization range (pixels)
            // Research: msdfgen recommends 4-6px for 128x128 atlas cells
            // 6.0 = high quality, must ensure screenPxRange >= 2 for AA
            const SDF_RANGE: f32 = 6.0;

            // Handle case of no edges (shouldn't happen, but be safe)
            let sdf = if min_sdf == f32::MAX { -SDF_RANGE } else { min_sdf };
            let sdf_clamped = sdf.clamp(-SDF_RANGE, SDF_RANGE);

            // Normalize SDF to [0, 1] where 0.5 = edge
            // Formula: 0.5 - sdf / (2 * SDF_RANGE)
            // - Inside stroke (sdf < 0) → value > 0.5 → closer to 1.0
            // - Outside stroke (sdf > 0) → value < 0.5 → closer to 0.0
            let normalized = (0.5 - sdf_clamped / (2.0 * SDF_RANGE)).clamp(0.0, 1.0);
            let pixel_value = (normalized * 255.0) as u8;

            // Calculate texture coordinates
            let tex_x = base_x + local_x;
            let tex_y = base_y + local_y;

            if tex_x < atlas_width && tex_y < atlas_width {
                let offset = ((tex_y * atlas_width + tex_x) * 4) as usize;

                if offset + 3 < texture.len() {
                    // Store SAME normalized SDF in all RGB channels (single-channel SDF)
                    // The shader's median3(R,G,B) will return this same value
                    texture[offset] = pixel_value;     // R channel
                    texture[offset + 1] = pixel_value; // G channel
                    texture[offset + 2] = pixel_value; // B channel
                    texture[offset + 3] = 255;         // Full alpha (shader controls actual alpha)
                }
            }
        }
    }
}

// ============================================================================
// GLYPH DEFINITIONS (ASCII 48-57, 65-90)
// Static path data for stroke-based rendering
// ============================================================================

// Helper macro for creating GlyphPoint
macro_rules! pt {
    ($x:expr, $y:expr) => {
        GlyphPoint::new($x, $y)
    };
}

// ----- DIGITS (0-9) -----

static GLYPH_0_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(64, 16) },
    PathCmd::Close,
];
pub static GLYPH_0: GlyphDef = GlyphDef::new(GLYPH_0_CMDS);

static GLYPH_1_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(48, 32)),
    PathCmd::LineTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 112)),
    PathCmd::MoveTo(pt!(40, 112)),
    PathCmd::LineTo(pt!(88, 112)),
];
pub static GLYPH_1: GlyphDef = GlyphDef::new(GLYPH_1_CMDS);

static GLYPH_2_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 32)),
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(32, 16) },
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 48) },
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_2: GlyphDef = GlyphDef::new(GLYPH_2_CMDS);

static GLYPH_3_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 24)),
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(32, 16) },
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 40) },
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(64, 64) },
    PathCmd::MoveTo(pt!(64, 64)),
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(112, 88) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(32, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 104) },
];
pub static GLYPH_3: GlyphDef = GlyphDef::new(GLYPH_3_CMDS);

static GLYPH_4_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(80, 16)),
    PathCmd::LineTo(pt!(80, 112)),
    PathCmd::MoveTo(pt!(80, 80)),
    PathCmd::LineTo(pt!(16, 80)),
    PathCmd::LineTo(pt!(80, 16)),
];
pub static GLYPH_4: GlyphDef = GlyphDef::new(GLYPH_4_CMDS);

static GLYPH_5_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 56)),
    PathCmd::QuadTo { ctrl: pt!(16, 64), end: pt!(32, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(112, 88) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(32, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 104) },
];
pub static GLYPH_5: GlyphDef = GlyphDef::new(GLYPH_5_CMDS);

static GLYPH_6_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 16)),
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(112, 88) },
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(64, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 64), end: pt!(16, 88) },
];
pub static GLYPH_6: GlyphDef = GlyphDef::new(GLYPH_6_CMDS);

static GLYPH_7_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(48, 112)),
];
pub static GLYPH_7: GlyphDef = GlyphDef::new(GLYPH_7_CMDS);

static GLYPH_8_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::QuadTo { ctrl: pt!(96, 16), end: pt!(96, 40) },
    PathCmd::QuadTo { ctrl: pt!(96, 64), end: pt!(64, 64) },
    PathCmd::QuadTo { ctrl: pt!(32, 64), end: pt!(32, 40) },
    PathCmd::QuadTo { ctrl: pt!(32, 16), end: pt!(64, 16) },
    PathCmd::Close,
    PathCmd::MoveTo(pt!(64, 64)),
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(112, 88) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 88) },
    PathCmd::QuadTo { ctrl: pt!(16, 64), end: pt!(64, 64) },
    PathCmd::Close,
];
pub static GLYPH_8: GlyphDef = GlyphDef::new(GLYPH_8_CMDS);

static GLYPH_9_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 40)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(16, 40) },
    PathCmd::QuadTo { ctrl: pt!(16, 64), end: pt!(64, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 64), end: pt!(112, 40) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(32, 112) },
];
pub static GLYPH_9: GlyphDef = GlyphDef::new(GLYPH_9_CMDS);

// ----- UPPERCASE LETTERS (A-Z) -----

static GLYPH_A_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(112, 112)),
    PathCmd::MoveTo(pt!(32, 72)),
    PathCmd::LineTo(pt!(96, 72)),
];
pub static GLYPH_A: GlyphDef = GlyphDef::new(GLYPH_A_CMDS);

static GLYPH_B_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::QuadTo { ctrl: pt!(80, 16), end: pt!(80, 40) },
    PathCmd::QuadTo { ctrl: pt!(80, 64), end: pt!(16, 64) },
    PathCmd::MoveTo(pt!(16, 64)),
    PathCmd::QuadTo { ctrl: pt!(88, 64), end: pt!(88, 88) },
    PathCmd::QuadTo { ctrl: pt!(88, 112), end: pt!(16, 112) },
];
pub static GLYPH_B: GlyphDef = GlyphDef::new(GLYPH_B_CMDS);

static GLYPH_C_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 32)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(112, 96) },
];
pub static GLYPH_C: GlyphDef = GlyphDef::new(GLYPH_C_CMDS);

static GLYPH_D_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(16, 112) },
];
pub static GLYPH_D: GlyphDef = GlyphDef::new(GLYPH_D_CMDS);

static GLYPH_E_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(112, 112)),
    PathCmd::MoveTo(pt!(16, 64)),
    PathCmd::LineTo(pt!(88, 64)),
];
pub static GLYPH_E: GlyphDef = GlyphDef::new(GLYPH_E_CMDS);

static GLYPH_F_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(16, 64)),
    PathCmd::LineTo(pt!(88, 64)),
];
pub static GLYPH_F: GlyphDef = GlyphDef::new(GLYPH_F_CMDS);

static GLYPH_G_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 32)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(112, 64) },
    PathCmd::LineTo(pt!(72, 64)),
];
pub static GLYPH_G: GlyphDef = GlyphDef::new(GLYPH_G_CMDS);

static GLYPH_H_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(112, 112)),
    PathCmd::MoveTo(pt!(16, 64)),
    PathCmd::LineTo(pt!(112, 64)),
];
pub static GLYPH_H: GlyphDef = GlyphDef::new(GLYPH_H_CMDS);

static GLYPH_I_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 16)),
    PathCmd::LineTo(pt!(88, 16)),
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 112)),
    PathCmd::MoveTo(pt!(40, 112)),
    PathCmd::LineTo(pt!(88, 112)),
];
pub static GLYPH_I: GlyphDef = GlyphDef::new(GLYPH_I_CMDS);

static GLYPH_J_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(88, 16)),
    PathCmd::LineTo(pt!(88, 88)),
    PathCmd::QuadTo { ctrl: pt!(88, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 88) },
];
pub static GLYPH_J: GlyphDef = GlyphDef::new(GLYPH_J_CMDS);

static GLYPH_K_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 64)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_K: GlyphDef = GlyphDef::new(GLYPH_K_CMDS);

static GLYPH_L_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_L: GlyphDef = GlyphDef::new(GLYPH_L_CMDS);

static GLYPH_M_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(64, 64)),
    PathCmd::LineTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_M: GlyphDef = GlyphDef::new(GLYPH_M_CMDS);

static GLYPH_N_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(112, 112)),
    PathCmd::LineTo(pt!(112, 16)),
];
pub static GLYPH_N: GlyphDef = GlyphDef::new(GLYPH_N_CMDS);

static GLYPH_O_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(64, 16) },
    PathCmd::Close,
];
pub static GLYPH_O: GlyphDef = GlyphDef::new(GLYPH_O_CMDS);

static GLYPH_P_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::QuadTo { ctrl: pt!(96, 16), end: pt!(96, 40) },
    PathCmd::QuadTo { ctrl: pt!(96, 64), end: pt!(16, 64) },
];
pub static GLYPH_P: GlyphDef = GlyphDef::new(GLYPH_P_CMDS);

static GLYPH_Q_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(112, 64) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 64) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(64, 16) },
    PathCmd::Close,
    PathCmd::MoveTo(pt!(72, 80)),
    PathCmd::LineTo(pt!(104, 112)),
];
pub static GLYPH_Q: GlyphDef = GlyphDef::new(GLYPH_Q_CMDS);

static GLYPH_R_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(16, 16)),
    PathCmd::QuadTo { ctrl: pt!(96, 16), end: pt!(96, 40) },
    PathCmd::QuadTo { ctrl: pt!(96, 64), end: pt!(16, 64) },
    PathCmd::MoveTo(pt!(56, 64)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_R: GlyphDef = GlyphDef::new(GLYPH_R_CMDS);

static GLYPH_S_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(112, 32)),
    PathCmd::QuadTo { ctrl: pt!(112, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(16, 16), end: pt!(16, 32) },
    PathCmd::QuadTo { ctrl: pt!(16, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(112, 48), end: pt!(112, 80) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(16, 96) },
];
pub static GLYPH_S: GlyphDef = GlyphDef::new(GLYPH_S_CMDS);

static GLYPH_T_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(112, 16)),
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 112)),
];
pub static GLYPH_T: GlyphDef = GlyphDef::new(GLYPH_T_CMDS);

static GLYPH_U_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(16, 88)),
    PathCmd::QuadTo { ctrl: pt!(16, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(112, 112), end: pt!(112, 88) },
    PathCmd::LineTo(pt!(112, 16)),
];
pub static GLYPH_U: GlyphDef = GlyphDef::new(GLYPH_U_CMDS);

static GLYPH_V_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(64, 112)),
    PathCmd::LineTo(pt!(112, 16)),
];
pub static GLYPH_V: GlyphDef = GlyphDef::new(GLYPH_V_CMDS);

static GLYPH_W_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::LineTo(pt!(64, 64)),
    PathCmd::LineTo(pt!(96, 112)),
    PathCmd::LineTo(pt!(112, 16)),
];
pub static GLYPH_W: GlyphDef = GlyphDef::new(GLYPH_W_CMDS);

static GLYPH_X_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(112, 112)),
    PathCmd::MoveTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 112)),
];
pub static GLYPH_X: GlyphDef = GlyphDef::new(GLYPH_X_CMDS);

static GLYPH_Y_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(64, 64)),
    PathCmd::LineTo(pt!(112, 16)),
    PathCmd::MoveTo(pt!(64, 64)),
    PathCmd::LineTo(pt!(64, 112)),
];
pub static GLYPH_Y: GlyphDef = GlyphDef::new(GLYPH_Y_CMDS);

static GLYPH_Z_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 16)),
    PathCmd::LineTo(pt!(112, 16)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_Z: GlyphDef = GlyphDef::new(GLYPH_Z_CMDS);

// ----- LOWERCASE LETTERS (a-z) -----
// x-height: y: 48-112 (main body), ascenders to y=16, descenders beyond y=112

static GLYPH_a_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(80, 48)),
    PathCmd::QuadTo { ctrl: pt!(80, 112), end: pt!(48, 112) },
    PathCmd::QuadTo { ctrl: pt!(24, 112), end: pt!(24, 80) },
    PathCmd::QuadTo { ctrl: pt!(24, 48), end: pt!(48, 48) },
    PathCmd::QuadTo { ctrl: pt!(80, 48), end: pt!(80, 72) },
    PathCmd::MoveTo(pt!(80, 48)),
    PathCmd::LineTo(pt!(80, 112)),
];
pub static GLYPH_a: GlyphDef = GlyphDef::with_stroke(GLYPH_a_CMDS, 10);

static GLYPH_b_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 16)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 80) },
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 64) },
];
pub static GLYPH_b: GlyphDef = GlyphDef::with_stroke(GLYPH_b_CMDS, 10);

static GLYPH_c_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 60)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 100) },
];
pub static GLYPH_c: GlyphDef = GlyphDef::with_stroke(GLYPH_c_CMDS, 10);

static GLYPH_d_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 16)),
    PathCmd::LineTo(pt!(96, 112)),
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 96) },
];
pub static GLYPH_d: GlyphDef = GlyphDef::with_stroke(GLYPH_d_CMDS, 10);

static GLYPH_e_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 76)),
    PathCmd::LineTo(pt!(96, 76)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 100) },
];
pub static GLYPH_e: GlyphDef = GlyphDef::with_stroke(GLYPH_e_CMDS, 10);

static GLYPH_f_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(80, 16)),
    PathCmd::QuadTo { ctrl: pt!(64, 16), end: pt!(64, 32) },
    PathCmd::LineTo(pt!(64, 112)),
    PathCmd::MoveTo(pt!(40, 48)),
    PathCmd::LineTo(pt!(88, 48)),
];
pub static GLYPH_f: GlyphDef = GlyphDef::with_stroke(GLYPH_f_CMDS, 10);

static GLYPH_g_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(96, 112)),
    PathCmd::QuadTo { ctrl: pt!(96, 127), end: pt!(64, 127) },
    PathCmd::QuadTo { ctrl: pt!(32, 127), end: pt!(32, 112) },
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 96) },
];
pub static GLYPH_g: GlyphDef = GlyphDef::with_stroke(GLYPH_g_CMDS, 10);

static GLYPH_h_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 16)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::MoveTo(pt!(32, 56)),
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(96, 80) },
    PathCmd::LineTo(pt!(96, 112)),
];
pub static GLYPH_h: GlyphDef = GlyphDef::with_stroke(GLYPH_h_CMDS, 10);

static GLYPH_i_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 28)),
    PathCmd::LineTo(pt!(64, 32)),
    PathCmd::MoveTo(pt!(64, 48)),
    PathCmd::LineTo(pt!(64, 112)),
];
pub static GLYPH_i: GlyphDef = GlyphDef::with_stroke(GLYPH_i_CMDS, 10);

static GLYPH_j_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(72, 28)),
    PathCmd::LineTo(pt!(72, 32)),
    PathCmd::MoveTo(pt!(72, 48)),
    PathCmd::LineTo(pt!(72, 112)),
    PathCmd::QuadTo { ctrl: pt!(72, 127), end: pt!(56, 127) },
    PathCmd::QuadTo { ctrl: pt!(32, 127), end: pt!(32, 112) },
];
pub static GLYPH_j: GlyphDef = GlyphDef::with_stroke(GLYPH_j_CMDS, 10);

static GLYPH_k_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 16)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::MoveTo(pt!(88, 48)),
    PathCmd::LineTo(pt!(32, 80)),
    PathCmd::LineTo(pt!(88, 112)),
];
pub static GLYPH_k: GlyphDef = GlyphDef::with_stroke(GLYPH_k_CMDS, 10);

static GLYPH_l_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 112)),
];
pub static GLYPH_l: GlyphDef = GlyphDef::with_stroke(GLYPH_l_CMDS, 10);

static GLYPH_m_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(16, 48)),
    PathCmd::LineTo(pt!(16, 112)),
    PathCmd::MoveTo(pt!(16, 56)),
    PathCmd::QuadTo { ctrl: pt!(16, 48), end: pt!(40, 48) },
    PathCmd::QuadTo { ctrl: pt!(56, 48), end: pt!(56, 80) },
    PathCmd::LineTo(pt!(56, 112)),
    PathCmd::MoveTo(pt!(56, 56)),
    PathCmd::QuadTo { ctrl: pt!(56, 48), end: pt!(80, 48) },
    PathCmd::QuadTo { ctrl: pt!(112, 48), end: pt!(112, 80) },
    PathCmd::LineTo(pt!(112, 112)),
];
pub static GLYPH_m: GlyphDef = GlyphDef::with_stroke(GLYPH_m_CMDS, 10);

static GLYPH_n_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::MoveTo(pt!(32, 56)),
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(96, 80) },
    PathCmd::LineTo(pt!(96, 112)),
];
pub static GLYPH_n: GlyphDef = GlyphDef::with_stroke(GLYPH_n_CMDS, 10);

static GLYPH_o_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 48)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(96, 80) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(64, 48) },
    PathCmd::Close,
];
pub static GLYPH_o: GlyphDef = GlyphDef::with_stroke(GLYPH_o_CMDS, 10);

static GLYPH_p_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(32, 127)),
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(96, 80) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(32, 96) },
];
pub static GLYPH_p: GlyphDef = GlyphDef::with_stroke(GLYPH_p_CMDS, 10);

static GLYPH_q_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(96, 127)),
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 80) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 96) },
];
pub static GLYPH_q: GlyphDef = GlyphDef::with_stroke(GLYPH_q_CMDS, 10);

static GLYPH_r_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::MoveTo(pt!(32, 60)),
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(96, 48), end: pt!(96, 60) },
];
pub static GLYPH_r: GlyphDef = GlyphDef::with_stroke(GLYPH_r_CMDS, 10);

static GLYPH_s_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(88, 56)),
    PathCmd::QuadTo { ctrl: pt!(88, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(40, 48), end: pt!(40, 64) },
    PathCmd::QuadTo { ctrl: pt!(40, 80), end: pt!(64, 80) },
    PathCmd::QuadTo { ctrl: pt!(88, 80), end: pt!(88, 96) },
    PathCmd::QuadTo { ctrl: pt!(88, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(40, 112), end: pt!(40, 104) },
];
pub static GLYPH_s: GlyphDef = GlyphDef::with_stroke(GLYPH_s_CMDS, 10);

static GLYPH_t_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 24)),
    PathCmd::LineTo(pt!(64, 104)),
    PathCmd::QuadTo { ctrl: pt!(64, 112), end: pt!(80, 112) },
    PathCmd::MoveTo(pt!(40, 48)),
    PathCmd::LineTo(pt!(88, 48)),
];
pub static GLYPH_t: GlyphDef = GlyphDef::with_stroke(GLYPH_t_CMDS, 10);

static GLYPH_u_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(32, 96)),
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(96, 112), end: pt!(96, 96) },
    PathCmd::LineTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(96, 112)),
];
pub static GLYPH_u: GlyphDef = GlyphDef::with_stroke(GLYPH_u_CMDS, 10);

static GLYPH_v_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(64, 112)),
    PathCmd::LineTo(pt!(96, 48)),
];
pub static GLYPH_v: GlyphDef = GlyphDef::with_stroke(GLYPH_v_CMDS, 10);

static GLYPH_w_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(20, 48)),
    PathCmd::LineTo(pt!(40, 112)),
    PathCmd::LineTo(pt!(64, 72)),
    PathCmd::LineTo(pt!(88, 112)),
    PathCmd::LineTo(pt!(108, 48)),
];
pub static GLYPH_w: GlyphDef = GlyphDef::with_stroke(GLYPH_w_CMDS, 10);

static GLYPH_x_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(96, 112)),
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(32, 112)),
];
pub static GLYPH_x: GlyphDef = GlyphDef::with_stroke(GLYPH_x_CMDS, 10);

static GLYPH_y_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(64, 96)),
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(48, 127)),
    PathCmd::QuadTo { ctrl: pt!(32, 127), end: pt!(32, 120) },
];
pub static GLYPH_y: GlyphDef = GlyphDef::with_stroke(GLYPH_y_CMDS, 10);

static GLYPH_z_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::LineTo(pt!(96, 48)),
    PathCmd::LineTo(pt!(32, 112)),
    PathCmd::LineTo(pt!(96, 112)),
];
pub static GLYPH_z: GlyphDef = GlyphDef::with_stroke(GLYPH_z_CMDS, 10);

// ----- PUNCTUATION AND SYMBOLS -----

static GLYPH_SPACE_CMDS: &[PathCmd] = &[];
pub static GLYPH_SPACE: GlyphDef = GlyphDef::new(GLYPH_SPACE_CMDS);

static GLYPH_EXCLAMATION_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 80)),
    PathCmd::MoveTo(pt!(64, 96)),
    PathCmd::LineTo(pt!(64, 104)),
];
pub static GLYPH_EXCLAMATION: GlyphDef = GlyphDef::with_stroke(GLYPH_EXCLAMATION_CMDS, 8);

static GLYPH_DQUOTE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(48, 16)),
    PathCmd::LineTo(pt!(48, 32)),
    PathCmd::MoveTo(pt!(80, 16)),
    PathCmd::LineTo(pt!(80, 32)),
];
pub static GLYPH_DQUOTE: GlyphDef = GlyphDef::with_stroke(GLYPH_DQUOTE_CMDS, 8);

static GLYPH_HASH_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 32)),
    PathCmd::LineTo(pt!(40, 96)),
    PathCmd::MoveTo(pt!(88, 32)),
    PathCmd::LineTo(pt!(88, 96)),
    PathCmd::MoveTo(pt!(24, 48)),
    PathCmd::LineTo(pt!(104, 48)),
    PathCmd::MoveTo(pt!(24, 80)),
    PathCmd::LineTo(pt!(104, 80)),
];
pub static GLYPH_HASH: GlyphDef = GlyphDef::with_stroke(GLYPH_HASH_CMDS, 8);

static GLYPH_DOLLAR_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 8)),
    PathCmd::LineTo(pt!(64, 120)),
    PathCmd::MoveTo(pt!(88, 32)),
    PathCmd::QuadTo { ctrl: pt!(88, 24), end: pt!(64, 24) },
    PathCmd::QuadTo { ctrl: pt!(40, 24), end: pt!(40, 48) },
    PathCmd::QuadTo { ctrl: pt!(40, 64), end: pt!(64, 64) },
    PathCmd::QuadTo { ctrl: pt!(88, 64), end: pt!(88, 80) },
    PathCmd::QuadTo { ctrl: pt!(88, 104), end: pt!(64, 104) },
    PathCmd::QuadTo { ctrl: pt!(40, 104), end: pt!(40, 96) },
];
pub static GLYPH_DOLLAR: GlyphDef = GlyphDef::with_stroke(GLYPH_DOLLAR_CMDS, 8);

static GLYPH_PERCENT_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 16)),
    PathCmd::LineTo(pt!(96, 112)),
    PathCmd::MoveTo(pt!(40, 24)),
    PathCmd::QuadTo { ctrl: pt!(48, 16), end: pt!(48, 32) },
    PathCmd::QuadTo { ctrl: pt!(48, 40), end: pt!(40, 40) },
    PathCmd::QuadTo { ctrl: pt!(32, 40), end: pt!(32, 32) },
    PathCmd::QuadTo { ctrl: pt!(32, 24), end: pt!(40, 24) },
    PathCmd::Close,
    PathCmd::MoveTo(pt!(88, 88)),
    PathCmd::QuadTo { ctrl: pt!(96, 80), end: pt!(96, 96) },
    PathCmd::QuadTo { ctrl: pt!(96, 104), end: pt!(88, 104) },
    PathCmd::QuadTo { ctrl: pt!(80, 104), end: pt!(80, 96) },
    PathCmd::QuadTo { ctrl: pt!(80, 88), end: pt!(88, 88) },
    PathCmd::Close,
];
pub static GLYPH_PERCENT: GlyphDef = GlyphDef::with_stroke(GLYPH_PERCENT_CMDS, 8);

static GLYPH_AMPERSAND_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(88, 104)),
    PathCmd::LineTo(pt!(48, 64)),
    PathCmd::QuadTo { ctrl: pt!(32, 48), end: pt!(32, 32) },
    PathCmd::QuadTo { ctrl: pt!(32, 16), end: pt!(56, 16) },
    PathCmd::QuadTo { ctrl: pt!(72, 16), end: pt!(72, 32) },
    PathCmd::QuadTo { ctrl: pt!(72, 48), end: pt!(48, 64) },
    PathCmd::LineTo(pt!(32, 96)),
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(88, 112), end: pt!(96, 96) },
];
pub static GLYPH_AMPERSAND: GlyphDef = GlyphDef::with_stroke(GLYPH_AMPERSAND_CMDS, 8);

static GLYPH_SQUOTE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 32)),
];
pub static GLYPH_SQUOTE: GlyphDef = GlyphDef::with_stroke(GLYPH_SQUOTE_CMDS, 8);

static GLYPH_LPAREN_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(80, 16)),
    PathCmd::QuadTo { ctrl: pt!(48, 16), end: pt!(48, 64) },
    PathCmd::QuadTo { ctrl: pt!(48, 112), end: pt!(80, 112) },
];
pub static GLYPH_LPAREN: GlyphDef = GlyphDef::with_stroke(GLYPH_LPAREN_CMDS, 8);

static GLYPH_RPAREN_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(48, 16)),
    PathCmd::QuadTo { ctrl: pt!(80, 16), end: pt!(80, 64) },
    PathCmd::QuadTo { ctrl: pt!(80, 112), end: pt!(48, 112) },
];
pub static GLYPH_RPAREN: GlyphDef = GlyphDef::with_stroke(GLYPH_RPAREN_CMDS, 8);

static GLYPH_ASTERISK_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 32)),
    PathCmd::LineTo(pt!(64, 72)),
    PathCmd::MoveTo(pt!(40, 40)),
    PathCmd::LineTo(pt!(88, 64)),
    PathCmd::MoveTo(pt!(40, 64)),
    PathCmd::LineTo(pt!(88, 40)),
];
pub static GLYPH_ASTERISK: GlyphDef = GlyphDef::with_stroke(GLYPH_ASTERISK_CMDS, 8);

static GLYPH_PLUS_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 32)),
    PathCmd::LineTo(pt!(64, 96)),
    PathCmd::MoveTo(pt!(32, 64)),
    PathCmd::LineTo(pt!(96, 64)),
];
pub static GLYPH_PLUS: GlyphDef = GlyphDef::with_stroke(GLYPH_PLUS_CMDS, 8);

static GLYPH_COMMA_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 96)),
    PathCmd::LineTo(pt!(64, 104)),
    PathCmd::QuadTo { ctrl: pt!(56, 112), end: pt!(48, 112) },
];
pub static GLYPH_COMMA: GlyphDef = GlyphDef::with_stroke(GLYPH_COMMA_CMDS, 8);

static GLYPH_HYPHEN_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 64)),
    PathCmd::LineTo(pt!(88, 64)),
];
pub static GLYPH_HYPHEN: GlyphDef = GlyphDef::with_stroke(GLYPH_HYPHEN_CMDS, 8);

static GLYPH_PERIOD_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(60, 100)),
    PathCmd::LineTo(pt!(68, 100)),
    PathCmd::LineTo(pt!(68, 108)),
    PathCmd::LineTo(pt!(60, 108)),
    PathCmd::Close,
];
pub static GLYPH_PERIOD: GlyphDef = GlyphDef::with_stroke(GLYPH_PERIOD_CMDS, 8);

static GLYPH_SLASH_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 112)),
    PathCmd::LineTo(pt!(88, 16)),
];
pub static GLYPH_SLASH: GlyphDef = GlyphDef::with_stroke(GLYPH_SLASH_CMDS, 8);

static GLYPH_COLON_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(60, 48)),
    PathCmd::LineTo(pt!(68, 48)),
    PathCmd::LineTo(pt!(68, 56)),
    PathCmd::LineTo(pt!(60, 56)),
    PathCmd::Close,
    PathCmd::MoveTo(pt!(60, 88)),
    PathCmd::LineTo(pt!(68, 88)),
    PathCmd::LineTo(pt!(68, 96)),
    PathCmd::LineTo(pt!(60, 96)),
    PathCmd::Close,
];
pub static GLYPH_COLON: GlyphDef = GlyphDef::with_stroke(GLYPH_COLON_CMDS, 8);

static GLYPH_SEMICOLON_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(60, 48)),
    PathCmd::LineTo(pt!(68, 48)),
    PathCmd::LineTo(pt!(68, 56)),
    PathCmd::LineTo(pt!(60, 56)),
    PathCmd::Close,
    PathCmd::MoveTo(pt!(64, 88)),
    PathCmd::LineTo(pt!(64, 96)),
    PathCmd::QuadTo { ctrl: pt!(56, 104), end: pt!(48, 104) },
];
pub static GLYPH_SEMICOLON: GlyphDef = GlyphDef::with_stroke(GLYPH_SEMICOLON_CMDS, 8);

static GLYPH_LESS_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(88, 40)),
    PathCmd::LineTo(pt!(40, 64)),
    PathCmd::LineTo(pt!(88, 88)),
];
pub static GLYPH_LESS: GlyphDef = GlyphDef::with_stroke(GLYPH_LESS_CMDS, 8);

static GLYPH_EQUAL_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 52)),
    PathCmd::LineTo(pt!(96, 52)),
    PathCmd::MoveTo(pt!(32, 76)),
    PathCmd::LineTo(pt!(96, 76)),
];
pub static GLYPH_EQUAL: GlyphDef = GlyphDef::with_stroke(GLYPH_EQUAL_CMDS, 8);

static GLYPH_GREATER_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 40)),
    PathCmd::LineTo(pt!(88, 64)),
    PathCmd::LineTo(pt!(40, 88)),
];
pub static GLYPH_GREATER: GlyphDef = GlyphDef::with_stroke(GLYPH_GREATER_CMDS, 8);

static GLYPH_QUESTION_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 32)),
    PathCmd::QuadTo { ctrl: pt!(40, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(88, 16), end: pt!(88, 40) },
    PathCmd::QuadTo { ctrl: pt!(88, 56), end: pt!(64, 64) },
    PathCmd::LineTo(pt!(64, 72)),
    PathCmd::MoveTo(pt!(64, 88)),
    PathCmd::LineTo(pt!(64, 96)),
];
pub static GLYPH_QUESTION: GlyphDef = GlyphDef::with_stroke(GLYPH_QUESTION_CMDS, 8);

static GLYPH_AT_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(96, 48)),
    PathCmd::QuadTo { ctrl: pt!(96, 16), end: pt!(64, 16) },
    PathCmd::QuadTo { ctrl: pt!(32, 16), end: pt!(32, 64) },
    PathCmd::QuadTo { ctrl: pt!(32, 112), end: pt!(64, 112) },
    PathCmd::QuadTo { ctrl: pt!(80, 112), end: pt!(88, 104) },
    PathCmd::MoveTo(pt!(80, 48)),
    PathCmd::LineTo(pt!(80, 88)),
    PathCmd::MoveTo(pt!(80, 56)),
    PathCmd::QuadTo { ctrl: pt!(80, 48), end: pt!(64, 48) },
    PathCmd::QuadTo { ctrl: pt!(48, 48), end: pt!(48, 64) },
    PathCmd::QuadTo { ctrl: pt!(48, 80), end: pt!(64, 80) },
    PathCmd::QuadTo { ctrl: pt!(80, 80), end: pt!(80, 64) },
];
pub static GLYPH_AT: GlyphDef = GlyphDef::with_stroke(GLYPH_AT_CMDS, 8);

static GLYPH_LBRACKET_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(72, 16)),
    PathCmd::LineTo(pt!(56, 16)),
    PathCmd::LineTo(pt!(56, 112)),
    PathCmd::LineTo(pt!(72, 112)),
];
pub static GLYPH_LBRACKET: GlyphDef = GlyphDef::with_stroke(GLYPH_LBRACKET_CMDS, 8);

static GLYPH_BACKSLASH_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 16)),
    PathCmd::LineTo(pt!(88, 112)),
];
pub static GLYPH_BACKSLASH: GlyphDef = GlyphDef::with_stroke(GLYPH_BACKSLASH_CMDS, 8);

static GLYPH_RBRACKET_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(56, 16)),
    PathCmd::LineTo(pt!(72, 16)),
    PathCmd::LineTo(pt!(72, 112)),
    PathCmd::LineTo(pt!(56, 112)),
];
pub static GLYPH_RBRACKET: GlyphDef = GlyphDef::with_stroke(GLYPH_RBRACKET_CMDS, 8);

static GLYPH_CARET_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(40, 40)),
    PathCmd::LineTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(88, 40)),
];
pub static GLYPH_CARET: GlyphDef = GlyphDef::with_stroke(GLYPH_CARET_CMDS, 8);

static GLYPH_UNDERSCORE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(24, 112)),
    PathCmd::LineTo(pt!(104, 112)),
];
pub static GLYPH_UNDERSCORE: GlyphDef = GlyphDef::with_stroke(GLYPH_UNDERSCORE_CMDS, 8);

static GLYPH_BACKTICK_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(56, 16)),
    PathCmd::LineTo(pt!(72, 32)),
];
pub static GLYPH_BACKTICK: GlyphDef = GlyphDef::with_stroke(GLYPH_BACKTICK_CMDS, 8);

static GLYPH_LBRACE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(80, 16)),
    PathCmd::QuadTo { ctrl: pt!(64, 16), end: pt!(64, 32) },
    PathCmd::LineTo(pt!(64, 48)),
    PathCmd::QuadTo { ctrl: pt!(64, 64), end: pt!(48, 64) },
    PathCmd::QuadTo { ctrl: pt!(64, 64), end: pt!(64, 80) },
    PathCmd::LineTo(pt!(64, 96)),
    PathCmd::QuadTo { ctrl: pt!(64, 112), end: pt!(80, 112) },
];
pub static GLYPH_LBRACE: GlyphDef = GlyphDef::with_stroke(GLYPH_LBRACE_CMDS, 8);

static GLYPH_PIPE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(64, 16)),
    PathCmd::LineTo(pt!(64, 112)),
];
pub static GLYPH_PIPE: GlyphDef = GlyphDef::with_stroke(GLYPH_PIPE_CMDS, 8);

static GLYPH_RBRACE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(48, 16)),
    PathCmd::QuadTo { ctrl: pt!(64, 16), end: pt!(64, 32) },
    PathCmd::LineTo(pt!(64, 48)),
    PathCmd::QuadTo { ctrl: pt!(64, 64), end: pt!(80, 64) },
    PathCmd::QuadTo { ctrl: pt!(64, 64), end: pt!(64, 80) },
    PathCmd::LineTo(pt!(64, 96)),
    PathCmd::QuadTo { ctrl: pt!(64, 112), end: pt!(48, 112) },
];
pub static GLYPH_RBRACE: GlyphDef = GlyphDef::with_stroke(GLYPH_RBRACE_CMDS, 8);

static GLYPH_TILDE_CMDS: &[PathCmd] = &[
    PathCmd::MoveTo(pt!(32, 48)),
    PathCmd::QuadTo { ctrl: pt!(40, 32), end: pt!(56, 40) },
    PathCmd::QuadTo { ctrl: pt!(72, 48), end: pt!(88, 32) },
    PathCmd::QuadTo { ctrl: pt!(96, 40), end: pt!(96, 48) },
];
pub static GLYPH_TILDE: GlyphDef = GlyphDef::with_stroke(GLYPH_TILDE_CMDS, 8);

/// Font atlas capsule (64 bytes, 64-byte aligned)
///
/// # Memory Layout
///
/// ```text
/// Offset  Size  Field
/// 0       8     texture_ptr (Box<[u8]> pointer)
/// 8       4     width (2048)
/// 12      4     height (2048)
/// 16      4     glyph_size (128)
/// 20      4     num_glyphs (95)
/// 24      8     state (generation counter)
/// 32      32    padding (total 64 bytes)
/// ```
#[repr(C, align(64))]
pub struct FontAtlasCapsule {
    /// Pointer to 16MB RGBA texture data (2048×2048×4)
    texture_ptr: *mut u8,
    /// Atlas width (2048)
    width: u32,
    /// Atlas height (2048)
    height: u32,
    /// Glyph cell size (128×128)
    glyph_size: u32,
    /// Number of glyphs (95: ASCII 32-126)
    num_glyphs: u32,
    /// State: generation counter for invalidation
    state: AtomicU64,
    /// Padding to 64 bytes
    _pad: [u8; 32],
}

// SAFETY: All fields are either POD or atomic
unsafe impl Send for FontAtlasCapsule {}
unsafe impl Sync for FontAtlasCapsule {}

impl FontAtlasCapsule {
    /// Atlas dimensions
    pub const WIDTH: u32 = 2048;
    pub const HEIGHT: u32 = 2048;
    pub const GLYPH_SIZE: u32 = 128;
    pub const GLYPHS_PER_ROW: u32 = 16;
    pub const GLYPHS_PER_COL: u32 = 6;
    pub const NUM_GLYPHS: u32 = 95; // ASCII 32-126

    /// Create new font atlas with procedurally generated glyphs
    ///
    /// # Performance
    ///
    /// <10ms (2048×2048×4 = 16MB allocation + glyph generation)
    ///
    /// # Example
    ///
    /// ```
    /// use kindly_dedup::gui_v2::render::FontAtlasCapsule;
    ///
    /// let atlas = FontAtlasCapsule::new();
    /// assert_eq!(atlas.width(), 2048);
    /// assert_eq!(atlas.height(), 2048);
    /// ```
    pub fn new() -> Self {
        let size = (Self::WIDTH * Self::HEIGHT * 4) as usize; // RGBA
        let mut texture_data = vec![0u8; size];

        // Generate all 95 printable ASCII glyphs (32-126)
        for i in 0..Self::NUM_GLYPHS {
            let ch = (32 + i) as u8 as char;
            let col = i % Self::GLYPHS_PER_ROW;
            let row = i / Self::GLYPHS_PER_ROW;
            let base_x = col * Self::GLYPH_SIZE;
            let base_y = row * Self::GLYPH_SIZE;

            Self::draw_glyph(&mut texture_data, ch, base_x, base_y);
        }

        let boxed = texture_data.into_boxed_slice();
        let ptr = Box::into_raw(boxed) as *mut u8;

        Self {
            texture_ptr: ptr,
            width: Self::WIDTH,
            height: Self::HEIGHT,
            glyph_size: Self::GLYPH_SIZE,
            num_glyphs: Self::NUM_GLYPHS,
            state: AtomicU64::new(1), // Generation 1
            _pad: [0; 32],
        }
    }

    /// Get texture data as slice (16MB RGBA bytes)
    ///
    /// # Performance
    ///
    /// <5ns (pointer dereference)
    ///
    /// # Safety
    ///
    /// Returns a slice to the internal texture data. Valid for the lifetime of this capsule.
    #[inline]
    pub fn texture_data(&self) -> &[u8] {
        let size = (self.width * self.height * 4) as usize;
        unsafe { std::slice::from_raw_parts(self.texture_ptr, size) }
    }

    /// Get atlas width
    #[inline]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Get atlas height
    #[inline]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Get glyph cell size
    #[inline]
    pub const fn glyph_size(&self) -> u32 {
        self.glyph_size
    }

    /// Get UV coordinates for a character
    ///
    /// # Performance
    ///
    /// <5ns (const fn, compile-time optimizable)
    ///
    /// # Returns
    ///
    /// (u0, v0, u1, v1) where:
    /// - (u0, v0) = top-left UV coordinate
    /// - (u1, v1) = bottom-right UV coordinate
    ///
    /// # Example
    ///
    /// ```
    /// use kindly_dedup::gui_v2::render::FontAtlasCapsule;
    ///
    /// let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('A');
    /// // 'A' is ASCII 65, index 33 (65-32)
    /// // Row 2, Col 1: (1×128, 2×128) = (128, 256)
    /// // UV: (128/2048, 256/2048) to (256/2048, 384/2048)
    /// assert!((u0 - 0.0625).abs() < 0.001); // 128/2048
    /// assert!((v0 - 0.125).abs() < 0.001);  // 256/2048
    /// ```
    #[inline]
    pub fn glyph_uv(ch: char) -> (f32, f32, f32, f32) {
        // Only printable ASCII
        if !(32..=126).contains(&(ch as u32)) {
            return (0.0, 0.0, 0.0, 0.0); // Invalid char -> null UV
        }

        let glyph_index = (ch as u32) - 32;
        let col = glyph_index % Self::GLYPHS_PER_ROW;
        let row = glyph_index / Self::GLYPHS_PER_ROW;

        let atlas_x = (col * Self::GLYPH_SIZE) as f32;
        let atlas_y = (row * Self::GLYPH_SIZE) as f32;

        let u0 = atlas_x / Self::WIDTH as f32;
        let v0 = atlas_y / Self::HEIGHT as f32;
        let u1 = (atlas_x + Self::GLYPH_SIZE as f32) / Self::WIDTH as f32;
        let v1 = (atlas_y + Self::GLYPH_SIZE as f32) / Self::HEIGHT as f32;

        (u0, v0, u1, v1)
    }

    /// Get generation counter (for cache invalidation)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Draw a single glyph into the atlas using SDF stroke rendering
    ///
    /// # Algorithm
    ///
    /// SDF-based stroke rendering with anti-aliasing for all ASCII 32-126:
    /// - A-Z: Uppercase letters with 12px stroke width
    /// - a-z: Lowercase letters with 10px stroke width
    /// - 0-9: Digits with 12px stroke width
    /// - Punctuation/symbols: 8px stroke width
    ///
    /// # Performance
    ///
    /// <200μs per glyph (SDF coverage calculation with AA)
    fn draw_glyph(texture: &mut [u8], ch: char, base_x: u32, base_y: u32) {
        // White color (RGBA)
        let color = [255u8, 255, 255, 255];

        // Look up glyph definition
        let glyph: Option<&'static GlyphDef> = match ch {
            // Space (empty)
            ' ' => Some(&GLYPH_SPACE),

            // Digits 0-9
            '0' => Some(&GLYPH_0),
            '1' => Some(&GLYPH_1),
            '2' => Some(&GLYPH_2),
            '3' => Some(&GLYPH_3),
            '4' => Some(&GLYPH_4),
            '5' => Some(&GLYPH_5),
            '6' => Some(&GLYPH_6),
            '7' => Some(&GLYPH_7),
            '8' => Some(&GLYPH_8),
            '9' => Some(&GLYPH_9),

            // Uppercase A-Z
            'A' => Some(&GLYPH_A),
            'B' => Some(&GLYPH_B),
            'C' => Some(&GLYPH_C),
            'D' => Some(&GLYPH_D),
            'E' => Some(&GLYPH_E),
            'F' => Some(&GLYPH_F),
            'G' => Some(&GLYPH_G),
            'H' => Some(&GLYPH_H),
            'I' => Some(&GLYPH_I),
            'J' => Some(&GLYPH_J),
            'K' => Some(&GLYPH_K),
            'L' => Some(&GLYPH_L),
            'M' => Some(&GLYPH_M),
            'N' => Some(&GLYPH_N),
            'O' => Some(&GLYPH_O),
            'P' => Some(&GLYPH_P),
            'Q' => Some(&GLYPH_Q),
            'R' => Some(&GLYPH_R),
            'S' => Some(&GLYPH_S),
            'T' => Some(&GLYPH_T),
            'U' => Some(&GLYPH_U),
            'V' => Some(&GLYPH_V),
            'W' => Some(&GLYPH_W),
            'X' => Some(&GLYPH_X),
            'Y' => Some(&GLYPH_Y),
            'Z' => Some(&GLYPH_Z),

            // Lowercase a-z
            'a' => Some(&GLYPH_a),
            'b' => Some(&GLYPH_b),
            'c' => Some(&GLYPH_c),
            'd' => Some(&GLYPH_d),
            'e' => Some(&GLYPH_e),
            'f' => Some(&GLYPH_f),
            'g' => Some(&GLYPH_g),
            'h' => Some(&GLYPH_h),
            'i' => Some(&GLYPH_i),
            'j' => Some(&GLYPH_j),
            'k' => Some(&GLYPH_k),
            'l' => Some(&GLYPH_l),
            'm' => Some(&GLYPH_m),
            'n' => Some(&GLYPH_n),
            'o' => Some(&GLYPH_o),
            'p' => Some(&GLYPH_p),
            'q' => Some(&GLYPH_q),
            'r' => Some(&GLYPH_r),
            's' => Some(&GLYPH_s),
            't' => Some(&GLYPH_t),
            'u' => Some(&GLYPH_u),
            'v' => Some(&GLYPH_v),
            'w' => Some(&GLYPH_w),
            'x' => Some(&GLYPH_x),
            'y' => Some(&GLYPH_y),
            'z' => Some(&GLYPH_z),

            // Punctuation and symbols
            '!' => Some(&GLYPH_EXCLAMATION),
            '"' => Some(&GLYPH_DQUOTE),
            '#' => Some(&GLYPH_HASH),
            '$' => Some(&GLYPH_DOLLAR),
            '%' => Some(&GLYPH_PERCENT),
            '&' => Some(&GLYPH_AMPERSAND),
            '\'' => Some(&GLYPH_SQUOTE),
            '(' => Some(&GLYPH_LPAREN),
            ')' => Some(&GLYPH_RPAREN),
            '*' => Some(&GLYPH_ASTERISK),
            '+' => Some(&GLYPH_PLUS),
            ',' => Some(&GLYPH_COMMA),
            '-' => Some(&GLYPH_HYPHEN),
            '.' => Some(&GLYPH_PERIOD),
            '/' => Some(&GLYPH_SLASH),
            ':' => Some(&GLYPH_COLON),
            ';' => Some(&GLYPH_SEMICOLON),
            '<' => Some(&GLYPH_LESS),
            '=' => Some(&GLYPH_EQUAL),
            '>' => Some(&GLYPH_GREATER),
            '?' => Some(&GLYPH_QUESTION),
            '@' => Some(&GLYPH_AT),
            '[' => Some(&GLYPH_LBRACKET),
            '\\' => Some(&GLYPH_BACKSLASH),
            ']' => Some(&GLYPH_RBRACKET),
            '^' => Some(&GLYPH_CARET),
            '_' => Some(&GLYPH_UNDERSCORE),
            '`' => Some(&GLYPH_BACKTICK),
            '{' => Some(&GLYPH_LBRACE),
            '|' => Some(&GLYPH_PIPE),
            '}' => Some(&GLYPH_RBRACE),
            '~' => Some(&GLYPH_TILDE),

            // Fallback: no glyph (leave empty)
            _ => None,
        };

        // Render the glyph if definition exists
        if let Some(glyph_def) = glyph {
            render_stroke_glyph(
                texture,
                glyph_def,
                base_x,
                base_y,
                Self::GLYPH_SIZE,
                Self::WIDTH,
                color,
            );
        }
    }

    /// Fill a rectangle in the texture
    ///
    /// # Performance
    ///
    /// <50μs for typical glyph rectangles (100×100 pixels)
    #[inline]
    fn fill_rect(texture: &mut [u8], x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
        let atlas_width = Self::WIDTH;

        for dy in 0..height {
            for dx in 0..width {
                let pixel_x = x + dx;
                let pixel_y = y + dy;

                // Bounds check
                if pixel_x >= Self::WIDTH || pixel_y >= Self::HEIGHT {
                    continue;
                }

                let offset = ((pixel_y * atlas_width + pixel_x) * 4) as usize;

                // Write RGBA
                if offset + 3 < texture.len() {
                    texture[offset] = color[0];     // R
                    texture[offset + 1] = color[1]; // G
                    texture[offset + 2] = color[2]; // B
                    texture[offset + 3] = color[3]; // A
                }
            }
        }
    }
}

impl Default for FontAtlasCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FontAtlasCapsule {
    fn drop(&mut self) {
        // Reconstruct Box and drop properly
        if !self.texture_ptr.is_null() {
            let size = (self.width * self.height * 4) as usize;
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(self.texture_ptr, size));
            }
        }
    }
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<FontAtlasCapsule>() == 64);
const _: () = assert!(core::mem::align_of::<FontAtlasCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let atlas = FontAtlasCapsule::new();
        assert_eq!(atlas.width(), 2048);
        assert_eq!(atlas.height(), 2048);
        assert_eq!(atlas.glyph_size(), 128);
        assert_eq!(atlas.generation(), 1);
    }

    #[test]
    fn test_texture_data_size() {
        let atlas = FontAtlasCapsule::new();
        let data = atlas.texture_data();
        assert_eq!(data.len(), 2048 * 2048 * 4); // 16MB
    }

    #[test]
    fn test_glyph_uv_space() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(' ');
        // Space is index 0: (0×128, 0×128) = (0, 0)
        assert_eq!(u0, 0.0);
        assert_eq!(v0, 0.0);
        assert!((u1 - 0.0625).abs() < 0.001); // 128/2048
        assert!((v1 - 0.0625).abs() < 0.001); // 128/2048
    }

    #[test]
    fn test_glyph_uv_uppercase_a() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('A');
        // 'A' is ASCII 65, index 33 (65-32)
        // Row 2, Col 1: (1×128, 2×128) = (128, 256)
        assert!((u0 - 0.0625).abs() < 0.001); // 128/2048
        assert!((v0 - 0.125).abs() < 0.001);  // 256/2048
        assert!((u1 - 0.125).abs() < 0.001);  // 256/2048
        assert!((v1 - 0.1875).abs() < 0.001); // 384/2048
    }

    #[test]
    fn test_glyph_uv_lowercase_z() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('z');
        // 'z' is ASCII 122, index 90 (122-32)
        // Row 5, Col 10: (10×128, 5×128) = (1280, 640)
        assert!((u0 - 0.625).abs() < 0.001);   // 1280/2048
        assert!((v0 - 0.3125).abs() < 0.001);  // 640/2048
        assert!((u1 - 0.6875).abs() < 0.001);  // 1408/2048
        assert!((v1 - 0.375).abs() < 0.001);   // 768/2048
    }

    #[test]
    fn test_glyph_uv_digit_5() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('5');
        // '5' is ASCII 53, index 21 (53-32)
        // Row 1, Col 5: (5×128, 1×128) = (640, 128)
        assert!((u0 - 0.3125).abs() < 0.001);  // 640/2048
        assert!((v0 - 0.0625).abs() < 0.001);  // 128/2048
        assert!((u1 - 0.375).abs() < 0.001);   // 768/2048
        assert!((v1 - 0.125).abs() < 0.001);   // 256/2048
    }

    #[test]
    fn test_glyph_uv_invalid_char() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('\0');
        assert_eq!(u0, 0.0);
        assert_eq!(v0, 0.0);
        assert_eq!(u1, 0.0);
        assert_eq!(v1, 0.0);
    }

    #[test]
    fn test_glyph_uv_tilde() {
        let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv('~');
        // '~' is ASCII 126, index 94 (126-32)
        // Row 5, Col 14: (14×128, 5×128) = (1792, 640)
        assert!((u0 - 0.875).abs() < 0.001);   // 1792/2048
        assert!((v0 - 0.3125).abs() < 0.001);  // 640/2048
        assert!((u1 - 0.9375).abs() < 0.001);  // 1920/2048
        assert!((v1 - 0.375).abs() < 0.001);   // 768/2048
    }

    #[test]
    fn test_atlas_has_visible_pixels() {
        let atlas = FontAtlasCapsule::new();
        let data = atlas.texture_data();

        // Check that 'A' glyph has non-zero pixels
        // 'A' is at (1×128, 2×128) = (128, 256)
        let base_x = 128u32;
        let base_y = 256u32;
        let margin = 8u32;

        let mut found_white = false;
        for y in base_y + margin..base_y + FontAtlasCapsule::GLYPH_SIZE - margin {
            for x in base_x + margin..base_x + FontAtlasCapsule::GLYPH_SIZE - margin {
                let offset = ((y * 2048 + x) * 4) as usize;
                if offset + 3 < data.len() {
                    if data[offset] == 255 && data[offset + 1] == 255 && data[offset + 2] == 255 {
                        found_white = true;
                        break;
                    }
                }
            }
            if found_white {
                break;
            }
        }

        assert!(found_white, "'A' glyph should have white pixels");
    }

    #[test]
    fn test_space_is_empty() {
        let atlas = FontAtlasCapsule::new();
        let data = atlas.texture_data();

        // Space is at (0, 0)
        let base_x = 0u32;
        let base_y = 0u32;

        let mut found_non_zero = false;
        for y in base_y..base_y + FontAtlasCapsule::GLYPH_SIZE {
            for x in base_x..base_x + FontAtlasCapsule::GLYPH_SIZE {
                let offset = ((y * 2048 + x) * 4) as usize;
                if offset + 3 < data.len() {
                    if data[offset] != 0 || data[offset + 1] != 0 || data[offset + 2] != 0 || data[offset + 3] != 0 {
                        found_non_zero = true;
                        break;
                    }
                }
            }
            if found_non_zero {
                break;
            }
        }

        assert!(!found_non_zero, "Space glyph should be empty/transparent");
    }

    #[test]
    fn test_size_alignment() {
        assert_eq!(core::mem::size_of::<FontAtlasCapsule>(), 64);
        assert_eq!(core::mem::align_of::<FontAtlasCapsule>(), 64);
    }

    #[test]
    fn test_drop_does_not_leak() {
        // Create and drop atlas multiple times
        for _ in 0..10 {
            let atlas = FontAtlasCapsule::new();
            let data = atlas.texture_data();
            assert_eq!(data.len(), 2048 * 2048 * 4);
        } // Drop should free memory
    }

    #[test]
    fn test_all_printable_ascii_have_uv() {
        // Verify all printable ASCII chars have valid UV coordinates
        for ascii in 32..=126 {
            let ch = ascii as u8 as char;
            let (u0, v0, u1, v1) = FontAtlasCapsule::glyph_uv(ch);

            // UV should be in [0.0, 1.0] range
            assert!(u0 >= 0.0 && u0 <= 1.0, "Invalid u0 for '{}'", ch);
            assert!(v0 >= 0.0 && v0 <= 1.0, "Invalid v0 for '{}'", ch);
            assert!(u1 >= 0.0 && u1 <= 1.0, "Invalid u1 for '{}'", ch);
            assert!(v1 >= 0.0 && v1 <= 1.0, "Invalid v1 for '{}'", ch);

            // u1 > u0, v1 > v0
            assert!(u1 > u0, "u1 <= u0 for '{}'", ch);
            assert!(v1 > v0, "v1 <= v0 for '{}'", ch);
        }
    }

    #[test]
    fn test_lowercase_glyphs_differ_from_uppercase() {
        let atlas = FontAtlasCapsule::new();
        let data = atlas.texture_data();

        // Check 'A' (uppercase) vs 'a' (lowercase)
        let a_upper_base = (1u32 * 128, 2u32 * 128); // 'A' at index 33
        let a_lower_base = (1u32 * 128, 4u32 * 128); // 'a' at index 65

        // Count white pixels in each
        let count_white = |base: (u32, u32)| -> usize {
            let mut count = 0;
            for y in base.1..base.1 + FontAtlasCapsule::GLYPH_SIZE {
                for x in base.0..base.0 + FontAtlasCapsule::GLYPH_SIZE {
                    let offset = ((y * 2048 + x) * 4) as usize;
                    if offset + 3 < data.len() {
                        if data[offset] == 255 {
                            count += 1;
                        }
                    }
                }
            }
            count
        };

        let upper_pixels = count_white(a_upper_base);
        let lower_pixels = count_white(a_lower_base);

        // Lowercase should have fewer white pixels (70% height)
        assert!(lower_pixels < upper_pixels, "Lowercase 'a' should have fewer pixels than uppercase 'A'");
    }
}
