// KGPU GUI Signed Distance Field (SDF) Shape Rendering
//
// Purpose: High-quality 2D shape rendering using distance fields
// Stage: Vertex + Fragment
// Features: Circles, rectangles, rounded rectangles, triangles (analytical SDF)
//
// Performance Target:
// - SDF evaluation: <5 cycles/pixel (analytical math)
// - Fill rate: 2+ Gpixel/sec (complex shapes)
// - Anti-aliasing: Free (sub-pixel precision)
//
// Framework Compliance:
// - UCE34: T7 Heterogeneous (GPU SDF evaluation)
// - COCA: Immutable shader (compile-time SDF functions)
// - B32: <0.5ms per shape (1080p)
//
// References:
// - Inigo Quilez SDF functions: https://iquilezles.org/articles/distfunctions2d/
// - SDF rendering: https://www.shadertoy.com/view/4dfXDn

// --- Vertex Shader (Fullscreen Quad) ---

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,  // Normalized device coordinates (-1 to 1)
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate fullscreen quad from vertex index
    // Triangle strip: 0 (BL), 1 (BR), 2 (TL), 3 (TR)
    var uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),  // 0, 2, 0, 2 -> 0, 1, 0, 1
        f32(vertex_index & 2u)             // 0, 0, 2, 2 -> 0, 0, 1, 1
    );

    var output: VertexOutput;
    output.clip_position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);  // Map to clip space
    output.uv = uv * 2.0 - 1.0;  // Map to -1..1 for SDF evaluation
    return output;
}

// --- SDF Functions ---

// Circle SDF
fn sdf_circle(p: vec2<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

// Rectangle SDF (axis-aligned)
fn sdf_rectangle(p: vec2<f32>, size: vec2<f32>) -> f32 {
    let d = abs(p) - size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// Rounded rectangle SDF
fn sdf_rounded_rectangle(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p) - size + radius;
    return length(max(d, vec2<f32>(0.0))) - radius + min(max(d.x, d.y), 0.0);
}

// Triangle SDF (equilateral)
fn sdf_triangle(p: vec2<f32>, size: f32) -> f32 {
    let k = sqrt(3.0);
    let px = abs(p.x) - size;
    let py = p.y + size / k;

    if (px + k * py > 0.0) {
        let t = vec2<f32>(px - k * py, -k * px - py) / 2.0;
        return -length(t) * sign(t.x);
    }

    return -py - size / k;
}

// Hexagon SDF
fn sdf_hexagon(p: vec2<f32>, radius: f32) -> f32 {
    let k = vec3<f32>(-0.866025404, 0.5, 0.577350269);  // sqrt(3)/2, 1/2, 1/sqrt(3)
    var q = abs(p);
    q = vec2<f32>(q.x - 2.0 * min(dot(k.xy, q), 0.0) * k.x, q.y);
    q = vec2<f32>(q.x - clamp(q.x, -k.z * radius, k.z * radius), q.y - radius);
    return length(q) * sign(q.y);
}

// --- Fragment Shader ---

struct ShapeParams {
    shape_type: u32,         // 0=circle, 1=rect, 2=rounded_rect, 3=triangle, 4=hexagon
    size: vec2<f32>,         // Shape size (width, height)
    radius: f32,             // Circle radius OR rounded corner radius
    color: vec4<f32>,        // Shape color (RGBA)
    border_width: f32,       // Border width (0 = filled)
    border_color: vec4<f32>, // Border color
    position: vec2<f32>,     // Shape center position
    _padding: vec2<f32>,     // 16B alignment
}

@group(0) @binding(0)
var<uniform> shape: ShapeParams;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // Transform UV to shape-local coordinates
    let p = uv - shape.position;

    // Evaluate SDF based on shape type
    var dist: f32;
    if (shape.shape_type == 0u) {
        dist = sdf_circle(p, shape.radius);
    } else if (shape.shape_type == 1u) {
        dist = sdf_rectangle(p, shape.size);
    } else if (shape.shape_type == 2u) {
        dist = sdf_rounded_rectangle(p, shape.size, shape.radius);
    } else if (shape.shape_type == 3u) {
        dist = sdf_triangle(p, shape.size.x);
    } else if (shape.shape_type == 4u) {
        dist = sdf_hexagon(p, shape.radius);
    } else {
        dist = 1.0;  // Invalid shape type
    }

    // Anti-aliasing via smoothstep
    let pixel_size = length(vec2<f32>(dpdx(uv.x), dpdy(uv.y)));
    let smoothing = pixel_size * 1.5;  // 1.5 pixels for smooth edges

    // Fill or border rendering
    var color: vec4<f32>;
    if (shape.border_width > 0.0) {
        // Border: check if distance is within border range
        let border_inner = -shape.border_width;
        let border_outer = 0.0;

        let alpha_fill = 1.0 - smoothstep(-smoothing, smoothing, dist - border_inner);
        let alpha_border = 1.0 - smoothstep(-smoothing, smoothing, dist - border_outer);

        // Mix fill and border colors
        let fill_color = shape.color * alpha_fill;
        let border_only = shape.border_color * (alpha_border - alpha_fill);
        color = fill_color + border_only;
    } else {
        // Filled shape
        let alpha = 1.0 - smoothstep(-smoothing, smoothing, dist);
        color = shape.color * alpha;
    }

    return color;
}

// --- Alternative: Multi-Shape Batching ---

struct BatchedShape {
    position: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
    shape_type: u32,
    radius: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(1)
var<storage, read> shapes: array<BatchedShape>;

@fragment
fn fs_batched(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Render all shapes (painter's algorithm, back-to-front)
    for (var i = 0u; i < arrayLength(&shapes); i = i + 1u) {
        let s = shapes[i];
        let p = uv - s.position;

        var dist: f32;
        if (s.shape_type == 0u) {
            dist = sdf_circle(p, s.radius);
        } else if (s.shape_type == 1u) {
            dist = sdf_rectangle(p, s.size);
        } else if (s.shape_type == 2u) {
            dist = sdf_rounded_rectangle(p, s.size, s.radius);
        } else {
            continue;
        }

        let pixel_size = length(vec2<f32>(dpdx(uv.x), dpdy(uv.y)));
        let smoothing = pixel_size * 1.5;
        let alpha = 1.0 - smoothstep(-smoothing, smoothing, dist);

        // Alpha compositing (over operator)
        let shape_color = vec4<f32>(s.color.rgb * alpha, alpha);
        final_color = shape_color + final_color * (1.0 - alpha);
    }

    return final_color;
}
