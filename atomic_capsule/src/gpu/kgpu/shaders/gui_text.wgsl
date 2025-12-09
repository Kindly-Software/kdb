// KGPU GUI Text/Glyph Rendering Shader
//
// Purpose: High-quality text rendering using SDF glyph atlas
// Stage: Vertex + Fragment
// Features: SDF text, subpixel positioning, drop shadows, outlines
//
// Performance Target:
// - Glyph throughput: 100K+ glyphs/sec
// - Text rendering: <1ms per 1000 glyphs (1080p)
// - Atlas lookup: <10ns (texture cache hit)
//
// Framework Compliance:
// - UCE34: T7 Heterogeneous (GPU glyph rendering)
// - COCA: Immutable shader (compile-time glyph positioning)
// - B32: <0.5ms per text block (typical 200 glyphs)
//
// References:
// - Valve SDF text rendering: https://steamcdn-a.akamaihd.net/apps/valve/2007/SIGGRAPH2007_AlphaTestedMagnification.pdf
// - Multi-channel SDF: https://github.com/Chlumsky/msdfgen

// --- Vertex Shader (Instanced Glyph Quads) ---

struct GlyphInstance {
    @location(0) position: vec2<f32>,    // Glyph position (screen space)
    @location(1) size: vec2<f32>,        // Glyph size (pixels)
    @location(2) uv_offset: vec2<f32>,   // Atlas UV offset
    @location(3) uv_size: vec2<f32>,     // Atlas UV size
    @location(4) color: vec4<f32>,       // Glyph color (RGBA)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,          // Atlas UV coordinates
    @location(1) color: vec4<f32>,       // Glyph color
    @location(2) position: vec2<f32>,    // Screen position (for effects)
}

struct Uniforms {
    projection: mat4x4<f32>,  // Orthographic projection
    sdf_threshold: f32,        // SDF distance threshold (0.5 typical)
    smoothing: f32,            // Anti-aliasing smoothing (pixels)
    outline_width: f32,        // Outline width (0 = no outline)
    shadow_offset: vec2<f32>,  // Drop shadow offset (pixels)
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
    instance: GlyphInstance,
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    // Generate quad vertices from vertex index (0-3)
    let quad_uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u) * 0.5,  // 0, 1, 0, 1
        f32(vertex_index & 2u) * 0.5             // 0, 0, 1, 1
    );

    // Position in screen space
    let position = instance.position + quad_uv * instance.size;

    // UV in atlas space
    let uv = instance.uv_offset + quad_uv * instance.uv_size;

    var output: VertexOutput;
    output.clip_position = uniforms.projection * vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    output.color = instance.color;
    output.position = position;

    return output;
}

// --- Fragment Shader (SDF Glyph Rendering) ---

@group(1) @binding(0)
var glyph_atlas: texture_2d<f32>;  // SDF glyph atlas (R8 or R16F)

@group(1) @binding(1)
var glyph_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample SDF distance from atlas (stored in red channel)
    let distance = textureSample(glyph_atlas, glyph_sampler, input.uv).r;

    // Convert distance to alpha (SDF threshold = 0.5 typical)
    let pixel_dist = (distance - uniforms.sdf_threshold) / uniforms.smoothing;
    let alpha = clamp(0.5 - pixel_dist, 0.0, 1.0);

    // Output color with alpha
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}

// --- Alternative: Outlined Text ---

@fragment
fn fs_outlined(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(glyph_atlas, glyph_sampler, input.uv).r;

    // Inner fill alpha
    let fill_pixel_dist = (distance - uniforms.sdf_threshold) / uniforms.smoothing;
    let fill_alpha = clamp(0.5 - fill_pixel_dist, 0.0, 1.0);

    // Outer outline alpha
    let outline_threshold = uniforms.sdf_threshold - uniforms.outline_width;
    let outline_pixel_dist = (distance - outline_threshold) / uniforms.smoothing;
    let outline_alpha = clamp(0.5 - outline_pixel_dist, 0.0, 1.0);

    // Mix fill and outline
    let outline_color = vec4<f32>(0.0, 0.0, 0.0, 1.0);  // Black outline
    let fill_color = input.color;

    let final_color = mix(outline_color, fill_color, fill_alpha);
    let final_alpha = max(fill_alpha, outline_alpha);

    return vec4<f32>(final_color.rgb, final_alpha * input.color.a);
}

// --- Alternative: Drop Shadow ---

@fragment
fn fs_shadow(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample main glyph
    let distance = textureSample(glyph_atlas, glyph_sampler, input.uv).r;
    let pixel_dist = (distance - uniforms.sdf_threshold) / uniforms.smoothing;
    let alpha = clamp(0.5 - pixel_dist, 0.0, 1.0);

    // Sample shadow (offset UV)
    let shadow_uv = input.uv + uniforms.shadow_offset / vec2<f32>(textureDimensions(glyph_atlas, 0));
    let shadow_distance = textureSample(glyph_atlas, glyph_sampler, shadow_uv).r;
    let shadow_pixel_dist = (shadow_distance - uniforms.sdf_threshold) / (uniforms.smoothing * 2.0);
    let shadow_alpha = clamp(0.5 - shadow_pixel_dist, 0.0, 1.0);

    // Composite shadow under text
    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, 0.5);  // Semi-transparent black
    let text_color = input.color;

    let final_color = mix(shadow_color, text_color, alpha);
    let final_alpha = max(alpha, shadow_alpha * 0.5);

    return vec4<f32>(final_color.rgb, final_alpha * input.color.a);
}

// --- Alternative: Multi-Channel SDF (Higher Quality) ---

@fragment
fn fs_msdf(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample multi-channel SDF (RGB channels encode median distance)
    let msd = textureSample(glyph_atlas, glyph_sampler, input.uv).rgb;

    // Median of RGB channels
    let median = max(min(msd.r, msd.g), min(max(msd.r, msd.g), msd.b));

    // Convert to alpha
    let pixel_dist = (median - 0.5) / uniforms.smoothing;
    let alpha = clamp(0.5 - pixel_dist, 0.0, 1.0);

    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}

// --- Alternative: Subpixel Rendering (RGB LCD) ---

@fragment
fn fs_subpixel(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample SDF at RGB subpixel offsets (1/3 pixel each)
    let pixel_width = 1.0 / f32(textureDimensions(glyph_atlas, 0).x);
    let subpixel_offset = pixel_width / 3.0;

    let distance_r = textureSample(glyph_atlas, glyph_sampler, input.uv + vec2<f32>(-subpixel_offset, 0.0)).r;
    let distance_g = textureSample(glyph_atlas, glyph_sampler, input.uv).r;
    let distance_b = textureSample(glyph_atlas, glyph_sampler, input.uv + vec2<f32>(subpixel_offset, 0.0)).r;

    // Convert each channel to alpha
    let alpha_r = clamp(0.5 - (distance_r - uniforms.sdf_threshold) / uniforms.smoothing, 0.0, 1.0);
    let alpha_g = clamp(0.5 - (distance_g - uniforms.sdf_threshold) / uniforms.smoothing, 0.0, 1.0);
    let alpha_b = clamp(0.5 - (distance_b - uniforms.sdf_threshold) / uniforms.smoothing, 0.0, 1.0);

    // Output subpixel RGB values (for LCD rendering)
    let color = input.color.rgb;
    return vec4<f32>(
        color.r * alpha_r,
        color.g * alpha_g,
        color.b * alpha_b,
        (alpha_r + alpha_g + alpha_b) / 3.0  // Average alpha
    );
}
