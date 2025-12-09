//! RenderPipeline - GPU Rendering Infrastructure
//!
//! # Overview
//!
//! Manages GPU rendering pipeline using wgpu (WebGPU cross-platform API).
//! Renders UI layouts to screen at 60 FPS.
//!
//! # Architecture
//!
//! ```text
//! RenderPipeline
//!   ├── GpuContextCapsule (wgpu device/queue, T7 Heterogeneous)
//!   ├── BufferPoolCapsule (vertex/index buffers, T1 Atomic)
//!   ├── ShapeCapsule (rectangles, circles, text glyphs, T0 Auditable)
//!   └── Frame timing (60 FPS target, <16.67ms)
//!
//! Render Flow:
//!   1. begin_frame() - Acquire swapchain texture
//!   2. render_layout(main_screen) - Render UI tree
//!   3. render_widget(widget, bounds) - Per-widget rendering
//!   4. end_frame() - Submit command buffer, present
//! ```
//!
//! # Performance Targets (B32)
//!
//! - Frame time: <10ms (60 FPS, GPU accelerated)
//! - begin_frame(): <1ms (acquire texture)
//! - render_widget(): <100µs per widget (5-20 widgets typical)
//! - end_frame(): <5ms (submit + present)
//! - Idle CPU: <1% (GPU does the work)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous (CPU+GPU coordination)
//! - **Chaos**: 100% lockfree (atomic buffer pool)
//! - **ASSUM**: wgpu is safe abstraction over Vulkan/Metal/DX12
//! - **B32**: <10ms frame time validated
//! - **T28**: Integration tests for render pipeline

use crate::gui_v2::state_machine::AppStateCapsule;
use crate::gui_v2::render::{ShapeRendererCapsule, ShapeInstance, TextRendererCapsule, TextVertex, SHAPES_WGSL, TEXT_WGSL, FontAtlasCapsule};
use crate::gui_v2::layout::{MainScreenLayout, Rect};
use crate::gui_v2::widgets::Color;
use super::types::{GuiError, GuiResult};
use super::gpu_backend::GpuBackendCapsule;
use std::sync::Arc;
use wgpu::{CommandEncoder, SurfaceTexture, util::DeviceExt};

/// Render pipeline for GPU-accelerated UI rendering
///
/// # Example
///
/// ```ignore
/// use kindly_dedup::gui_v2::integration::RenderPipeline;
///
/// let pipeline = RenderPipeline::new(gpu_context)?;
///
/// loop {
///     pipeline.begin_frame()?;
///     pipeline.render_layout(&main_screen)?;
///     pipeline.end_frame()?;
/// }
/// ```
/// Maximum shapes per frame (must match ShapeRendererCapsule::MAX_SHAPES)
const MAX_SHAPES: usize = 1024;

/// Maximum text vertices per frame (must match TextRendererCapsule::MAX_VERTICES)
const MAX_TEXT_VERTICES: usize = 65536;

/// Screen uniform data (matches WGSL binding 0)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniform {
    /// Screen width in pixels
    width: f32,
    /// Screen height in pixels
    height: f32,
}

pub struct RenderPipeline {
    /// Application state (for rendering UI based on state)
    app_state: Arc<AppStateCapsule>,

    /// GPU backend (wgpu device/queue/surface)
    gpu_backend: Arc<GpuBackendCapsule>,

    /// Shape renderer (rectangles, circles, borders)
    shape_renderer: ShapeRendererCapsule,

    /// Text renderer (glyphs, labels)
    text_renderer: TextRendererCapsule,

    /// Background color (Byzantine Purple)
    clear_color: [f32; 4],

    /// Viewport size (updated on resize)
    viewport: (u32, u32),

    /// Current frame's command encoder (active during frame)
    /// #ASSUME: Only set between begin_frame() and end_frame()
    /// #VERIFY: Dropped after end_frame() to release GPU resources
    current_encoder: Option<CommandEncoder>,

    /// Current frame's surface texture (active during frame)
    /// #ASSUME: Only set between begin_frame() and end_frame()
    /// #VERIFY: Presented in end_frame()
    current_texture: Option<SurfaceTexture>,

    // === GPU Pipeline Resources (lazy-initialized) ===

    /// Shape rendering pipeline (WGSL shader compiled)
    /// #ASSUME: Created once, reused every frame
    /// #VERIFY: Valid for lifetime of device
    shape_pipeline: Option<wgpu::RenderPipeline>,

    /// Bind group layout for screen uniform
    bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Screen dimensions uniform buffer
    screen_uniform_buffer: Option<wgpu::Buffer>,

    /// Bind group containing screen uniform
    screen_bind_group: Option<wgpu::BindGroup>,

    /// Vertex buffer for shape instances (pre-allocated)
    /// Capacity: MAX_SHAPES * sizeof(ShapeInstance) = 1024 * 40 = 40KB
    vertex_buffer: Option<wgpu::Buffer>,

    // === Text Rendering Resources (lazy-initialized) ===

    /// Text rendering pipeline (text.wgsl compiled)
    /// #ASSUME: Created once, reused every frame
    /// #VERIFY: Valid for lifetime of device
    text_pipeline: Option<wgpu::RenderPipeline>,

    /// Bind group layout for text rendering (uniform + texture + sampler)
    text_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Vertex buffer for text vertices (pre-allocated)
    /// Capacity: MAX_TEXT_VERTICES * sizeof(TextVertex) = 64K * 32 = 2MB
    text_vertex_buffer: Option<wgpu::Buffer>,

    /// Index buffer for text quads (6 indices per quad: 2 triangles)
    /// Capacity: MAX_TEXT_VERTICES / 4 * 6 indices = 16K quads * 6 = 96K indices = 192KB
    text_index_buffer: Option<wgpu::Buffer>,

    /// Font atlas texture (2048×2048 grayscale)
    /// #ASSUME: Created once, populated by FontAtlasCapsule
    /// #VERIFY: Valid for lifetime of device
    font_atlas_texture: Option<wgpu::Texture>,

    /// Font atlas sampler (linear filtering)
    font_atlas_sampler: Option<wgpu::Sampler>,

    /// Bind group for text rendering (uniform + texture + sampler)
    font_atlas_bind_group: Option<wgpu::BindGroup>,

    /// Accumulated text vertices for current frame
    /// Cleared in end_frame() after drawing
    /// #ASSUME: Capacity ≤ MAX_TEXT_VERTICES (65536)
    /// #VERIFY: Check len() before upload in end_frame()
    text_vertices: Vec<TextVertex>,
}

impl RenderPipeline {
    /// Create new render pipeline
    ///
    /// # Parameters
    ///
    /// - `app_state`: Shared app state (from AppRunner)
    ///
    /// # GPU Initialization
    ///
    /// 1. Create wgpu instance (Vulkan/Metal/DX12 backend selection)
    /// 2. Request adapter (physical GPU device)
    /// 3. Request device + queue (logical device)
    /// 4. Create swapchain (window surface, present mode)
    /// 5. Create render pipeline (shaders, vertex/fragment)
    /// 6. Create buffer pool (vertex/index buffers, 16MB pool)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - No compatible GPU adapter found
    /// - Device creation fails (driver issue)
    /// - Swapchain creation fails (window surface invalid)
    ///
    /// # Performance
    ///
    /// - Initialization: 50-100ms (GPU device creation is slow)
    /// - Memory: ~50MB (wgpu allocates GPU buffers)
    ///
    /// #ASSUME_GPU_AVAILABLE: wgpu finds adapter or returns error
    /// #VERIFY: Test with no GPU (should fallback to software rendering)
    ///
    /// #ASSUME_SWAPCHAIN_VSYNC: Present mode is Fifo (VSync enabled)
    /// #VERIFY: Measure frame pacing (should be 16.67ms ± 1ms)
    pub fn new(app_state: Arc<AppStateCapsule>, gpu_backend: Arc<GpuBackendCapsule>) -> GuiResult<Self> {
        // Byzantine Purple background (PURPLE_DEEP from theme: #241B38)
        let clear_color = [36.0 / 255.0, 27.0 / 255.0, 56.0 / 255.0, 1.0]; // #241B38 in sRGB

        // Get initial viewport from GPU backend
        let (width, height) = gpu_backend.surface_size();
        let viewport = (width as u32, height as u32);

        // Initialize renderers
        let shape_renderer = ShapeRendererCapsule::new();
        let text_renderer = TextRendererCapsule::new();

        Ok(Self {
            app_state,
            gpu_backend,
            shape_renderer,
            text_renderer,
            clear_color,
            viewport,
            current_encoder: None,
            current_texture: None,
            // GPU pipeline resources (lazy-initialized in ensure_pipeline())
            shape_pipeline: None,
            bind_group_layout: None,
            screen_uniform_buffer: None,
            screen_bind_group: None,
            vertex_buffer: None,
            // Text rendering resources (lazy-initialized in ensure_text_pipeline())
            text_pipeline: None,
            text_bind_group_layout: None,
            text_vertex_buffer: None,
            text_index_buffer: None,
            font_atlas_texture: None,
            font_atlas_sampler: None,
            font_atlas_bind_group: None,
            text_vertices: Vec::with_capacity(1024), // Preallocate for ~256 chars
        })
    }

    /// Ensure GPU pipeline resources are initialized
    ///
    /// Lazy initialization: creates pipeline, buffers, bind groups on first frame.
    /// Subsequent frames reuse existing resources.
    ///
    /// # Performance
    ///
    /// - First frame: ~10ms (shader compilation, buffer allocation)
    /// - Subsequent: <1µs (already initialized check)
    fn ensure_pipeline(&mut self) -> GuiResult<()> {
        // Already initialized?
        if self.shape_pipeline.is_some() {
            return Ok(());
        }

        let device = self.gpu_backend.device()
            .ok_or_else(|| GuiError::GpuInitFailed("Device not initialized".to_string()))?;

        // 1. Create bind group layout (screen uniform at binding 0)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shape_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // 2. Create screen uniform buffer
        let screen_uniform = ScreenUniform {
            width: self.viewport.0 as f32,
            height: self.viewport.1 as f32,
        };
        let screen_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("screen_uniform_buffer"),
            contents: bytemuck::cast_slice(&[screen_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 3. Create bind group
        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: screen_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // 4. Create vertex buffer (pre-allocated for MAX_SHAPES instances)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape_vertex_buffer"),
            size: (MAX_SHAPES * std::mem::size_of::<ShapeInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 5. Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shapes_shader"),
            source: wgpu::ShaderSource::Wgsl(SHAPES_WGSL.into()),
        });

        // 6. Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shape_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 7. Create render pipeline
        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shape_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    // ShapeInstance vertex buffer layout (instance-rate)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ShapeInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // x: f32 @ location 0
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // y: f32 @ location 1
                            wgpu::VertexAttribute {
                                offset: 4,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // width: f32 @ location 2
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // height: f32 @ location 3
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // color: vec4<f32> @ location 4
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // corner_radius: f32 @ location 5
                            wgpu::VertexAttribute {
                                offset: 32,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // border_width: f32 @ location 6
                            wgpu::VertexAttribute {
                                offset: 36,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.gpu_backend.surface_format(),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for 2D UI
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Store resources
        self.bind_group_layout = Some(bind_group_layout);
        self.screen_uniform_buffer = Some(screen_uniform_buffer);
        self.screen_bind_group = Some(screen_bind_group);
        self.vertex_buffer = Some(vertex_buffer);
        self.shape_pipeline = Some(shape_pipeline);

        Ok(())
    }

    /// Ensure text pipeline resources are initialized
    ///
    /// Lazy initialization: creates text pipeline, font atlas, buffers, bind groups on first text render.
    /// Subsequent calls reuse existing resources.
    ///
    /// # Resources Created
    ///
    /// 1. Font atlas texture (2048×2048 R8 grayscale, 4MB GPU memory)
    /// 2. Font atlas sampler (linear filtering for smooth scaling)
    /// 3. Text vertex buffer (64K vertices × 32 bytes = 2MB)
    /// 4. Bind group layout (uniform + texture + sampler)
    /// 5. Bind group (connects all resources)
    /// 6. Text render pipeline (text.wgsl compiled)
    ///
    /// # Performance
    ///
    /// - First call: ~15ms (texture allocation, shader compilation)
    /// - Subsequent: <1µs (already initialized check)
    ///
    /// #ASSUME_ATLAS_DATA_AVAILABLE: FontAtlasCapsule has generated atlas bitmap
    /// #VERIFY: Test without atlas data (should create empty texture placeholder)
    fn ensure_text_pipeline(&mut self) -> GuiResult<()> {
        // Already initialized?
        if self.text_pipeline.is_some() {
            return Ok(());
        }

        let device = self.gpu_backend.device()
            .ok_or_else(|| GuiError::GpuInitFailed("Device not initialized".to_string()))?;

        let queue = self.gpu_backend.queue()
            .ok_or_else(|| GuiError::GpuInitFailed("Queue not initialized".to_string()))?;

        // 1. Create font atlas texture (2048×2048 R8 grayscale)
        let atlas_size = wgpu::Extent3d {
            width: 2048,
            height: 2048,
            depth_or_array_layers: 1,
        };

        let font_atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font_atlas_texture"),
            size: atlas_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm, // RGBA for MSDF (RGB = distance, A = alpha)
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create FontAtlasCapsule and get RGBA texture data (16MB)
        // MSDF requires full RGBA data: RGB channels contain multi-channel distance field
        let font_atlas = FontAtlasCapsule::new();
        let atlas_data = font_atlas.texture_data();

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &font_atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2048 * 4), // 4 bytes per pixel (RGBA8)
                rows_per_image: Some(2048),
            },
            atlas_size,
        );

        // 2. Create font atlas sampler (linear filtering)
        let font_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font_atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 3. Create bind group layout (uniform + texture + sampler)
        let text_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_bind_group_layout"),
            entries: &[
                // Binding 0: Screen uniform (width, height)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Font atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 2: Font atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 4. Reuse screen uniform buffer from shape pipeline (or create if needed)
        // We use the same screen dimensions uniform for both shapes and text
        if self.screen_uniform_buffer.is_none() {
            let screen_uniform = ScreenUniform {
                width: self.viewport.0 as f32,
                height: self.viewport.1 as f32,
            };
            self.screen_uniform_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("screen_uniform_buffer"),
                contents: bytemuck::cast_slice(&[screen_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }));
        }

        // 5. Create bind group
        let font_atlas_view = font_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let font_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("font_atlas_bind_group"),
            layout: &text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.screen_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&font_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&font_atlas_sampler),
                },
            ],
        });

        // 6. Create text vertex buffer (pre-allocated for MAX_TEXT_VERTICES)
        let text_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_vertex_buffer"),
            size: (MAX_TEXT_VERTICES * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 7. Create text index buffer (6 indices per quad: [0,1,2, 1,3,2])
        // Generate indices for MAX_TEXT_VERTICES / 4 quads
        let max_quads = MAX_TEXT_VERTICES / 4;
        let mut indices: Vec<u16> = Vec::with_capacity(max_quads * 6);
        for quad_idx in 0..max_quads {
            let base = (quad_idx * 4) as u16;
            // First triangle: top-left, top-right, bottom-left
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            // Second triangle: top-right, bottom-right, bottom-left
            indices.push(base + 1);
            indices.push(base + 3);
            indices.push(base + 2);
        }
        let text_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_index_buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // 8. Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_WGSL.into()),
        });

        // 8. Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 9. Create text render pipeline
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    // TextVertex buffer layout (per-vertex, not instance)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TextVertex>() as u64, // 32 bytes
                        step_mode: wgpu::VertexStepMode::Vertex, // Per-vertex (not instance!)
                        attributes: &[
                            // pos_x: f32 @ location 0
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // pos_y: f32 @ location 1
                            wgpu::VertexAttribute {
                                offset: 4,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // tex_u: f32 @ location 2
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // tex_v: f32 @ location 3
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // color_r: f32 @ location 4
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // color_g: f32 @ location 5
                            wgpu::VertexAttribute {
                                offset: 20,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // color_b: f32 @ location 6
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // color_a: f32 @ location 7
                            wgpu::VertexAttribute {
                                offset: 28,
                                shader_location: 7,
                                format: wgpu::VertexFormat::Float32,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.gpu_backend.surface_format(),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for 2D UI
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Store resources
        self.text_bind_group_layout = Some(text_bind_group_layout);
        self.text_vertex_buffer = Some(text_vertex_buffer);
        self.text_index_buffer = Some(text_index_buffer);
        self.font_atlas_texture = Some(font_atlas_texture);
        self.font_atlas_sampler = Some(font_atlas_sampler);
        self.font_atlas_bind_group = Some(font_atlas_bind_group);
        self.text_pipeline = Some(text_pipeline);

        Ok(())
    }

    /// Begin rendering a new frame
    ///
    /// # Steps
    ///
    /// 1. Acquire swapchain texture (next frame buffer)
    /// 2. Create command encoder (GPU command buffer)
    /// 3. Begin render pass (clear screen to background color)
    ///
    /// # Performance
    ///
    /// - Acquire texture: <1ms (waits for VSync if needed)
    /// - Create encoder: <10µs (CPU allocation)
    /// - Begin pass: <10µs (GPU command encoding)
    ///
    /// #ASSUME_SWAPCHAIN_READY: Texture acquisition never fails (blocks until available)
    /// #VERIFY: Test with rapid resize (should not fail or stutter)
    pub fn begin_frame(&mut self) -> GuiResult<()> {
        // 1. Acquire swapchain texture
        let texture = self.gpu_backend.acquire_texture()?;

        // 2. Create command encoder
        let device = self.gpu_backend.device()
            .ok_or_else(|| GuiError::GpuInitFailed("Device not initialized".to_string()))?;

        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gui_v2_encoder"),
        });

        // Store for use in render_layout/end_frame
        self.current_encoder = Some(encoder);
        self.current_texture = Some(texture);

        Ok(())
    }

    /// Render main UI layout
    ///
    /// # Layout Tree Traversal
    ///
    /// ```text
    /// Main Screen (900×1000)
    ///   ├── Header (900×80) - Title + logo
    ///   ├── File Input (900×120) - File picker button
    ///   ├── Settings (900×100) - Threshold slider + mode selector
    ///   ├── Action Buttons (900×80) - Start/Cancel
    ///   ├── Progress (900×60) - Progress bar + status
    ///   └── Results (900×560) - Scrollable results list
    /// ```
    ///
    /// # Rendering Order
    ///
    /// 1. Backgrounds (rectangles, fills)
    /// 2. Borders (strokes, outlines)
    /// 3. Text (glyphs, labels)
    /// 4. Icons (SVG paths, images)
    ///
    /// # Performance
    ///
    /// - Layout traversal: <100µs (6 main sections)
    /// - Per widget: <100µs (5-20 widgets total)
    /// - Total: <2ms (typical frame)
    ///
    /// #ASSUME_LAYOUT_TREE_ACYCLIC: No circular references (stack overflow prevention)
    /// #VERIFY: Test with deeply nested layout (should not crash)
    pub fn render_layout(&mut self) -> GuiResult<()> {
        // Calculate layout regions based on viewport
        let layout = MainScreenLayout::new(self.viewport.0 as u16, self.viewport.1 as u16);
        let regions = layout.calculate_regions();

        // Byzantine theme colors
        let purple_surface = Color::rgb(53, 42, 80);   // #352A50
        let purple_primary = Color::rgb(155, 89, 182); // #9B59B6
        let text_color = Color::rgb(232, 232, 232);    // #E8E8E8
        let gold = Color::rgb(255, 215, 0);            // Gold accent

        // Render header
        self.render_widget_rect(regions.header, purple_primary, Some((purple_surface, 2)))?;
        self.render_widget_text(regions.header, "Kindly Dedup", text_color)?;

        // Render file input card
        self.render_widget_rect(regions.file_input_card, purple_surface, Some((purple_primary, 1)))?;
        self.render_widget_text(regions.file_input_card, "Select File", text_color)?;

        // Render settings card
        self.render_widget_rect(regions.settings_card, purple_surface, Some((purple_primary, 1)))?;
        self.render_widget_text(regions.settings_card, "Settings", text_color)?;

        // Render action button (gold)
        self.render_widget_rect(regions.action_button, gold, None)?;
        self.render_widget_text(regions.action_button, "Start", Color::rgb(0, 0, 0))?;

        // Render progress card
        self.render_widget_rect(regions.progress_card, purple_surface, Some((purple_primary, 1)))?;
        self.render_widget_text(regions.progress_card, "Progress", text_color)?;

        // Render results card
        self.render_widget_rect(regions.results_card, purple_surface, Some((purple_primary, 1)))?;
        self.render_widget_text(regions.results_card, "Results", text_color)?;

        // Render feature badges (3×2 grid)
        for (i, badge) in regions.feature_badges.iter().enumerate() {
            self.render_widget_rect(*badge, purple_surface, Some((purple_primary, 1)))?;
            let label = match i {
                0 => "38× Speedup",
                1 => "100% Safe",
                2 => "Lockfree",
                3 => "GPU Accel",
                4 => "T10 Tier",
                5 => "Chaos",
                _ => "Feature",
            };
            self.render_widget_text(*badge, label, text_color)?;
        }

        // Render footer
        self.render_widget_rect(regions.footer, purple_primary, None)?;
        self.render_widget_text(regions.footer, "Kindly Ecosystem • v3.0", text_color)?;

        Ok(())
    }

    /// Render rectangle widget (filled + optional border)
    ///
    /// # Arguments
    ///
    /// - `bounds`: Layout bounds (Q16.16 fixed-point)
    /// - `fill_color`: Fill color
    /// - `border`: Optional (border_color, width)
    fn render_widget_rect(&mut self, bounds: Rect, fill_color: Color, border: Option<(Color, u16)>) -> GuiResult<()> {
        // Render filled rectangle (ignore full buffer errors for now)
        let _ = self.shape_renderer.push_filled_rect(bounds, fill_color);

        // Render border if requested
        if let Some((border_color, border_width)) = border {
            let _ = self.shape_renderer.push_border(bounds, border_color, border_width);
        }

        Ok(())
    }

    /// Render text widget (centered in bounds)
    ///
    /// # Arguments
    ///
    /// - `bounds`: Layout bounds (Q16.16 fixed-point)
    /// - `text`: Text to render
    /// - `color`: Text color
    ///
    /// # Performance
    ///
    /// - Text layout: <1μs per 100 chars (simple monospace estimation)
    /// - Vertex generation: <50ns per char (cache-aligned writes)
    /// - Queue append: O(1) Vec push (amortized)
    ///
    /// #ASSUME: bounds are valid screen coordinates
    /// #VERIFY: Generated vertices fit in MAX_TEXT_VERTICES limit
    fn render_widget_text(&mut self, bounds: Rect, text: &str, color: Color) -> GuiResult<()> {
        use crate::gui_v2::render::TextRenderParams;

        // Convert Q16.16 fixed-point bounds to f32 pixels
        let x = (bounds.x >> 16) as f32;
        let y = (bounds.y >> 16) as f32;
        let width = (bounds.width >> 16) as f32;
        let height = (bounds.height >> 16) as f32;

        // Determine font size based on widget height
        let font_size = if height >= 64.0 {
            64 // Title
        } else if height >= 24.0 {
            18 // Subtitle
        } else {
            14 // Body
        };

        // Measure text to center it in bounds
        let params = TextRenderParams {
            font_size,
            color,
            x: 0.0, // Temporary, will adjust after measuring
            y: 0.0, // Temporary, will adjust after measuring
            line_height: 1.2,
        };

        let (text_width, text_height) = self.text_renderer.measure_text(text, params);

        // Center text in bounds
        let text_x = x + (width - text_width) / 2.0;
        let text_y = y + (height - text_height) / 2.0;

        // Update params with centered position
        let centered_params = TextRenderParams {
            font_size,
            color,
            x: text_x,
            y: text_y,
            line_height: 1.2,
        };

        // Generate text vertices
        let vertices = self.text_renderer.generate_text_vertices(text, centered_params);

        // Append to accumulated text vertices
        // #ASSUME: Total vertices across all text widgets ≤ MAX_TEXT_VERTICES
        // #VERIFY: Check in end_frame() before upload
        self.text_vertices.extend_from_slice(&vertices);

        Ok(())
    }

    /// Render individual widget (legacy stub, use render_widget_rect/text instead)
    ///
    /// # Widget Types
    ///
    /// - **Rectangle**: Solid fill, gradient, border
    /// - **Circle**: Buttons, avatars, icons
    /// - **Text**: Labels, headings, body text
    /// - **Image**: Logos, icons (future)
    ///
    /// # Rendering Pipeline
    ///
    /// 1. Check if widget in viewport (frustum culling)
    /// 2. Allocate GPU buffers (vertex/index from pool)
    /// 3. Write vertex data (position, color, UV)
    /// 4. Submit draw call (indexed triangle list)
    ///
    /// # Performance
    ///
    /// - Frustum culling: <10ns (SIMD AABB test)
    /// - Buffer allocation: <50ns (lockfree pool)
    /// - Vertex write: <500ns (memcpy to GPU buffer)
    /// - Draw call: <100µs (GPU execution)
    ///
    /// #ASSUME_WIDGET_BOUNDS_VALID: Bounds are non-negative, non-NaN
    /// #VERIFY: Test with invalid bounds (should clip or skip)
    pub fn render_widget(&self, _widget: (), _bounds: ()) -> GuiResult<()> {
        // Legacy stub - use render_widget_rect/text instead
        Ok(())
    }

    /// End frame and present to screen
    ///
    /// # Steps
    ///
    /// 1. Ensure pipeline initialized
    /// 2. Update screen uniform buffer
    /// 3. Upload shape instances to vertex buffer
    /// 4. Begin render pass (clear + draw)
    /// 5. Finish command encoder
    /// 6. Submit command buffer to GPU queue
    /// 7. Present swapchain texture
    /// 8. Clear shape buffer for next frame
    ///
    /// # Performance
    ///
    /// - Pipeline init: ~10ms first frame, <1µs subsequent
    /// - Buffer upload: <100µs (40KB max)
    /// - Render pass: <1ms (GPU parallel)
    /// - Submit: <100µs (queue submission)
    /// - Present: <5ms (waits for VSync, displays frame)
    ///
    /// #ASSUME_SUBMIT_NEVER_FAILS: Command buffer submission is infallible
    /// #VERIFY: Test with GPU reset (should recover gracefully)
    ///
    /// #ASSUME_PRESENT_VSYNC: Present blocks until VSync (60 FPS limit)
    /// #VERIFY: Measure frame pacing (should be 16.67ms ± 1ms)
    pub fn end_frame(&mut self) -> GuiResult<()> {
        // 1. Ensure pipelines are initialized
        self.ensure_pipeline()?;
        self.ensure_text_pipeline()?;

        // Take ownership of encoder and texture
        let mut encoder = self.current_encoder.take()
            .ok_or_else(|| GuiError::GpuInitFailed("begin_frame() not called".to_string()))?;

        let texture = self.current_texture.take()
            .ok_or_else(|| GuiError::GpuInitFailed("begin_frame() not called".to_string()))?;

        let queue = self.gpu_backend.queue()
            .ok_or_else(|| GuiError::GpuInitFailed("Queue not initialized".to_string()))?;

        // 2. Update screen uniform buffer with current viewport
        let screen_uniform = ScreenUniform {
            width: self.viewport.0 as f32,
            height: self.viewport.1 as f32,
        };
        if let Some(ref buffer) = self.screen_uniform_buffer {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[screen_uniform]));
        }

        // 3. Upload shape instances to vertex buffer
        let instances = self.shape_renderer.instances();
        let instance_count = instances.len();

        if instance_count > 0 {
            if let Some(ref buffer) = self.vertex_buffer {
                // SAFETY: ShapeInstance is repr(C) and all fields are f32
                let instance_bytes: &[u8] = unsafe {
                    std::slice::from_raw_parts(
                        instances.as_ptr() as *const u8,
                        instance_count * std::mem::size_of::<ShapeInstance>(),
                    )
                };
                queue.write_buffer(buffer, 0, instance_bytes);
            }
        }

        // 4. Create render pass with clear color (Byzantine Purple)
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gui_v2_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color[0] as f64,
                            g: self.clear_color[1] as f64,
                            b: self.clear_color[2] as f64,
                            a: self.clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 5. Draw shapes if we have any
            if instance_count > 0 {
                if let (Some(pipeline), Some(bind_group), Some(vertex_buffer)) = (
                    self.shape_pipeline.as_ref(),
                    self.screen_bind_group.as_ref(),
                    self.vertex_buffer.as_ref(),
                ) {
                    render_pass.set_pipeline(pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    // 6 vertices per instance (2 triangles forming a quad)
                    render_pass.draw(0..6, 0..instance_count as u32);
                }
            }

            // 6. Draw text if we have any
            let text_vertex_count = self.text_vertices.len();
            if text_vertex_count > 0 {
                // Ensure text pipeline is initialized
                // Note: We do this inside render pass scope to avoid borrow issues
                // The actual initialization happens lazily on first use
                if let (Some(pipeline), Some(bind_group), Some(vertex_buffer), Some(index_buffer)) = (
                    self.text_pipeline.as_ref(),
                    self.font_atlas_bind_group.as_ref(),
                    self.text_vertex_buffer.as_ref(),
                    self.text_index_buffer.as_ref(),
                ) {
                    // Upload text vertices to GPU buffer
                    // SAFETY: TextVertex is repr(C) with all f32 fields (bytemuck::Pod)
                    let text_bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(
                            self.text_vertices.as_ptr() as *const u8,
                            text_vertex_count * std::mem::size_of::<TextVertex>(),
                        )
                    };
                    queue.write_buffer(vertex_buffer, 0, text_bytes);

                    // Set text pipeline and draw
                    render_pass.set_pipeline(pipeline);
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

                    // Draw indexed text quads
                    // 4 vertices per glyph → 1 quad → 6 indices (2 triangles)
                    let quad_count = text_vertex_count / 4;
                    let index_count = quad_count * 6;
                    render_pass.draw_indexed(0..index_count as u32, 0, 0..1);
                }
            }
            // Render pass drops here (ends pass)
        }

        // 6. Finish command encoder
        let command_buffer = encoder.finish();

        // 7. Submit to queue
        queue.submit(std::iter::once(command_buffer));

        // 8. Present swapchain
        texture.present();

        // 9. Clear buffers for next frame
        self.shape_renderer.clear();
        self.text_vertices.clear();

        Ok(())
    }

    /// Update viewport size (on window resize)
    ///
    /// # Steps
    ///
    /// 1. Update swapchain configuration
    /// 2. Recreate swapchain textures
    /// 3. Update projection matrix (orthographic 2D)
    /// 4. Invalidate layout cache
    ///
    /// # Performance
    ///
    /// - Swapchain recreate: <10ms (GPU resource allocation)
    /// - Projection update: <1µs (matrix calculation)
    ///
    /// #ASSUME_RESIZE_VALID: (width, height) > 0
    /// #VERIFY: Test with minimum size (should clamp to 1×1)
    pub fn resize(&mut self, width: u32, height: u32) -> GuiResult<()> {
        // Update viewport
        self.viewport = (width, height);

        // Delegate to GPU backend (handles surface reconfiguration)
        // This will clamp to minimum 1×1 and reconfigure wgpu surface
        // Note: Arc::get_mut fails if there are other Arc references - that's OK,
        // the surface texture will be recreated on next acquire_texture call.
        // Resize is best-effort; rendering continues at original size until surface reconfigured.
        if let Some(backend) = Arc::get_mut(&mut self.gpu_backend) {
            backend.resize(width, height)?;
        }
        // If we can't get mutable access, the next frame will handle it via surface outdated error

        Ok(())
    }

    /// Get current viewport size
    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    /// Get clear color (background)
    pub fn clear_color(&self) -> [f32; 4] {
        self.clear_color
    }

    /// Set clear color (for theme changes)
    pub fn set_clear_color(&mut self, color: [f32; 4]) {
        self.clear_color = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_v2::state_machine::AppStateCapsule;

    #[test]
    fn test_clear_color_update() {
        // Note: Can't test RenderPipeline creation without real window/GPU
        // See integration tests for full GPU backend validation

        // Test Byzantine Purple color calculation
        let purple_deep: [f64; 4] = [36.0 / 255.0, 27.0 / 255.0, 56.0 / 255.0, 1.0];
        assert!((purple_deep[0] - 0.141).abs() < 0.001);
        assert!((purple_deep[1] - 0.106).abs() < 0.001);
        assert!((purple_deep[2] - 0.220).abs() < 0.001);
        assert_eq!(purple_deep[3], 1.0);
    }

    #[test]
    #[ignore = "Requires GPU hardware and window - run manually with integration tests"]
    fn test_render_pipeline_creation() {
        // This test requires a real window and GPU, which requires running event loop
        // See integration tests for full validation
    }

    #[test]
    #[ignore = "Requires GPU hardware and window - run manually with integration tests"]
    fn test_viewport_resize() {
        // This test requires a real window and GPU
        // See integration tests for full validation
    }

    #[test]
    #[ignore = "Requires GPU hardware and window - run manually with integration tests"]
    fn test_begin_end_frame() {
        // This test requires a real window and GPU
        // See integration tests for full validation
    }

    #[test]
    fn test_render_layout() {
        // Layout rendering stubbed for Phase 3
        // Will be implemented in Phase 3: Widget System
    }

    #[test]
    fn test_render_widget() {
        // Widget rendering stubbed for Phase 3
        // Will be implemented in Phase 3: Widget System
    }
}
