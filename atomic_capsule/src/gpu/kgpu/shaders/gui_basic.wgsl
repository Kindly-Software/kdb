// KGPU GUI Basic 2D Rendering Shader
//
// Purpose: Basic 2D rendering for GUI elements (rectangles, images, gradients)
// Stage: Vertex + Fragment
// Features: Position + Color + UV coordinates
//
// Performance Target:
// - Vertex throughput: 10M+ vertices/sec
// - Fill rate: 4+ Gpixel/sec (1080p @ 60 FPS)
//
// Framework Compliance:
// - UCE34: T7 Heterogeneous (GPU execution)
// - COCA: Immutable shader (compile-time verification)
// - B32: <1ms draw call overhead

// --- Vertex Shader ---

struct VertexInput {
    @location(0) position: vec2<f32>,  // Screen-space position (0-1)
    @location(1) color: vec4<f32>,     // Vertex color (RGBA)
    @location(2) uv: vec2<f32>,        // Texture coordinates (0-1)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

struct Uniforms {
    transform: mat4x4<f32>,   // Projection matrix (orthographic)
    time: f32,                 // Animation time (seconds)
    _padding: vec3<f32>,      // 16B alignment
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Transform position to clip space
    let position = vec4<f32>(input.position, 0.0, 1.0);
    output.clip_position = uniforms.transform * position;

    // Pass through color and UV
    output.color = input.color;
    output.uv = input.uv;

    return output;
}

// --- Fragment Shader ---

@group(1) @binding(0)
var tex: texture_2d<f32>;

@group(1) @binding(1)
var tex_sampler: sampler;

struct FragmentInput {
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {
    // Sample texture (or use white for solid color mode)
    let tex_color = textureSample(tex, tex_sampler, input.uv);

    // Multiply vertex color with texture color
    let final_color = input.color * tex_color;

    // Premultiplied alpha blending
    return final_color;
}

// --- Alternative: Solid Color (no texture sampling) ---

@fragment
fn fs_solid(input: FragmentInput) -> @location(0) vec4<f32> {
    // Use vertex color only (for rectangles, lines)
    return input.color;
}

// --- Alternative: Gradient (linear interpolation) ---

@fragment
fn fs_gradient(input: FragmentInput) -> @location(0) vec4<f32> {
    // Horizontal gradient using UV.x
    let color_start = vec4<f32>(0.2, 0.4, 0.8, 1.0);  // Blue
    let color_end = vec4<f32>(0.8, 0.4, 0.2, 1.0);    // Orange

    let gradient = mix(color_start, color_end, input.uv.x);
    return gradient * input.color;  // Multiply with vertex color for tinting
}
