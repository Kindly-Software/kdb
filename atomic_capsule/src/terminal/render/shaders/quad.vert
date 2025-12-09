#version 450

// Per-vertex attributes (quad corners)
layout(location = 0) in vec2 a_position;  // [-0.5, 0.5] normalized quad
layout(location = 1) in vec2 a_uv;        // [0, 1] texture coords

// Per-instance attributes (one per cell)
layout(location = 2) in vec4 a_cell_pos;   // x, y, width, height (pixels)
layout(location = 3) in vec4 a_uv_rect;    // u0, v0, u1, v1 (atlas coords)
layout(location = 4) in vec4 a_fg_color;   // RGBA normalized
layout(location = 5) in vec4 a_bg_color;   // RGBA normalized
layout(location = 6) in uint a_attributes; // bold(1)|italic(1)|underline(2)|strike(1)|...

// Uniforms
layout(set = 0, binding = 0) uniform Uniforms {
    mat4 u_projection;
    vec2 u_viewport_size;
    vec2 u_cell_size;
    float u_time;
    uint u_frame;
    vec2 _pad;
};

// Outputs to fragment shader
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_fg_color;
layout(location = 2) out vec4 v_bg_color;
layout(location = 3) flat out uint v_attributes;

void main() {
    // Transform quad to cell position
    vec2 pos = a_position * a_cell_pos.zw + a_cell_pos.xy;
    gl_Position = u_projection * vec4(pos, 0.0, 1.0);

    // Interpolate UVs within atlas region
    v_uv = mix(a_uv_rect.xy, a_uv_rect.zw, a_uv);

    // Pass through colors and attributes
    v_fg_color = a_fg_color;
    v_bg_color = a_bg_color;
    v_attributes = a_attributes;
}
