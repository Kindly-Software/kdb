#version 450

// Advanced SDF Text Fragment Shader
// UCE34 Tier: T2 (SIMD-friendly SDF sampling) + T7 (GPU acceleration)
//
// Features:
// - Multi-channel SDF (MSDF) support for sharp corners
// - Configurable edge sharpness
// - Subpixel rendering (RGB LCD)
// - Outline and shadow support
// - Glow/emission effects

// Inputs from vertex shader
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_fg_color;
layout(location = 2) in vec4 v_bg_color;
layout(location = 3) flat in uint v_attributes;

// Uniforms
layout(set = 0, binding = 0) uniform Uniforms {
    mat4 u_projection;
    vec2 u_viewport_size;
    vec2 u_cell_size;
    float u_time;
    uint u_frame;
    vec2 _pad;
};

// SDF-specific uniforms
layout(set = 1, binding = 0) uniform SdfUniforms {
    float u_sdf_threshold;      // Edge threshold (default 0.5)
    float u_sdf_smoothness;     // Edge smoothness (default 0.05)
    float u_outline_width;      // Outline width in SDF units (0 = disabled)
    vec4 u_outline_color;       // Outline color RGBA
    float u_shadow_offset_x;    // Shadow X offset in UV units
    float u_shadow_offset_y;    // Shadow Y offset in UV units
    float u_shadow_softness;    // Shadow blur amount
    vec4 u_shadow_color;        // Shadow color RGBA
    float u_glow_intensity;     // Glow strength (0 = disabled)
    float u_glow_radius;        // Glow spread
    uint u_render_mode;         // 0=standard, 1=MSDF, 2=subpixel
    float u_subpixel_offset;    // Subpixel sample offset
};

// Glyph atlas texture (SDF or MSDF)
layout(set = 0, binding = 1) uniform texture2D t_atlas;
layout(set = 0, binding = 2) uniform sampler s_atlas;

// Output
layout(location = 0) out vec4 o_color;

// Attribute bit flags (must match Rust)
const uint ATTR_BOLD = 1u;
const uint ATTR_ITALIC = 2u;
const uint ATTR_UNDERLINE_SINGLE = 4u;
const uint ATTR_UNDERLINE_DOUBLE = 8u;
const uint ATTR_STRIKETHROUGH = 16u;
const uint ATTR_BLINK = 32u;
const uint ATTR_INVERSE = 64u;
const uint ATTR_DIM = 128u;

// Render modes
const uint MODE_STANDARD = 0u;
const uint MODE_MSDF = 1u;
const uint MODE_SUBPIXEL = 2u;

// Sample SDF and convert to alpha with configurable smoothness
float sdf_to_alpha(float distance, float threshold, float smoothness) {
    return smoothstep(threshold - smoothness, threshold + smoothness, distance);
}

// Multi-channel SDF median for sharp corners
float msdf_median(vec3 msd) {
    return max(min(msd.r, msd.g), min(max(msd.r, msd.g), msd.b));
}

// Sample SDF with automatic derivative-based smoothing
float sample_sdf_auto_smooth(vec2 uv) {
    float distance = texture(sampler2D(t_atlas, s_atlas), uv).r;

    // Compute gradient for screen-space smoothing
    vec2 grad = vec2(dFdx(distance), dFdy(distance));
    float grad_mag = length(grad);

    // Adapt smoothness to screen-space gradient
    float adaptive_smooth = max(u_sdf_smoothness, grad_mag * 0.5);

    return sdf_to_alpha(distance, u_sdf_threshold, adaptive_smooth);
}

// MSDF sampling for sharp corners at any scale
float sample_msdf(vec2 uv) {
    vec3 msd = texture(sampler2D(t_atlas, s_atlas), uv).rgb;
    float sd = msdf_median(msd);

    // Screen-space derivative for anti-aliasing
    float screen_px_dist = fwidth(sd);
    float alpha = smoothstep(0.5 - screen_px_dist, 0.5 + screen_px_dist, sd);

    return alpha;
}

// Subpixel rendering (RGB LCD horizontal)
vec3 sample_subpixel(vec2 uv) {
    // Sample at 3 horizontal offsets for R, G, B subpixels
    float offset = u_subpixel_offset / u_viewport_size.x;

    float r = texture(sampler2D(t_atlas, s_atlas), uv - vec2(offset, 0.0)).r;
    float g = texture(sampler2D(t_atlas, s_atlas), uv).r;
    float b = texture(sampler2D(t_atlas, s_atlas), uv + vec2(offset, 0.0)).r;

    // Convert to alpha with smoothing
    vec3 alpha;
    alpha.r = sdf_to_alpha(r, u_sdf_threshold, u_sdf_smoothness);
    alpha.g = sdf_to_alpha(g, u_sdf_threshold, u_sdf_smoothness);
    alpha.b = sdf_to_alpha(b, u_sdf_threshold, u_sdf_smoothness);

    return alpha;
}

// Sample shadow with offset and blur
float sample_shadow(vec2 uv) {
    vec2 shadow_uv = uv - vec2(u_shadow_offset_x, u_shadow_offset_y);
    float distance = texture(sampler2D(t_atlas, s_atlas), shadow_uv).r;

    // Shadow uses softer threshold
    float shadow_threshold = u_sdf_threshold - u_shadow_softness;
    return sdf_to_alpha(distance, shadow_threshold, u_sdf_smoothness + u_shadow_softness);
}

// Compute glow effect
float compute_glow(float glyph_alpha) {
    // Extend glyph alpha for glow
    float glow = pow(glyph_alpha, 0.5) * u_glow_intensity;
    return clamp(glow * u_glow_radius, 0.0, 1.0);
}

void main() {
    // Handle colors (with inverse attribute)
    vec4 fg = v_fg_color;
    vec4 bg = v_bg_color;
    if ((v_attributes & ATTR_INVERSE) != 0u) {
        fg = v_bg_color;
        bg = v_fg_color;
    }

    // Handle dim attribute
    if ((v_attributes & ATTR_DIM) != 0u) {
        fg.rgb *= 0.6;
    }

    // Bold: thicken text by adjusting threshold
    float threshold = u_sdf_threshold;
    if ((v_attributes & ATTR_BOLD) != 0u) {
        threshold -= 0.05; // Expand glyph
    }

    // Sample glyph based on render mode
    float glyph_alpha = 0.0;
    vec3 subpixel_alpha = vec3(0.0);

    if (u_render_mode == MODE_MSDF) {
        glyph_alpha = sample_msdf(v_uv);
    } else if (u_render_mode == MODE_SUBPIXEL) {
        subpixel_alpha = sample_subpixel(v_uv);
        glyph_alpha = (subpixel_alpha.r + subpixel_alpha.g + subpixel_alpha.b) / 3.0;
    } else {
        // Standard SDF with auto-smoothing
        float distance = texture(sampler2D(t_atlas, s_atlas), v_uv).r;
        glyph_alpha = sdf_to_alpha(distance, threshold, u_sdf_smoothness);
    }

    // Handle blink (1Hz oscillation)
    if ((v_attributes & ATTR_BLINK) != 0u) {
        float blink = step(0.5, fract(u_time));
        glyph_alpha *= blink;
        subpixel_alpha *= blink;
    }

    // Start with background
    vec4 color = bg;

    // Layer 1: Shadow (if enabled)
    if (u_shadow_color.a > 0.0 && (u_shadow_offset_x != 0.0 || u_shadow_offset_y != 0.0)) {
        float shadow_alpha = sample_shadow(v_uv);
        color = mix(color, u_shadow_color, shadow_alpha * u_shadow_color.a);
    }

    // Layer 2: Glow (if enabled)
    if (u_glow_intensity > 0.0) {
        float glow = compute_glow(glyph_alpha);
        color = mix(color, fg, glow * 0.5);
    }

    // Layer 3: Outline (if enabled)
    if (u_outline_width > 0.0) {
        float distance = texture(sampler2D(t_atlas, s_atlas), v_uv).r;
        float outline_threshold = threshold + u_outline_width;
        float outline_alpha = sdf_to_alpha(distance, outline_threshold, u_sdf_smoothness);
        float inner_alpha = sdf_to_alpha(distance, threshold, u_sdf_smoothness);
        float outline_only = outline_alpha - inner_alpha;
        color = mix(color, u_outline_color, outline_only * u_outline_color.a);
    }

    // Layer 4: Glyph foreground
    if (u_render_mode == MODE_SUBPIXEL) {
        // Per-channel blending for subpixel rendering
        color.r = mix(color.r, fg.r, subpixel_alpha.r);
        color.g = mix(color.g, fg.g, subpixel_alpha.g);
        color.b = mix(color.b, fg.b, subpixel_alpha.b);
        color.a = mix(color.a, fg.a, glyph_alpha);
    } else {
        color = mix(color, fg, glyph_alpha);
    }

    // Underline rendering (bottom 10% of cell)
    vec2 cell_uv = fract(v_uv * u_viewport_size / u_cell_size);
    float underline_y = 0.9;
    float underline_thickness = 0.05;

    if ((v_attributes & ATTR_UNDERLINE_SINGLE) != 0u) {
        float underline_dist = abs(cell_uv.y - underline_y);
        float underline_alpha = 1.0 - smoothstep(0.0, underline_thickness, underline_dist);
        color = mix(color, fg, underline_alpha);
    }

    if ((v_attributes & ATTR_UNDERLINE_DOUBLE) != 0u) {
        float underline1_y = 0.85;
        float underline2_y = 0.95;
        float underline_dist1 = abs(cell_uv.y - underline1_y);
        float underline_dist2 = abs(cell_uv.y - underline2_y);
        float underline_alpha1 = 1.0 - smoothstep(0.0, underline_thickness * 0.5, underline_dist1);
        float underline_alpha2 = 1.0 - smoothstep(0.0, underline_thickness * 0.5, underline_dist2);
        color = mix(color, fg, max(underline_alpha1, underline_alpha2));
    }

    // Strikethrough (middle of cell)
    if ((v_attributes & ATTR_STRIKETHROUGH) != 0u) {
        float strike_dist = abs(cell_uv.y - 0.5);
        float strike_alpha = 1.0 - smoothstep(0.0, underline_thickness, strike_dist);
        color = mix(color, fg, strike_alpha);
    }

    o_color = color;
}
