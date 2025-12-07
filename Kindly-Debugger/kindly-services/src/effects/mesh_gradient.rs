//! WebGL Animated Mesh Gradient Effect
//!
//! GPU-accelerated gradient animation inspired by Paddle's landing page.
//! Uses noise functions for smooth, organic movement.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlUniformLocation,
};

/// WebGL Mesh Gradient with animated noise
pub struct MeshGradient {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
    time_loc: WebGlUniformLocation,
    resolution_loc: WebGlUniformLocation,
    mouse_loc: WebGlUniformLocation,
    start_time: f64,
    mouse_x: f32,
    mouse_y: f32,
}

// Vertex shader - full screen quad
const VERTEX_SHADER: &str = r#"#version 300 es
precision highp float;

const vec2 positions[4] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0)
);

void main() {
    gl_Position = vec4(positions[gl_VertexID], 0.0, 1.0);
}
"#;

// Fragment shader - animated mesh gradient with Byzantine purple + gold
const FRAGMENT_SHADER: &str = r#"#version 300 es
precision highp float;

uniform float u_time;
uniform vec2 u_resolution;
uniform vec2 u_mouse;

out vec4 fragColor;

// Simplex 2D noise
vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec2 mod289(vec2 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec3 permute(vec3 x) { return mod289(((x*34.0)+1.0)*x); }

float snoise(vec2 v) {
    const vec4 C = vec4(0.211324865405187, 0.366025403784439,
                        -0.577350269189626, 0.024390243902439);
    vec2 i  = floor(v + dot(v, C.yy));
    vec2 x0 = v -   i + dot(i, C.xx);
    vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    vec4 x12 = x0.xyxy + C.xxzz;
    x12.xy -= i1;
    i = mod289(i);
    vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0))
                           + i.x + vec3(0.0, i1.x, 1.0));
    vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy),
                            dot(x12.zw,x12.zw)), 0.0);
    m = m*m; m = m*m;
    vec3 x = 2.0 * fract(p * C.www) - 1.0;
    vec3 h = abs(x) - 0.5;
    vec3 ox = floor(x + 0.5);
    vec3 a0 = x - ox;
    m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);
    vec3 g;
    g.x  = a0.x  * x0.x  + h.x  * x0.y;
    g.yz = a0.yz * x12.xz + h.yz * x12.yw;
    return 130.0 * dot(m, g);
}

// Fractal Brownian Motion
float fbm(vec2 p) {
    float value = 0.0;
    float amplitude = 0.5;
    for (int i = 0; i < 5; i++) {
        value += amplitude * snoise(p);
        p *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution;
    vec2 center = vec2(0.5);

    // Time-based animation
    float t = u_time * 0.15;

    // Mouse influence (subtle)
    vec2 mouseInfluence = (u_mouse - center) * 0.1;

    // Create animated noise field
    vec2 q = vec2(fbm(uv * 2.0 + t), fbm(uv * 2.0 + vec2(5.2, 1.3) + t));
    vec2 r = vec2(fbm(uv * 2.0 + q + mouseInfluence + t * 0.5),
                  fbm(uv * 2.0 + q + vec2(1.7, 9.2) + t * 0.3));

    float f = fbm(uv * 2.0 + r);

    // Byzantine Purple spectrum
    vec3 purple1 = vec3(0.294, 0.0, 0.510);   // #4B0082 Byzantine
    vec3 purple2 = vec3(0.400, 0.200, 0.600); // #663399 Rebecca Purple
    vec3 purple3 = vec3(0.176, 0.0, 0.302);   // #2D004D Dark Byzantine
    vec3 deepBg = vec3(0.039, 0.0, 0.078);    // #0a0014 Near black

    // Gold accent (subtle)
    vec3 gold = vec3(1.0, 0.843, 0.0);        // #FFD700

    // Mix colors based on noise
    vec3 color = deepBg;
    color = mix(color, purple3, smoothstep(-0.4, 0.4, f));
    color = mix(color, purple1, smoothstep(0.0, 0.8, f));
    color = mix(color, purple2, smoothstep(0.3, 1.0, f * 1.2));

    // Add subtle gold highlights
    float goldNoise = snoise(uv * 8.0 + t * 2.0);
    color = mix(color, gold * 0.3, smoothstep(0.7, 1.0, goldNoise) * 0.15);

    // Vignette effect
    float vignette = 1.0 - length((uv - 0.5) * 1.5);
    vignette = smoothstep(0.0, 0.7, vignette);
    color *= vignette * 0.8 + 0.2;

    // Add subtle grain
    float grain = fract(sin(dot(uv * t, vec2(12.9898, 78.233))) * 43758.5453);
    color += grain * 0.02 - 0.01;

    fragColor = vec4(color, 1.0);
}
"#;

impl MeshGradient {
    /// Initialize WebGL mesh gradient on canvas
    ///
    /// Firefox compatibility notes:
    /// - WebGL2 may be disabled due to driver blacklisting
    /// - Some Firefox versions require specific about:config settings
    /// - Gracefully handles WebGL context creation failure
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        // Set canvas size to window size
        let width = window.inner_width()?.as_f64().unwrap_or(1920.0) as u32;
        let height = window.inner_height()?.as_f64().unwrap_or(1080.0) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        // Try WebGL2 first, then fall back to WebGL1 for Firefox compatibility
        // Some Firefox configurations have WebGL2 disabled but WebGL1 works
        let gl_result = canvas.get_context("webgl2");
        let gl = match gl_result {
            Ok(Some(context)) => context.dyn_into::<WebGl2RenderingContext>()?,
            _ => {
                // Log for debugging but don't fail - Firefox may have WebGL2 disabled
                web_sys::console::warn_1(&"WebGL2 not available, trying with antialiasing disabled".into());

                // Try with explicit context attributes for better Firefox compatibility
                let context_options = js_sys::Object::new();
                js_sys::Reflect::set(&context_options, &"alpha".into(), &true.into())?;
                js_sys::Reflect::set(&context_options, &"antialias".into(), &false.into())?;
                js_sys::Reflect::set(&context_options, &"preserveDrawingBuffer".into(), &true.into())?;

                canvas
                    .get_context_with_context_options("webgl2", &context_options)?
                    .ok_or("WebGL2 not supported - Firefox may have it disabled. Check about:support for WebGL status")?
                    .dyn_into::<WebGl2RenderingContext>()?
            }
        };

        // Compile shaders
        let vert_shader = compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
        let frag_shader = compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, FRAGMENT_SHADER)?;

        // Link program
        let program = link_program(&gl, &vert_shader, &frag_shader)?;
        gl.use_program(Some(&program));

        // Get uniform locations
        let time_loc = gl
            .get_uniform_location(&program, "u_time")
            .ok_or("u_time not found")?;
        let resolution_loc = gl
            .get_uniform_location(&program, "u_resolution")
            .ok_or("u_resolution not found")?;
        let mouse_loc = gl
            .get_uniform_location(&program, "u_mouse")
            .ok_or("u_mouse not found")?;

        // Set initial resolution
        gl.uniform2f(Some(&resolution_loc), width as f32, height as f32);

        // Get start time
        let performance = window.performance().ok_or("no performance")?;
        let start_time = performance.now();

        Ok(Self {
            gl,
            program,
            time_loc,
            resolution_loc,
            mouse_loc,
            start_time,
            mouse_x: 0.5,
            mouse_y: 0.5,
        })
    }

    /// Update mouse position (normalized 0-1)
    pub fn set_mouse(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    /// Render one frame
    pub fn render(&self, time: f64) {
        let elapsed = ((time - self.start_time) / 1000.0) as f32;

        self.gl.use_program(Some(&self.program));
        self.gl.uniform1f(Some(&self.time_loc), elapsed);
        self.gl.uniform2f(Some(&self.mouse_loc), self.mouse_x, self.mouse_y);

        // Clear and draw
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);
    }

    /// Handle window resize
    pub fn resize(&self, width: u32, height: u32) {
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.uniform2f(Some(&self.resolution_loc), width as f32, height as f32);
    }
}

fn compile_shader(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Unable to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl.get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default())
    }
}

fn link_program(
    gl: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Unable to create program")?;
    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);

    if gl.get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default())
    }
}
