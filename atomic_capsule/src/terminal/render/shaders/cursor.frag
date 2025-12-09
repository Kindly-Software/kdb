#version 450

// Cursor Rendering Fragment Shader
// UCE34 Tier: T1 (Atomic state) + T7 (GPU rendering)
//
// Features:
// - Multiple cursor styles: block, underline, bar (I-beam)
// - Smooth blinking animation with configurable frequency
// - Color inversion mode for visibility
// - Hollow block cursor variant
// - Rounded corners option
// - Cursor trail effect (optional)

// Inputs from vertex shader
layout(location = 0) in vec2 v_uv;           // [0,1] within cursor quad
layout(location = 1) in vec4 v_cursor_color; // Cursor color RGBA
layout(location = 2) in vec4 v_cell_color;   // Cell foreground color (for inversion)
layout(location = 3) in vec4 v_bg_color;     // Cell background color

// Uniforms
layout(set = 0, binding = 0) uniform CursorUniforms {
    vec2 u_cell_size;           // Cell dimensions in pixels
    float u_time;               // Animation time
    uint u_frame;               // Frame counter
    uint u_cursor_style;        // 0=block, 1=underline, 2=bar, 3=hollow_block
    float u_blink_rate;         // Blinks per second (0 = no blink)
    float u_blink_duty;         // On-time fraction (0.5 = 50% on)
    uint u_flags;               // Feature flags bitfield
    float u_corner_radius;      // Rounded corner radius (0 = sharp)
    float u_border_width;       // Hollow block border width
    float u_bar_width;          // Bar cursor width fraction (default 0.1)
    float u_underline_height;   // Underline height fraction (default 0.15)
    float u_trail_intensity;    // Cursor trail effect (0 = disabled)
};

// Output
layout(location = 0) out vec4 o_color;

// Cursor styles
const uint STYLE_BLOCK = 0u;
const uint STYLE_UNDERLINE = 1u;
const uint STYLE_BAR = 2u;
const uint STYLE_HOLLOW_BLOCK = 3u;

// Feature flags
const uint FLAG_INVERT_COLOR = 1u;      // Invert cursor color
const uint FLAG_SMOOTH_BLINK = 2u;      // Smooth fade vs hard blink
const uint FLAG_ROUNDED_CORNERS = 4u;   // Enable rounded corners
const uint FLAG_TRAIL_EFFECT = 8u;      // Enable cursor trail
const uint FLAG_GLOW = 16u;             // Enable cursor glow

// Signed distance function for rounded rectangle
float sdf_rounded_rect(vec2 p, vec2 size, float radius) {
    vec2 q = abs(p) - size + radius;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
}

// Compute blink visibility
float compute_blink(float time, float rate, float duty, bool smooth) {
    if (rate <= 0.0) return 1.0;

    float phase = fract(time * rate);

    if (smooth) {
        // Smooth sine-wave blink
        float t = phase * 6.28318; // 2*PI
        return (cos(t) + 1.0) * 0.5;
    } else {
        // Hard step blink
        return step(phase, duty);
    }
}

// Block cursor (full cell coverage)
float cursor_block(vec2 uv, float corner_radius) {
    if (corner_radius > 0.0) {
        vec2 p = uv - 0.5;
        float d = sdf_rounded_rect(p, vec2(0.5 - corner_radius), corner_radius);
        return 1.0 - smoothstep(-0.01, 0.01, d);
    }
    return 1.0; // Full coverage
}

// Hollow block cursor (border only)
float cursor_hollow_block(vec2 uv, float border_width, float corner_radius) {
    vec2 p = uv - 0.5;

    // Outer boundary
    float outer = sdf_rounded_rect(p, vec2(0.5), corner_radius);

    // Inner boundary
    float inner_size = 0.5 - border_width;
    float inner = sdf_rounded_rect(p, vec2(inner_size), max(0.0, corner_radius - border_width));

    // Border is outer - inner
    float outer_alpha = 1.0 - smoothstep(-0.01, 0.01, outer);
    float inner_alpha = 1.0 - smoothstep(-0.01, 0.01, inner);

    return outer_alpha - inner_alpha;
}

// Underline cursor (bottom of cell)
float cursor_underline(vec2 uv, float height) {
    float y_start = 1.0 - height;
    return smoothstep(y_start - 0.01, y_start, uv.y);
}

// Bar (I-beam) cursor (left edge of cell)
float cursor_bar(vec2 uv, float width) {
    return 1.0 - smoothstep(width - 0.01, width, uv.x);
}

// Compute glow effect around cursor
float compute_glow(float cursor_alpha, vec2 uv) {
    // Simple radial glow based on distance from center
    vec2 center = vec2(0.5);
    float dist = length(uv - center);
    float glow = exp(-dist * 4.0) * cursor_alpha;
    return glow * 0.3;
}

// Trail effect (motion blur simulation)
float compute_trail(vec2 uv, float time) {
    // Simulate horizontal trail
    float trail_x = uv.x + sin(time * 10.0) * 0.1;
    float trail = exp(-abs(trail_x - 0.5) * 5.0);
    return trail * 0.2;
}

void main() {
    // Compute cursor shape alpha
    float cursor_alpha = 0.0;
    float corner_radius = ((u_flags & FLAG_ROUNDED_CORNERS) != 0u) ? u_corner_radius : 0.0;

    switch (u_cursor_style) {
        case STYLE_BLOCK:
            cursor_alpha = cursor_block(v_uv, corner_radius);
            break;

        case STYLE_UNDERLINE:
            cursor_alpha = cursor_underline(v_uv, u_underline_height);
            break;

        case STYLE_BAR:
            cursor_alpha = cursor_bar(v_uv, u_bar_width);
            break;

        case STYLE_HOLLOW_BLOCK:
            cursor_alpha = cursor_hollow_block(v_uv, u_border_width, corner_radius);
            break;

        default:
            cursor_alpha = cursor_block(v_uv, corner_radius);
            break;
    }

    // Apply blinking
    bool smooth_blink = (u_flags & FLAG_SMOOTH_BLINK) != 0u;
    float blink = compute_blink(u_time, u_blink_rate, u_blink_duty, smooth_blink);
    cursor_alpha *= blink;

    // Determine cursor color (with optional inversion)
    vec4 cursor_color = v_cursor_color;
    vec4 base_color = v_bg_color;

    if ((u_flags & FLAG_INVERT_COLOR) != 0u) {
        // Invert: cursor takes background color, text shows through inverted
        cursor_color = v_bg_color;
        // When cursor is visible, invert the underlying content
        cursor_color.rgb = vec3(1.0) - v_cell_color.rgb;
    }

    // Start with background
    vec4 color = base_color;

    // Apply glow effect (behind cursor)
    if ((u_flags & FLAG_GLOW) != 0u && cursor_alpha > 0.0) {
        float glow = compute_glow(cursor_alpha, v_uv);
        color = mix(color, cursor_color, glow);
    }

    // Apply trail effect
    if ((u_flags & FLAG_TRAIL_EFFECT) != 0u && u_trail_intensity > 0.0) {
        float trail = compute_trail(v_uv, u_time) * u_trail_intensity;
        color = mix(color, cursor_color, trail * cursor_alpha);
    }

    // Apply cursor
    color = mix(color, cursor_color, cursor_alpha);

    // Ensure alpha is appropriate
    color.a = max(color.a, cursor_alpha);

    o_color = color;
}
