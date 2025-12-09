//! WebGL Animated Mesh Gradient Effect
//!
//! GPU-accelerated gradient animation inspired by Paddle's landing page.
//! Uses noise functions for smooth, organic movement.
//!
//! Fallback chain: WebGL2 -> WebGL1 -> Canvas2D -> static CSS gradient

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, WebGl2RenderingContext, WebGlProgram,
    WebGlRenderingContext, WebGlShader, WebGlUniformLocation,
};

/// Rendering backend type
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    WebGl2,
    WebGl1,
    Canvas2D,
}

/// WebGL2 rendering context and resources
struct WebGl2Resources {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
    time_loc: WebGlUniformLocation,
    resolution_loc: WebGlUniformLocation,
    mouse_loc: WebGlUniformLocation,
}

/// WebGL1 rendering context and resources
struct WebGl1Resources {
    gl: WebGlRenderingContext,
    program: WebGlProgram,
    time_loc: WebGlUniformLocation,
    resolution_loc: WebGlUniformLocation,
    mouse_loc: WebGlUniformLocation,
}

/// Canvas 2D fallback rendering context
struct Canvas2DResources {
    ctx: CanvasRenderingContext2d,
}

/// Rendering context enum
enum RenderContext {
    WebGl2(WebGl2Resources),
    WebGl1(WebGl1Resources),
    Canvas2D(Canvas2DResources),
}

/// WebGL Mesh Gradient with animated noise and fallback chain
pub struct MeshGradient {
    canvas: HtmlCanvasElement,
    context: RenderContext,
    start_time: f64,
    mouse_x: f32,
    mouse_y: f32,
    /// Closures for context loss events (kept alive)
    _context_lost_closure: Option<Closure<dyn FnMut(web_sys::WebGlContextEvent)>>,
    _context_restored_closure: Option<Closure<dyn FnMut(web_sys::WebGlContextEvent)>>,
}

// Vertex shader - full screen quad (WebGL2)
const VERTEX_SHADER_GL2: &str = r#"#version 300 es
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

// Vertex shader - full screen quad (WebGL1 - needs attribute)
const VERTEX_SHADER_GL1: &str = r#"
precision highp float;

attribute vec2 a_position;

void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
}
"#;

// Fragment shader - animated mesh gradient with Byzantine purple + gold (WebGL2)
const FRAGMENT_SHADER_GL2: &str = r#"#version 300 es
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

// Simplified fragment shader for mobile (WebGL2) - fewer FBM iterations
const FRAGMENT_SHADER_GL2_MOBILE: &str = r#"#version 300 es
precision mediump float;

uniform float u_time;
uniform vec2 u_resolution;
uniform vec2 u_mouse;

out vec4 fragColor;

// Simplex 2D noise (simplified)
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

// Simplified FBM for mobile (2 iterations instead of 5)
float fbm(vec2 p) {
    float value = 0.0;
    float amplitude = 0.5;
    for (int i = 0; i < 2; i++) {
        value += amplitude * snoise(p);
        p *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution;

    // Time-based animation (slower for mobile)
    float t = u_time * 0.1;

    // Simplified noise field (single layer)
    float f = fbm(uv * 2.0 + t);

    // Byzantine Purple spectrum
    vec3 purple1 = vec3(0.294, 0.0, 0.510);   // #4B0082 Byzantine
    vec3 purple3 = vec3(0.176, 0.0, 0.302);   // #2D004D Dark Byzantine
    vec3 deepBg = vec3(0.039, 0.0, 0.078);    // #0a0014 Near black

    // Mix colors based on noise
    vec3 color = deepBg;
    color = mix(color, purple3, smoothstep(-0.4, 0.4, f));
    color = mix(color, purple1, smoothstep(0.0, 0.8, f));

    // Simple vignette
    float vignette = 1.0 - length((uv - 0.5) * 1.5);
    vignette = smoothstep(0.0, 0.7, vignette);
    color *= vignette * 0.8 + 0.2;

    fragColor = vec4(color, 1.0);
}
"#;

// Fragment shader for WebGL1 (no version directive, uses gl_FragColor)
const FRAGMENT_SHADER_GL1: &str = r#"
precision mediump float;

uniform float u_time;
uniform vec2 u_resolution;
uniform vec2 u_mouse;

// Simplex 2D noise (simplified for WebGL1)
vec3 mod289_3(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec2 mod289_2(vec2 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec3 permute(vec3 x) { return mod289_3(((x*34.0)+1.0)*x); }

float snoise(vec2 v) {
    vec4 C = vec4(0.211324865405187, 0.366025403784439,
                  -0.577350269189626, 0.024390243902439);
    vec2 i  = floor(v + dot(v, C.yy));
    vec2 x0 = v -   i + dot(i, C.xx);
    vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    vec4 x12 = x0.xyxy + C.xxzz;
    x12.xy -= i1;
    i = mod289_2(i);
    vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0))
                           + i.x + vec3(0.0, i1.x, 1.0));
    vec3 m = max(vec3(0.5) - vec3(dot(x0,x0), dot(x12.xy,x12.xy),
                            dot(x12.zw,x12.zw)), vec3(0.0));
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

// Simplified FBM (2 iterations for WebGL1)
float fbm(vec2 p) {
    float value = 0.0;
    float amplitude = 0.5;
    for (int i = 0; i < 2; i++) {
        value += amplitude * snoise(p);
        p *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution;

    float t = u_time * 0.1;
    float f = fbm(uv * 2.0 + t);

    // Byzantine Purple spectrum
    vec3 purple1 = vec3(0.294, 0.0, 0.510);
    vec3 purple3 = vec3(0.176, 0.0, 0.302);
    vec3 deepBg = vec3(0.039, 0.0, 0.078);

    vec3 color = deepBg;
    color = mix(color, purple3, smoothstep(-0.4, 0.4, f));
    color = mix(color, purple1, smoothstep(0.0, 0.8, f));

    float vignette = 1.0 - length((uv - 0.5) * 1.5);
    vignette = smoothstep(0.0, 0.7, vignette);
    color *= vignette * 0.8 + 0.2;

    gl_FragColor = vec4(color, 1.0);
}
"#;

impl MeshGradient {
    /// Initialize mesh gradient with fallback chain: WebGL2 -> WebGL1 -> Canvas2D
    ///
    /// Returns Err only if ALL backends fail (including Canvas2D).
    /// In that case, caller should fall back to static CSS gradient.
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        // Detect mobile via viewport width
        let width = window.inner_width()?.as_f64().unwrap_or(1920.0) as u32;
        let height = window.inner_height()?.as_f64().unwrap_or(1080.0) as u32;
        let is_mobile = width < 768;

        canvas.set_width(width);
        canvas.set_height(height);

        // Get start time
        let performance = window.performance().ok_or("no performance")?;
        let start_time = performance.now();

        // Try WebGL2 first
        if let Ok(resources) = Self::try_webgl2(&canvas, width, height, is_mobile) {
            web_sys::console::log_1(&"MeshGradient: Using WebGL2 backend".into());

            let mut gradient = Self {
                canvas,
                context: RenderContext::WebGl2(resources),
                start_time,
                mouse_x: 0.5,
                mouse_y: 0.5,
                _context_lost_closure: None,
                _context_restored_closure: None,
            };

            // Set up context loss handlers
            gradient.setup_context_loss_handlers()?;

            return Ok(gradient);
        }

        web_sys::console::warn_1(&"MeshGradient: WebGL2 failed, trying WebGL1".into());

        // Try WebGL1 as fallback
        if let Ok(resources) = Self::try_webgl1(&canvas, width, height) {
            web_sys::console::log_1(&"MeshGradient: Using WebGL1 backend".into());

            let mut gradient = Self {
                canvas,
                context: RenderContext::WebGl1(resources),
                start_time,
                mouse_x: 0.5,
                mouse_y: 0.5,
                _context_lost_closure: None,
                _context_restored_closure: None,
            };

            // Set up context loss handlers
            gradient.setup_context_loss_handlers()?;

            return Ok(gradient);
        }

        web_sys::console::warn_1(&"MeshGradient: WebGL1 failed, trying Canvas2D".into());

        // Try Canvas2D as final fallback
        if let Ok(resources) = Self::try_canvas2d(&canvas) {
            web_sys::console::log_1(&"MeshGradient: Using Canvas2D backend".into());

            return Ok(Self {
                canvas,
                context: RenderContext::Canvas2D(resources),
                start_time,
                mouse_x: 0.5,
                mouse_y: 0.5,
                _context_lost_closure: None,
                _context_restored_closure: None,
            });
        }

        // All backends failed
        Err("All rendering backends failed (WebGL2, WebGL1, Canvas2D)".into())
    }

    /// Try to initialize WebGL2 context
    fn try_webgl2(
        canvas: &HtmlCanvasElement,
        width: u32,
        height: u32,
        is_mobile: bool,
    ) -> Result<WebGl2Resources, JsValue> {
        // Try with low-power preference for mobile
        let context_options = js_sys::Object::new();
        js_sys::Reflect::set(&context_options, &"alpha".into(), &true.into())?;
        js_sys::Reflect::set(&context_options, &"antialias".into(), &(!is_mobile).into())?;
        js_sys::Reflect::set(
            &context_options,
            &"preserveDrawingBuffer".into(),
            &false.into(),
        )?;
        js_sys::Reflect::set(&context_options, &"failIfMajorPerformanceCaveat".into(), &false.into())?;

        if is_mobile {
            js_sys::Reflect::set(&context_options, &"powerPreference".into(), &"low-power".into())?;
        }

        let gl = canvas
            .get_context_with_context_options("webgl2", &context_options)?
            .ok_or("WebGL2 not supported")?
            .dyn_into::<WebGl2RenderingContext>()?;

        // Choose shader based on mobile detection
        let frag_source = if is_mobile {
            FRAGMENT_SHADER_GL2_MOBILE
        } else {
            FRAGMENT_SHADER_GL2
        };

        // Compile shaders
        let vert_shader = compile_shader_gl2(&gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER_GL2)?;
        let frag_shader = compile_shader_gl2(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, frag_source)?;

        // Link program
        let program = link_program_gl2(&gl, &vert_shader, &frag_shader)?;
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

        Ok(WebGl2Resources {
            gl,
            program,
            time_loc,
            resolution_loc,
            mouse_loc,
        })
    }

    /// Try to initialize WebGL1 context
    fn try_webgl1(canvas: &HtmlCanvasElement, width: u32, height: u32) -> Result<WebGl1Resources, JsValue> {
        let context_options = js_sys::Object::new();
        js_sys::Reflect::set(&context_options, &"alpha".into(), &true.into())?;
        js_sys::Reflect::set(&context_options, &"antialias".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"preserveDrawingBuffer".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"failIfMajorPerformanceCaveat".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"powerPreference".into(), &"low-power".into())?;

        let gl = canvas
            .get_context_with_context_options("webgl", &context_options)?
            .ok_or("WebGL1 not supported")?
            .dyn_into::<WebGlRenderingContext>()?;

        // Compile shaders
        let vert_shader = compile_shader_gl1(&gl, WebGlRenderingContext::VERTEX_SHADER, VERTEX_SHADER_GL1)?;
        let frag_shader = compile_shader_gl1(&gl, WebGlRenderingContext::FRAGMENT_SHADER, FRAGMENT_SHADER_GL1)?;

        // Link program
        let program = link_program_gl1(&gl, &vert_shader, &frag_shader)?;
        gl.use_program(Some(&program));

        // Set up vertex buffer for WebGL1 (no gl_VertexID)
        let positions: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        let buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&buffer));

        // Safety: transmuting f32 array to u8 is safe for WebGL
        unsafe {
            let positions_array = js_sys::Float32Array::view(&positions);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ARRAY_BUFFER,
                &positions_array,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }

        let position_loc = gl.get_attrib_location(&program, "a_position") as u32;
        gl.enable_vertex_attrib_array(position_loc);
        gl.vertex_attrib_pointer_with_i32(position_loc, 2, WebGlRenderingContext::FLOAT, false, 0, 0);

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

        Ok(WebGl1Resources {
            gl,
            program,
            time_loc,
            resolution_loc,
            mouse_loc,
        })
    }

    /// Try to initialize Canvas2D context (static gradient fallback)
    fn try_canvas2d(canvas: &HtmlCanvasElement) -> Result<Canvas2DResources, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .ok_or("Canvas2D not supported")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        Ok(Canvas2DResources { ctx })
    }

    /// Set up WebGL context loss event handlers
    fn setup_context_loss_handlers(&mut self) -> Result<(), JsValue> {
        // Only set up for WebGL contexts
        match &self.context {
            RenderContext::Canvas2D(_) => return Ok(()),
            _ => {}
        }

        // Create context lost handler
        let context_lost = Closure::wrap(Box::new(move |event: web_sys::WebGlContextEvent| {
            event.prevent_default();
            web_sys::console::warn_1(&"WebGL context lost - animation paused".into());
        }) as Box<dyn FnMut(web_sys::WebGlContextEvent)>);

        // Create context restored handler
        let context_restored = Closure::wrap(Box::new(move |_event: web_sys::WebGlContextEvent| {
            web_sys::console::log_1(&"WebGL context restored - reinitializing".into());
            // Note: Full reinitialization would require recreating shaders
            // For now, we just log and let the next render attempt handle it
        }) as Box<dyn FnMut(web_sys::WebGlContextEvent)>);

        // Add event listeners
        self.canvas.add_event_listener_with_callback(
            "webglcontextlost",
            context_lost.as_ref().unchecked_ref(),
        )?;
        self.canvas.add_event_listener_with_callback(
            "webglcontextrestored",
            context_restored.as_ref().unchecked_ref(),
        )?;

        // Store closures to keep them alive
        self._context_lost_closure = Some(context_lost);
        self._context_restored_closure = Some(context_restored);

        Ok(())
    }

    /// Get the current render backend
    pub fn backend(&self) -> RenderBackend {
        match &self.context {
            RenderContext::WebGl2(_) => RenderBackend::WebGl2,
            RenderContext::WebGl1(_) => RenderBackend::WebGl1,
            RenderContext::Canvas2D(_) => RenderBackend::Canvas2D,
        }
    }

    /// Check if context is lost
    pub fn is_context_lost(&self) -> bool {
        match &self.context {
            RenderContext::WebGl2(res) => res.gl.is_context_lost(),
            RenderContext::WebGl1(res) => res.gl.is_context_lost(),
            RenderContext::Canvas2D(_) => false,
        }
    }

    /// Update mouse position (normalized 0-1)
    pub fn set_mouse(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    /// Render one frame
    ///
    /// Returns false if context is lost and rendering should be paused.
    pub fn render(&self, time: f64) -> bool {
        // Check for context loss
        if self.is_context_lost() {
            return false;
        }

        let elapsed = ((time - self.start_time) / 1000.0) as f32;

        match &self.context {
            RenderContext::WebGl2(res) => {
                res.gl.use_program(Some(&res.program));
                res.gl.uniform1f(Some(&res.time_loc), elapsed);
                res.gl.uniform2f(Some(&res.mouse_loc), self.mouse_x, self.mouse_y);

                res.gl.clear_color(0.0, 0.0, 0.0, 1.0);
                res.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
                res.gl.draw_arrays(WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);
            }
            RenderContext::WebGl1(res) => {
                res.gl.use_program(Some(&res.program));
                res.gl.uniform1f(Some(&res.time_loc), elapsed);
                res.gl.uniform2f(Some(&res.mouse_loc), self.mouse_x, self.mouse_y);

                res.gl.clear_color(0.0, 0.0, 0.0, 1.0);
                res.gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT);
                res.gl.draw_arrays(WebGlRenderingContext::TRIANGLE_STRIP, 0, 4);
            }
            RenderContext::Canvas2D(res) => {
                // Simple animated gradient for Canvas2D fallback
                let width = self.canvas.width() as f64;
                let height = self.canvas.height() as f64;

                // Create gradient
                let gradient = res
                    .ctx
                    .create_radial_gradient(
                        width * 0.5,
                        height * 0.5,
                        0.0,
                        width * 0.5,
                        height * 0.5,
                        width.max(height) * 0.7,
                    )
                    .unwrap();

                // Animate color stops slightly based on time
                let t = (elapsed * 0.1).sin() * 0.1;

                let _ = gradient.add_color_stop(0.0, &format!("rgb({}, 0, {})", (75.0 + t * 50.0) as u8, (130.0 + t * 30.0) as u8));
                let _ = gradient.add_color_stop(0.5, "#2D004D");
                let _ = gradient.add_color_stop(1.0, "#0a0014");

                res.ctx.set_fill_style_canvas_gradient(&gradient);
                res.ctx.fill_rect(0.0, 0.0, width, height);
            }
        }

        true
    }

    /// Handle window resize
    pub fn resize(&self, width: u32, height: u32) {
        match &self.context {
            RenderContext::WebGl2(res) => {
                res.gl.viewport(0, 0, width as i32, height as i32);
                res.gl.uniform2f(Some(&res.resolution_loc), width as f32, height as f32);
            }
            RenderContext::WebGl1(res) => {
                res.gl.viewport(0, 0, width as i32, height as i32);
                res.gl.uniform2f(Some(&res.resolution_loc), width as f32, height as f32);
            }
            RenderContext::Canvas2D(_) => {
                // Canvas2D doesn't need special resize handling
            }
        }
    }
}

fn compile_shader_gl2(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Unable to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl.get_shader_info_log(&shader).unwrap_or_default();
        gl.delete_shader(Some(&shader));
        Err(log)
    }
}

fn link_program_gl2(
    gl: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Unable to create program")?;
    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        let log = gl.get_program_info_log(&program).unwrap_or_default();
        gl.delete_program(Some(&program));
        Err(log)
    }
}

fn compile_shader_gl1(
    gl: &WebGlRenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Unable to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGlRenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl.get_shader_info_log(&shader).unwrap_or_default();
        gl.delete_shader(Some(&shader));
        Err(log)
    }
}

fn link_program_gl1(
    gl: &WebGlRenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Unable to create program")?;
    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGlRenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        let log = gl.get_program_info_log(&program).unwrap_or_default();
        gl.delete_program(Some(&program));
        Err(log)
    }
}
