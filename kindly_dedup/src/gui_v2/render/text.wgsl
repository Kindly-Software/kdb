// Text rendering shader for gui_v2
//
// Renders text quads sampling from glyph atlas texture.
//
// # Vertex Format
//
// - pos_x, pos_y: Screen position (pixels)
// - tex_u, tex_v: Atlas UV coordinates (0.0-1.0)
// - color_r, color_g, color_b, color_a: Text color (0.0-1.0)
//
// # Uniforms
//
// - screen_width, screen_height: Viewport dimensions for NDC conversion
//
// # Fragment Output
//
// - RGBA color with alpha from atlas texture

// ============================================================================
// Vertex Shader
// ============================================================================

struct VertexInput {
    @location(0) pos_x: f32,
    @location(1) pos_y: f32,
    @location(2) tex_u: f32,
    @location(3) tex_v: f32,
    @location(4) color_r: f32,
    @location(5) color_g: f32,
    @location(6) color_b: f32,
    @location(7) color_a: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    screen_width: f32,
    screen_height: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert screen coordinates to NDC (-1.0 to 1.0)
    // Screen origin is top-left (0, 0)
    // NDC origin is center (0, 0)
    let ndc_x = (in.pos_x / uniforms.screen_width) * 2.0 - 1.0;
    let ndc_y = -(in.pos_y / uniforms.screen_height) * 2.0 + 1.0; // Flip Y

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.tex_coords = vec2<f32>(in.tex_u, in.tex_v);
    out.color = vec4<f32>(in.color_r, in.color_g, in.color_b, in.color_a);

    return out;
}

// ============================================================================
// Fragment Shader - MSDF (Multi-Channel Signed Distance Field)
// ============================================================================
//
// MSDF Algorithm (Chlumsky 2015):
// 1. Sample RGB channels containing normalized SDF distances (0.5 = edge)
// 2. Compute median(R, G, B) to recover sharp corners
// 3. Apply screen-space anti-aliasing with smoothstep + fwidth
//
// Benefits:
// - 2-4× sharper corners (K, E, F, W, M) vs single-channel SDF
// - Resolution-independent (scales infinitely without blur)
// - Screen-space AA adapts to zoom level automatically

@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

// MSDF median reconstruction (Chlumsky formula)
// median(a, b, c) = max(min(a, b), min(max(a, b), c))
// Branchless, 4 min/max ops, preserves sharp corners
fn median3(a: f32, b: f32, c: f32) -> f32 {
    return max(min(a, b), min(max(a, b), c));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample MSDF channels (RGB contain normalized SDF distances)
    let msdf = textureSample(atlas_texture, atlas_sampler, in.tex_coords);

    // MSDF median reconstruction for sharp corners
    // 0.5 = exactly on edge, <0.5 = outside, >0.5 = inside
    let sd = median3(msdf.r, msdf.g, msdf.b);

    // =========================================================================
    // Phase 7I: Fixed Anti-Aliasing for Stroke Fonts
    // =========================================================================
    // Problem: fwidth(sd) can produce very small values causing aliasing.
    // Solution: Use a fixed AA width based on SDF texture resolution.
    //
    // For 128x128 glyph cells with SDF_RANGE=6.0:
    // - 6 pixels of SDF range maps to ~0.047 normalized (6/128)
    // - We want ~1.5px of AA width = 0.012 normalized
    // - Use fixed width with minimum to prevent hard edges
    // =========================================================================

    // Screen-space gradient (still useful for adapting to zoom)
    let screen_px_distance = fwidth(sd);

    // Minimum AA width to prevent aliasing artifacts
    // 0.02 = ~2.5 pixels of AA in normalized SDF space (0.02 * 128 ≈ 2.5)
    let MIN_AA_WIDTH: f32 = 0.02;
    let aa_width = max(screen_px_distance, MIN_AA_WIDTH);

    // Smoothstep edge transition (0.5 = edge threshold)
    let alpha = smoothstep(0.5 - aa_width, 0.5 + aa_width, sd);

    // Apply text color with MSDF alpha
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
