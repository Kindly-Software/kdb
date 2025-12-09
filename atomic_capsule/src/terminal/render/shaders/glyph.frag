#version 450

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

// Glyph atlas texture
layout(set = 0, binding = 1) uniform texture2D t_atlas;
layout(set = 0, binding = 2) uniform sampler s_atlas;

// Output
layout(location = 0) out vec4 o_color;

// Attribute bit flags
const uint ATTR_BOLD = 1u;
const uint ATTR_ITALIC = 2u;
const uint ATTR_UNDERLINE_SINGLE = 4u;
const uint ATTR_UNDERLINE_DOUBLE = 8u;
const uint ATTR_STRIKETHROUGH = 16u;
const uint ATTR_BLINK = 32u;
const uint ATTR_INVERSE = 64u;

// SDF rendering constants
const float SDF_THRESHOLD = 0.5;
const float SDF_SMOOTH = 0.05;

void main() {
    // Sample glyph from atlas (SDF texture)
    float glyph_distance = texture(sampler2D(t_atlas, s_atlas), v_uv).r;

    // Convert SDF to alpha with smooth edge
    float glyph_alpha = smoothstep(SDF_THRESHOLD - SDF_SMOOTH, SDF_THRESHOLD + SDF_SMOOTH, glyph_distance);

    // Bold: expand SDF threshold
    if ((v_attributes & ATTR_BOLD) != 0u) {
        glyph_alpha = smoothstep(SDF_THRESHOLD - SDF_SMOOTH - 0.05, SDF_THRESHOLD + SDF_SMOOTH - 0.05, glyph_distance);
    }

    // Handle inverse attribute
    vec4 fg = v_fg_color;
    vec4 bg = v_bg_color;
    if ((v_attributes & ATTR_INVERSE) != 0u) {
        fg = v_bg_color;
        bg = v_fg_color;
    }

    // Handle blink (simple on/off based on time, 1Hz)
    if ((v_attributes & ATTR_BLINK) != 0u) {
        float blink = step(0.5, fract(u_time * 1.0));
        glyph_alpha *= blink;
    }

    // Mix foreground and background based on glyph alpha
    vec4 base_color = mix(bg, fg, glyph_alpha);

    // Underline rendering (bottom 10% of cell)
    vec2 cell_uv = fract(v_uv * u_viewport_size / u_cell_size);
    float underline_y = 0.9; // 90% down the cell
    float underline_thickness = 0.05; // 5% of cell height

    if ((v_attributes & ATTR_UNDERLINE_SINGLE) != 0u) {
        float underline_dist = abs(cell_uv.y - underline_y);
        if (underline_dist < underline_thickness) {
            base_color = fg;
        }
    }

    if ((v_attributes & ATTR_UNDERLINE_DOUBLE) != 0u) {
        float underline1_y = 0.85;
        float underline2_y = 0.95;
        float underline_dist1 = abs(cell_uv.y - underline1_y);
        float underline_dist2 = abs(cell_uv.y - underline2_y);
        if (underline_dist1 < underline_thickness * 0.5 || underline_dist2 < underline_thickness * 0.5) {
            base_color = fg;
        }
    }

    // Strikethrough rendering (middle 50% of cell)
    if ((v_attributes & ATTR_STRIKETHROUGH) != 0u) {
        float strike_y = 0.5;
        float strike_thickness = 0.05;
        float strike_dist = abs(cell_uv.y - strike_y);
        if (strike_dist < strike_thickness) {
            base_color = fg;
        }
    }

    // Italic: apply shear to UVs (visual effect only, not actual skew)
    // This is a simple approximation - true italic would need modified atlas UVs
    if ((v_attributes & ATTR_ITALIC) != 0u) {
        // Slight brightness increase to simulate different font weight
        base_color.rgb = min(base_color.rgb * 1.05, vec3(1.0));
    }

    o_color = base_color;
}
