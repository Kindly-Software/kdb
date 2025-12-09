// shapes.wgsl - SDF-based shape rendering for kindly_dedup gui_v2
//
// COCA-compliant GPU shader for filled rects, rounded rects, and borders
// using Signed Distance Functions (SDF) for sub-pixel anti-aliasing.
//
// Architecture:
// - Vertex shader: Generate quad vertices from shape bounds
// - Fragment shader: SDF rendering with smooth edges
//
// Performance Target: <100ns per shape @ 60 FPS

// Vertex input (from ShapeInstance buffer)
struct ShapeInstance {
    // Rectangle bounds (Q16.16 fixed-point, converted to pixels)
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) width: f32,
    @location(3) height: f32,
    // Color (RGBA8 packed as vec4<f32> 0.0-1.0)
    @location(4) color: vec4<f32>,
    // Shape parameters
    @location(5) corner_radius: f32, // Pixels (0.0 = rect, >0.0 = rounded)
    @location(6) border_width: f32,  // Pixels (0.0 = filled, >0.0 = border only)
}

// Vertex output / fragment input
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>, // UV coords (0,0 = top-left, 1,1 = bottom-right)
    @location(2) size: vec2<f32>, // Shape size in pixels
    @location(3) corner_radius: f32,
    @location(4) border_width: f32,
}

// Push constants for screen dimensions (update per frame)
@group(0) @binding(0)
var<uniform> screen: vec2<f32>; // Screen width/height in pixels

// Vertex shader: Generate quad vertices (2 triangles, 6 vertices)
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: ShapeInstance,
) -> VertexOutput {
    var out: VertexOutput;

    // Quad vertices: 0=TL, 1=TR, 2=BL, 3=BL, 4=TR, 5=BR
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), // TL
        vec2<f32>(1.0, 0.0), // TR
        vec2<f32>(0.0, 1.0), // BL
        vec2<f32>(0.0, 1.0), // BL (second triangle)
        vec2<f32>(1.0, 0.0), // TR
        vec2<f32>(1.0, 1.0), // BR
    );

    let pos = positions[vertex_idx];

    // Calculate pixel position
    let pixel_x = instance.x + pos.x * instance.width;
    let pixel_y = instance.y + pos.y * instance.height;

    // Convert to NDC (-1 to 1)
    let ndc_x = (pixel_x / screen.x) * 2.0 - 1.0;
    let ndc_y = -((pixel_y / screen.y) * 2.0 - 1.0); // Flip Y (screen coords are top-down)

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.color;
    out.uv = pos;
    out.size = vec2<f32>(instance.width, instance.height);
    out.corner_radius = instance.corner_radius;
    out.border_width = instance.border_width;

    return out;
}

// SDF: Rounded rectangle
// Returns signed distance: negative = inside, positive = outside, 0 = edge
fn sdf_rounded_rect(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    // Offset to center
    let half_size = size * 0.5;
    let q = abs(p - half_size) - half_size + vec2<f32>(radius, radius);

    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - radius;
}

// Fragment shader: SDF rendering with anti-aliasing
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate pixel position within shape (0,0 = top-left corner)
    let pixel_pos = in.uv * in.size;

    // SDF distance
    let dist = sdf_rounded_rect(pixel_pos, in.size, in.corner_radius);

    // Anti-aliasing: smooth transition over 1 pixel
    let edge_softness = 1.0;

    // Determine alpha based on fill vs border
    var alpha: f32;

    if (in.border_width > 0.0) {
        // Border mode: only render pixels near edge
        let outer_dist = dist;
        let inner_dist = dist + in.border_width;

        // Alpha = 1.0 if inside border band, 0.0 otherwise
        alpha = smoothstep(edge_softness, -edge_softness, outer_dist)
              * smoothstep(-edge_softness, edge_softness, inner_dist);
    } else {
        // Fill mode: render all pixels inside shape
        alpha = smoothstep(edge_softness, -edge_softness, dist);
    }

    // Premultiply color by alpha
    return vec4<f32>(in.color.rgb * alpha, in.color.a * alpha);
}
