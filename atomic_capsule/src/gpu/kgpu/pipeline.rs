//! KgpuPipelineCapsule - Lockfree GPU Pipeline State Objects
//!
//! **Tier**: T1+T6 (Atomic + Mixed)
//! **Size**: 512B (render) / 256B (compute) - cache-aligned
//! **Purpose**: GPU pipeline state management with lockfree atomics
//!
//! # Architecture
//!
//! Pipelines define the complete configuration for GPU rendering or compute operations.
//! This module provides two capsule types:
//!
//! - **KgpuRenderPipelineCapsule**: Full graphics pipeline (vertex, fragment, depth, blend)
//! - **KgpuComputePipelineCapsule**: Compute pipeline (compute shader only)
//!
//! # Memory Layout
//!
//! ## Render Pipeline (512B)
//! ```text
//! Offset  Size    Field
//! 0       64      KgpuHandle<RenderPipeline>
//! 64      8       Primary: state(8) | stage_count(8) | generation(48)
//! 72      8       Secondary: vertex_buffer_count(8) | color_target_count(8) | flags(48)
//! 80      8       Vertex shader handle
//! 88      8       Fragment shader handle
//! 96      64      Vertex layouts (4 x 16B)
//! 160     1       Primitive topology
//! 161     1       Front face
//! 162     1       Cull mode
//! 163     1       Depth compare
//! 164     1       Depth write enabled
//! 165     1       Stencil enabled
//! 166     2       Reserved
//! 168     32      Blend states (4 x 8B)
//! 200     32      Bind group layouts (4 x 8B)
//! 232     4       Reference count
//! 236     276     Padding to 512B
//! ```
//!
//! ## Compute Pipeline (256B)
//! ```text
//! Offset  Size    Field
//! 0       64      KgpuHandle<ComputePipeline>
//! 64      8       Primary: state(8) | workgroup_size_xyz(24) | generation(32)
//! 72      8       Secondary: shared_memory_size(32) | flags(32)
//! 80      8       Compute shader handle
//! 88      32      Bind group layouts (4 x 8B)
//! 120     4       Reference count
//! 124     132     Padding to 256B
//! ```
//!
//! # ASSUM Safety Documentation
//!
//! - `#ASSUME_PIPELINE_IMMUTABLE_AFTER_CREATE`: Pipeline state is set once during building
//!   and becomes immutable after finalization. This enables safe caching and hashing.
//!
//! - `#ASSUME_GENERATION_ABA_SAFE`: Generation counters prevent ABA problems.
//!
//! - `#ASSUME_HASH_DETERMINISTIC`: Pipeline hash is computed from immutable state,
//!   ensuring consistent results for pipeline caching.
//!
//! - `#ASSUME_CACHE_ALIGNED`: 512B/256B alignment prevents false sharing.
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1+T6 tier selection
//! - **Chaos**: 100% lockfree, zero mutex
//! - **ASSUM**: All assumptions documented
//! - **T28**: Comprehensive tests

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};

use super::handle::KgpuHandle;

// ============================================================================
// Enums
// ============================================================================

/// Primitive topology for rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PrimitiveTopology {
    /// Draw points
    #[default]
    PointList = 0,
    /// Draw lines (2 vertices per line)
    LineList = 1,
    /// Draw connected lines (each vertex connects to previous)
    LineStrip = 2,
    /// Draw triangles (3 vertices per triangle)
    TriangleList = 3,
    /// Draw connected triangles
    TriangleStrip = 4,
}

impl PrimitiveTopology {
    /// Convert from raw u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::PointList),
            1 => Some(Self::LineList),
            2 => Some(Self::LineStrip),
            3 => Some(Self::TriangleList),
            4 => Some(Self::TriangleStrip),
            _ => None,
        }
    }
}

/// Front face winding order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FrontFace {
    /// Counter-clockwise winding
    #[default]
    Ccw = 0,
    /// Clockwise winding
    Cw = 1,
}

impl FrontFace {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ccw),
            1 => Some(Self::Cw),
            _ => None,
        }
    }
}

/// Face culling mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CullMode {
    /// No culling
    #[default]
    None = 0,
    /// Cull front faces
    Front = 1,
    /// Cull back faces
    Back = 2,
}

impl CullMode {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Front),
            2 => Some(Self::Back),
            _ => None,
        }
    }
}

/// Depth comparison function.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CompareFunction {
    /// Never pass
    Never = 0,
    /// Pass if less than
    Less = 1,
    /// Pass if equal
    Equal = 2,
    /// Pass if less than or equal
    LessEqual = 3,
    /// Pass if greater than
    Greater = 4,
    /// Pass if not equal
    NotEqual = 5,
    /// Pass if greater than or equal
    GreaterEqual = 6,
    /// Always pass
    #[default]
    Always = 7,
}

impl CompareFunction {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Never),
            1 => Some(Self::Less),
            2 => Some(Self::Equal),
            3 => Some(Self::LessEqual),
            4 => Some(Self::Greater),
            5 => Some(Self::NotEqual),
            6 => Some(Self::GreaterEqual),
            7 => Some(Self::Always),
            _ => None,
        }
    }
}

/// Blend factor for color blending.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BlendFactor {
    /// Factor is zero
    #[default]
    Zero = 0,
    /// Factor is one
    One = 1,
    /// Factor is source color
    Src = 2,
    /// Factor is one minus source color
    OneMinusSrc = 3,
    /// Factor is destination color
    Dst = 4,
    /// Factor is one minus destination color
    OneMinusDst = 5,
    /// Factor is source alpha
    SrcAlpha = 6,
    /// Factor is one minus source alpha
    OneMinusSrcAlpha = 7,
    /// Factor is destination alpha
    DstAlpha = 8,
    /// Factor is one minus destination alpha
    OneMinusDstAlpha = 9,
}

impl BlendFactor {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Zero),
            1 => Some(Self::One),
            2 => Some(Self::Src),
            3 => Some(Self::OneMinusSrc),
            4 => Some(Self::Dst),
            5 => Some(Self::OneMinusDst),
            6 => Some(Self::SrcAlpha),
            7 => Some(Self::OneMinusSrcAlpha),
            8 => Some(Self::DstAlpha),
            9 => Some(Self::OneMinusDstAlpha),
            _ => None,
        }
    }
}

/// Blend operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BlendOperation {
    /// Result = src + dst
    #[default]
    Add = 0,
    /// Result = src - dst
    Subtract = 1,
    /// Result = dst - src
    ReverseSubtract = 2,
    /// Result = min(src, dst)
    Min = 3,
    /// Result = max(src, dst)
    Max = 4,
}

impl BlendOperation {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Add),
            1 => Some(Self::Subtract),
            2 => Some(Self::ReverseSubtract),
            3 => Some(Self::Min),
            4 => Some(Self::Max),
            _ => None,
        }
    }
}

/// Vertex input step mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VertexStepMode {
    /// Advance per vertex
    #[default]
    Vertex = 0,
    /// Advance per instance
    Instance = 1,
}

impl VertexStepMode {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Vertex),
            1 => Some(Self::Instance),
            _ => None,
        }
    }
}

/// Pipeline state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PipelineState {
    /// Pipeline is being built
    #[default]
    Building = 0,
    /// Pipeline is ready for use
    Ready = 1,
    /// Pipeline is bound
    Bound = 2,
    /// Pipeline has been destroyed
    Destroyed = 3,
}

impl PipelineState {
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Building),
            1 => Some(Self::Ready),
            2 => Some(Self::Bound),
            3 => Some(Self::Destroyed),
            _ => None,
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Maximum vertex buffers per pipeline
pub const MAX_VERTEX_BUFFERS: usize = 4;

/// Maximum color targets per pipeline
pub const MAX_COLOR_TARGETS: usize = 4;

/// Maximum bind group layouts per pipeline
pub const MAX_BIND_GROUPS: usize = 4;

// ============================================================================
// Bit Field Masks (Primary)
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;

const STAGE_COUNT_SHIFT: u64 = 48;
const STAGE_COUNT_MASK: u64 = 0xFF << STAGE_COUNT_SHIFT;

const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// ============================================================================
// Bit Field Masks (Secondary - Render)
// ============================================================================

const VB_COUNT_SHIFT: u64 = 56;
const VB_COUNT_MASK: u64 = 0xFF << VB_COUNT_SHIFT;

const CT_COUNT_SHIFT: u64 = 48;
const CT_COUNT_MASK: u64 = 0xFF << CT_COUNT_SHIFT;

const FLAGS_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// ============================================================================
// Bit Field Masks (Primary - Compute)
// ============================================================================

const WORKGROUP_X_SHIFT: u64 = 40;
const WORKGROUP_X_MASK: u64 = 0xFF << WORKGROUP_X_SHIFT;

const WORKGROUP_Y_SHIFT: u64 = 32;
const WORKGROUP_Y_MASK: u64 = 0xFF << WORKGROUP_Y_SHIFT;

const WORKGROUP_Z_SHIFT: u64 = 24;
const WORKGROUP_Z_MASK: u64 = 0xFF << WORKGROUP_Z_SHIFT;

const COMPUTE_GEN_MASK: u64 = 0x0000_0000_00FF_FFFF;

// ============================================================================
// Bit Field Masks (Secondary - Compute)
// ============================================================================

const SHARED_MEM_SHIFT: u64 = 32;
const SHARED_MEM_MASK: u64 = 0xFFFF_FFFF << SHARED_MEM_SHIFT;

// ============================================================================
// Pipeline Flags
// ============================================================================

/// Pipeline has depth testing enabled
pub const PIPELINE_FLAG_DEPTH_TEST: u64 = 1 << 0;

/// Pipeline has depth writing enabled
pub const PIPELINE_FLAG_DEPTH_WRITE: u64 = 1 << 1;

/// Pipeline has stencil testing enabled
pub const PIPELINE_FLAG_STENCIL_TEST: u64 = 1 << 2;

/// Pipeline has blending enabled
pub const PIPELINE_FLAG_BLEND: u64 = 1 << 3;

/// Pipeline uses multisampling
pub const PIPELINE_FLAG_MULTISAMPLE: u64 = 1 << 4;

/// Pipeline is immutable after creation
pub const PIPELINE_FLAG_IMMUTABLE: u64 = 1 << 5;

// ============================================================================
// Marker Types
// ============================================================================

/// Marker type for render pipeline resources
#[derive(Debug, Clone, Copy)]
pub struct RenderPipeline;

/// Marker type for compute pipeline resources
#[derive(Debug, Clone, Copy)]
pub struct ComputePipeline;

// ============================================================================
// VertexLayoutSlot
// ============================================================================

/// Vertex buffer layout configuration.
///
/// # Layout (16B)
/// ```text
/// 0-4     stride (AtomicU32)
/// 4-5     step_mode (AtomicU8)
/// 5-6     attribute_count (AtomicU8)
/// 6-16    padding
/// ```
#[repr(C)]
pub struct VertexLayoutSlot {
    /// Stride between vertices in bytes
    stride: AtomicU32,
    /// Per-vertex or per-instance stepping
    step_mode: AtomicU8,
    /// Number of attributes in this layout
    attribute_count: AtomicU8,
    /// Padding
    _padding: [u8; 10],
}

impl VertexLayoutSlot {
    /// Create a new empty vertex layout slot
    #[inline]
    pub const fn new() -> Self {
        Self {
            stride: AtomicU32::new(0),
            step_mode: AtomicU8::new(VertexStepMode::Vertex as u8),
            attribute_count: AtomicU8::new(0),
            _padding: [0; 10],
        }
    }

    /// Set the vertex layout
    pub fn set(&self, stride: u32, step_mode: VertexStepMode, attribute_count: u8) {
        self.stride.store(stride, Ordering::Release);
        self.step_mode.store(step_mode as u8, Ordering::Release);
        self.attribute_count
            .store(attribute_count, Ordering::Release);
    }

    /// Get stride
    #[inline]
    pub fn stride(&self) -> u32 {
        self.stride.load(Ordering::Acquire)
    }

    /// Get step mode
    #[inline]
    pub fn step_mode(&self) -> VertexStepMode {
        let raw = self.step_mode.load(Ordering::Acquire);
        VertexStepMode::from_u8(raw).unwrap_or(VertexStepMode::Vertex)
    }

    /// Get attribute count
    #[inline]
    pub fn attribute_count(&self) -> u8 {
        self.attribute_count.load(Ordering::Acquire)
    }

    /// Check if slot is configured
    #[inline]
    pub fn is_configured(&self) -> bool {
        self.stride() > 0
    }
}

impl Default for VertexLayoutSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for VertexLayoutSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VertexLayoutSlot")
            .field("stride", &self.stride())
            .field("step_mode", &self.step_mode())
            .field("attribute_count", &self.attribute_count())
            .finish()
    }
}

const _: () = {
    assert!(core::mem::size_of::<VertexLayoutSlot>() == 16);
};

// ============================================================================
// BlendState
// ============================================================================

/// Blend state for a color target.
///
/// # Layout (8B)
/// ```text
/// 0-1     src_factor (AtomicU8)
/// 1-2     dst_factor (AtomicU8)
/// 2-3     operation (AtomicU8)
/// 3-4     src_alpha_factor (AtomicU8)
/// 4-5     dst_alpha_factor (AtomicU8)
/// 5-6     alpha_operation (AtomicU8)
/// 6-8     padding
/// ```
#[repr(C)]
pub struct BlendState {
    src_factor: AtomicU8,
    dst_factor: AtomicU8,
    operation: AtomicU8,
    src_alpha_factor: AtomicU8,
    dst_alpha_factor: AtomicU8,
    alpha_operation: AtomicU8,
    _padding: [u8; 2],
}

impl BlendState {
    /// Create a new default blend state (no blending)
    #[inline]
    pub const fn new() -> Self {
        Self {
            src_factor: AtomicU8::new(BlendFactor::One as u8),
            dst_factor: AtomicU8::new(BlendFactor::Zero as u8),
            operation: AtomicU8::new(BlendOperation::Add as u8),
            src_alpha_factor: AtomicU8::new(BlendFactor::One as u8),
            dst_alpha_factor: AtomicU8::new(BlendFactor::Zero as u8),
            alpha_operation: AtomicU8::new(BlendOperation::Add as u8),
            _padding: [0; 2],
        }
    }

    /// Set blend state for standard alpha blending
    pub fn set_alpha_blend(&self) {
        self.src_factor.store(BlendFactor::SrcAlpha as u8, Ordering::Release);
        self.dst_factor.store(BlendFactor::OneMinusSrcAlpha as u8, Ordering::Release);
        self.operation.store(BlendOperation::Add as u8, Ordering::Release);
        self.src_alpha_factor.store(BlendFactor::One as u8, Ordering::Release);
        self.dst_alpha_factor.store(BlendFactor::Zero as u8, Ordering::Release);
        self.alpha_operation.store(BlendOperation::Add as u8, Ordering::Release);
    }

    /// Set custom blend state
    pub fn set(
        &self,
        src: BlendFactor,
        dst: BlendFactor,
        op: BlendOperation,
        src_alpha: BlendFactor,
        dst_alpha: BlendFactor,
        alpha_op: BlendOperation,
    ) {
        self.src_factor.store(src as u8, Ordering::Release);
        self.dst_factor.store(dst as u8, Ordering::Release);
        self.operation.store(op as u8, Ordering::Release);
        self.src_alpha_factor.store(src_alpha as u8, Ordering::Release);
        self.dst_alpha_factor.store(dst_alpha as u8, Ordering::Release);
        self.alpha_operation.store(alpha_op as u8, Ordering::Release);
    }

    /// Get source factor
    #[inline]
    pub fn src_factor(&self) -> BlendFactor {
        BlendFactor::from_u8(self.src_factor.load(Ordering::Acquire)).unwrap_or(BlendFactor::One)
    }

    /// Get destination factor
    #[inline]
    pub fn dst_factor(&self) -> BlendFactor {
        BlendFactor::from_u8(self.dst_factor.load(Ordering::Acquire)).unwrap_or(BlendFactor::Zero)
    }

    /// Get blend operation
    #[inline]
    pub fn operation(&self) -> BlendOperation {
        BlendOperation::from_u8(self.operation.load(Ordering::Acquire))
            .unwrap_or(BlendOperation::Add)
    }

    /// Check if this is standard alpha blending
    #[inline]
    pub fn is_alpha_blend(&self) -> bool {
        self.src_factor() == BlendFactor::SrcAlpha
            && self.dst_factor() == BlendFactor::OneMinusSrcAlpha
    }
}

impl Default for BlendState {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for BlendState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BlendState")
            .field("src_factor", &self.src_factor())
            .field("dst_factor", &self.dst_factor())
            .field("operation", &self.operation())
            .finish()
    }
}

const _: () = {
    assert!(core::mem::size_of::<BlendState>() == 8);
};

// ============================================================================
// PipelineError
// ============================================================================

/// Errors that can occur during pipeline operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineError {
    /// Pipeline is in an invalid state
    InvalidState,
    /// Pipeline has been destroyed
    Destroyed,
    /// Vertex shader not set
    MissingVertexShader,
    /// Compute shader not set
    MissingComputeShader,
    /// Index out of range
    IndexOutOfRange,
    /// Pipeline is immutable
    Immutable,
}

impl core::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState => write!(f, "Invalid pipeline state"),
            Self::Destroyed => write!(f, "Pipeline destroyed"),
            Self::MissingVertexShader => write!(f, "Vertex shader not set"),
            Self::MissingComputeShader => write!(f, "Compute shader not set"),
            Self::IndexOutOfRange => write!(f, "Index out of range"),
            Self::Immutable => write!(f, "Pipeline is immutable"),
        }
    }
}

/// Result type for pipeline operations
pub type PipelineResult<T> = Result<T, PipelineError>;

// ============================================================================
// KgpuRenderPipelineCapsule
// ============================================================================

/// GPU Render Pipeline Capsule with Lockfree Atomics
///
/// Complete graphics pipeline state including vertex processing,
/// primitive assembly, rasterization, fragment processing, and output merging.
///
/// # Tier: T1+T6 (Atomic + Mixed)
/// # Size: 512B (cache-aligned)
#[repr(C, align(512))]
pub struct KgpuRenderPipelineCapsule {
    /// Resource handle
    handle: KgpuHandle<RenderPipeline>,

    /// Primary: state(8) | stage_count(8) | generation(48)
    primary: AtomicU64,

    /// Secondary: vertex_buffer_count(8) | color_target_count(8) | flags(48)
    secondary: AtomicU64,

    /// Vertex shader handle
    vertex_shader: AtomicU64,

    /// Fragment shader handle (optional)
    fragment_shader: AtomicU64,

    /// Vertex buffer layouts (4 slots, 16B each = 64B)
    vertex_layouts: [VertexLayoutSlot; MAX_VERTEX_BUFFERS],

    /// Primitive topology
    primitive_topology: AtomicU8,

    /// Front face winding
    front_face: AtomicU8,

    /// Cull mode
    cull_mode: AtomicU8,

    /// Depth comparison function
    depth_compare: AtomicU8,

    /// Depth write enabled
    depth_write_enabled: AtomicBool,

    /// Stencil test enabled
    stencil_enabled: AtomicBool,

    /// Reserved/padding
    _reserved1: [u8; 2],

    /// Blend states (4 targets, 8B each = 32B)
    blend_states: [BlendState; MAX_COLOR_TARGETS],

    /// Bind group layout handles (4 slots, 8B each = 32B)
    bind_group_layouts: [AtomicU64; MAX_BIND_GROUPS],

    /// Reference count
    ref_count: AtomicU32,

    /// Padding to 512B
    /// 64 + 8 + 8 + 8 + 8 + 64 + 1 + 1 + 1 + 1 + 1 + 1 + 2 + 32 + 32 + 4 = 236
    /// 512 - 236 = 276
    _padding: [u8; 276],
}

const _: () = {
    assert!(core::mem::size_of::<KgpuRenderPipelineCapsule>() == 512);
    assert!(core::mem::align_of::<KgpuRenderPipelineCapsule>() == 512);
};

impl KgpuRenderPipelineCapsule {
    /// Create a new render pipeline in Building state.
    pub fn new() -> Self {
        let primary = ((PipelineState::Building as u64) << STATE_SHIFT) | 1;
        let secondary = 0u64;

        Self {
            handle: KgpuHandle::new(0, 1),
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(secondary),
            vertex_shader: AtomicU64::new(0),
            fragment_shader: AtomicU64::new(0),
            vertex_layouts: [
                VertexLayoutSlot::new(),
                VertexLayoutSlot::new(),
                VertexLayoutSlot::new(),
                VertexLayoutSlot::new(),
            ],
            primitive_topology: AtomicU8::new(PrimitiveTopology::TriangleList as u8),
            front_face: AtomicU8::new(FrontFace::Ccw as u8),
            cull_mode: AtomicU8::new(CullMode::None as u8),
            depth_compare: AtomicU8::new(CompareFunction::Always as u8),
            depth_write_enabled: AtomicBool::new(false),
            stencil_enabled: AtomicBool::new(false),
            _reserved1: [0; 2],
            blend_states: [
                BlendState::new(),
                BlendState::new(),
                BlendState::new(),
                BlendState::new(),
            ],
            bind_group_layouts: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            ref_count: AtomicU32::new(1),
            _padding: [0; 276],
        }
    }

    // ========================================================================
    // Shader Configuration
    // ========================================================================

    /// Set the vertex shader.
    pub fn set_vertex_shader(&self, handle: u64) {
        self.vertex_shader.store(handle, Ordering::Release);
        self.update_stage_count();
    }

    /// Set the fragment shader.
    pub fn set_fragment_shader(&self, handle: u64) {
        self.fragment_shader.store(handle, Ordering::Release);
        self.update_stage_count();
    }

    /// Get vertex shader handle.
    #[inline]
    pub fn vertex_shader(&self) -> u64 {
        self.vertex_shader.load(Ordering::Acquire)
    }

    /// Get fragment shader handle.
    #[inline]
    pub fn fragment_shader(&self) -> u64 {
        self.fragment_shader.load(Ordering::Acquire)
    }

    // ========================================================================
    // Vertex Layout Configuration
    // ========================================================================

    /// Set vertex layout for a buffer slot.
    pub fn set_vertex_layout(
        &self,
        index: u8,
        stride: u32,
        step_mode: VertexStepMode,
        attribute_count: u8,
    ) -> PipelineResult<()> {
        if index as usize >= MAX_VERTEX_BUFFERS {
            return Err(PipelineError::IndexOutOfRange);
        }

        self.vertex_layouts[index as usize].set(stride, step_mode, attribute_count);
        self.update_vb_count();
        Ok(())
    }

    /// Get vertex layout for a buffer slot.
    pub fn get_vertex_layout(&self, index: u8) -> Option<(u32, VertexStepMode, u8)> {
        if index as usize >= MAX_VERTEX_BUFFERS {
            return None;
        }

        let slot = &self.vertex_layouts[index as usize];
        Some((slot.stride(), slot.step_mode(), slot.attribute_count()))
    }

    // ========================================================================
    // Primitive State Configuration
    // ========================================================================

    /// Set primitive topology.
    pub fn set_primitive_topology(&self, topology: PrimitiveTopology) {
        self.primitive_topology.store(topology as u8, Ordering::Release);
    }

    /// Get primitive topology.
    #[inline]
    pub fn primitive_topology(&self) -> PrimitiveTopology {
        PrimitiveTopology::from_u8(self.primitive_topology.load(Ordering::Acquire))
            .unwrap_or(PrimitiveTopology::TriangleList)
    }

    /// Set front face winding.
    pub fn set_front_face(&self, front_face: FrontFace) {
        self.front_face.store(front_face as u8, Ordering::Release);
    }

    /// Get front face winding.
    #[inline]
    pub fn front_face(&self) -> FrontFace {
        FrontFace::from_u8(self.front_face.load(Ordering::Acquire)).unwrap_or(FrontFace::Ccw)
    }

    /// Set cull mode.
    pub fn set_cull_mode(&self, cull_mode: CullMode) {
        self.cull_mode.store(cull_mode as u8, Ordering::Release);
    }

    /// Get cull mode.
    #[inline]
    pub fn cull_mode(&self) -> CullMode {
        CullMode::from_u8(self.cull_mode.load(Ordering::Acquire)).unwrap_or(CullMode::None)
    }

    // ========================================================================
    // Depth/Stencil Configuration
    // ========================================================================

    /// Set depth state.
    pub fn set_depth_state(&self, compare: CompareFunction, write_enabled: bool) {
        self.depth_compare.store(compare as u8, Ordering::Release);
        self.depth_write_enabled.store(write_enabled, Ordering::Release);

        // Update flags
        let mut flags = self.flags();
        if compare != CompareFunction::Always || write_enabled {
            flags |= PIPELINE_FLAG_DEPTH_TEST;
        }
        if write_enabled {
            flags |= PIPELINE_FLAG_DEPTH_WRITE;
        }
        self.set_flags(flags);
    }

    /// Get depth compare function.
    #[inline]
    pub fn depth_compare(&self) -> CompareFunction {
        CompareFunction::from_u8(self.depth_compare.load(Ordering::Acquire))
            .unwrap_or(CompareFunction::Always)
    }

    /// Get depth write enabled.
    #[inline]
    pub fn depth_write_enabled(&self) -> bool {
        self.depth_write_enabled.load(Ordering::Acquire)
    }

    /// Set stencil enabled.
    pub fn set_stencil_enabled(&self, enabled: bool) {
        self.stencil_enabled.store(enabled, Ordering::Release);
        if enabled {
            self.add_flags(PIPELINE_FLAG_STENCIL_TEST);
        }
    }

    /// Get stencil enabled.
    #[inline]
    pub fn stencil_enabled(&self) -> bool {
        self.stencil_enabled.load(Ordering::Acquire)
    }

    // ========================================================================
    // Blend State Configuration
    // ========================================================================

    /// Set blend state for a color target.
    pub fn set_blend_state(&self, target: u8, state: &BlendState) -> PipelineResult<()> {
        if target as usize >= MAX_COLOR_TARGETS {
            return Err(PipelineError::IndexOutOfRange);
        }

        self.blend_states[target as usize].set(
            state.src_factor(),
            state.dst_factor(),
            state.operation(),
            BlendFactor::from_u8(state.src_alpha_factor.load(Ordering::Relaxed))
                .unwrap_or(BlendFactor::One),
            BlendFactor::from_u8(state.dst_alpha_factor.load(Ordering::Relaxed))
                .unwrap_or(BlendFactor::Zero),
            BlendOperation::from_u8(state.alpha_operation.load(Ordering::Relaxed))
                .unwrap_or(BlendOperation::Add),
        );

        self.add_flags(PIPELINE_FLAG_BLEND);
        self.update_ct_count();
        Ok(())
    }

    /// Enable alpha blending for a color target.
    pub fn enable_alpha_blend(&self, target: u8) -> PipelineResult<()> {
        if target as usize >= MAX_COLOR_TARGETS {
            return Err(PipelineError::IndexOutOfRange);
        }

        self.blend_states[target as usize].set_alpha_blend();
        self.add_flags(PIPELINE_FLAG_BLEND);
        self.update_ct_count();
        Ok(())
    }

    /// Get blend state for a color target.
    pub fn get_blend_state(&self, target: u8) -> Option<&BlendState> {
        if target as usize >= MAX_COLOR_TARGETS {
            return None;
        }
        Some(&self.blend_states[target as usize])
    }

    // ========================================================================
    // Bind Group Layout Configuration
    // ========================================================================

    /// Set bind group layout.
    pub fn set_bind_group_layout(&self, index: u8, layout_id: u64) -> PipelineResult<()> {
        if index as usize >= MAX_BIND_GROUPS {
            return Err(PipelineError::IndexOutOfRange);
        }

        self.bind_group_layouts[index as usize].store(layout_id, Ordering::Release);
        Ok(())
    }

    /// Get bind group layout.
    pub fn get_bind_group_layout(&self, index: u8) -> Option<u64> {
        if index as usize >= MAX_BIND_GROUPS {
            return None;
        }
        Some(self.bind_group_layouts[index as usize].load(Ordering::Acquire))
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Check if pipeline is valid (has required shaders).
    pub fn is_valid(&self) -> bool {
        self.vertex_shader() != 0
    }

    /// Get current state.
    #[inline]
    pub fn state(&self) -> PipelineState {
        let primary = self.primary.load(Ordering::Acquire);
        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;
        PipelineState::from_u8(state).unwrap_or(PipelineState::Destroyed)
    }

    /// Get generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Get shader stage count.
    #[inline]
    pub fn stage_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STAGE_COUNT_MASK) >> STAGE_COUNT_SHIFT) as u8
    }

    /// Get vertex buffer count.
    #[inline]
    pub fn vertex_buffer_count(&self) -> u8 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & VB_COUNT_MASK) >> VB_COUNT_SHIFT) as u8
    }

    /// Get color target count.
    #[inline]
    pub fn color_target_count(&self) -> u8 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & CT_COUNT_MASK) >> CT_COUNT_SHIFT) as u8
    }

    /// Get flags.
    #[inline]
    pub fn flags(&self) -> u64 {
        let secondary = self.secondary.load(Ordering::Acquire);
        secondary & FLAGS_MASK
    }

    /// Get handle reference.
    #[inline]
    pub fn handle(&self) -> &KgpuHandle<RenderPipeline> {
        &self.handle
    }

    // ========================================================================
    // Hash for Pipeline Caching
    // ========================================================================

    /// Compute a hash of the pipeline state for caching.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_HASH_DETERMINISTIC`: Hash is computed from immutable state
    pub fn hash(&self) -> u64 {
        // Simple FNV-1a hash of key pipeline state
        let mut hash = 0xcbf29ce484222325u64;

        let mix = |h: u64, val: u64| -> u64 {
            let h = h ^ val;
            h.wrapping_mul(0x100000001b3)
        };

        hash = mix(hash, self.vertex_shader());
        hash = mix(hash, self.fragment_shader());
        hash = mix(hash, self.primitive_topology() as u64);
        hash = mix(hash, self.front_face() as u64);
        hash = mix(hash, self.cull_mode() as u64);
        hash = mix(hash, self.depth_compare() as u64);
        hash = mix(hash, self.depth_write_enabled() as u64);
        hash = mix(hash, self.flags());

        hash
    }

    // ========================================================================
    // State Transitions
    // ========================================================================

    /// Finalize the pipeline (Building -> Ready).
    pub fn finalize(&self) -> PipelineResult<()> {
        if !self.is_valid() {
            return Err(PipelineError::MissingVertexShader);
        }

        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

            if state != PipelineState::Building as u8 {
                return Err(PipelineError::InvalidState);
            }

            let stage_count = (primary & STAGE_COUNT_MASK) >> STAGE_COUNT_SHIFT;
            let generation = (primary & GENERATION_MASK) + 1;

            let new_primary = ((PipelineState::Ready as u64) << STATE_SHIFT)
                | (stage_count << STAGE_COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Reference Counting
    // ========================================================================

    /// Increment reference count.
    #[inline]
    pub fn increment_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement reference count. Returns true if should destroy.
    #[inline]
    pub fn decrement_ref(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::AcqRel) == 1
    }

    /// Get reference count.
    #[inline]
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Acquire)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn update_stage_count(&self) {
        let count = if self.vertex_shader() != 0 { 1 } else { 0 }
            + if self.fragment_shader() != 0 { 1 } else { 0 };

        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let generation = primary & GENERATION_MASK;

            let new_primary =
                (state << STATE_SHIFT) | ((count as u64) << STAGE_COUNT_SHIFT) | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn update_vb_count(&self) {
        let count = self
            .vertex_layouts
            .iter()
            .filter(|l| l.is_configured())
            .count() as u8;

        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let ct_count = (secondary & CT_COUNT_MASK) >> CT_COUNT_SHIFT;
            let flags = secondary & FLAGS_MASK;

            let new_secondary =
                ((count as u64) << VB_COUNT_SHIFT) | (ct_count << CT_COUNT_SHIFT) | flags;

            if self
                .secondary
                .compare_exchange_weak(
                    secondary,
                    new_secondary,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn update_ct_count(&self) {
        // For simplicity, just count non-default blend states
        let count = self
            .blend_states
            .iter()
            .filter(|s| s.src_factor() != BlendFactor::One || s.dst_factor() != BlendFactor::Zero)
            .count() as u8;

        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let vb_count = (secondary & VB_COUNT_MASK) >> VB_COUNT_SHIFT;
            let flags = secondary & FLAGS_MASK;

            let new_secondary =
                (vb_count << VB_COUNT_SHIFT) | ((count as u64) << CT_COUNT_SHIFT) | flags;

            if self
                .secondary
                .compare_exchange_weak(
                    secondary,
                    new_secondary,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn set_flags(&self, flags: u64) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let vb_count = (secondary & VB_COUNT_MASK) >> VB_COUNT_SHIFT;
            let ct_count = (secondary & CT_COUNT_MASK) >> CT_COUNT_SHIFT;

            let new_secondary =
                (vb_count << VB_COUNT_SHIFT) | (ct_count << CT_COUNT_SHIFT) | (flags & FLAGS_MASK);

            if self
                .secondary
                .compare_exchange_weak(
                    secondary,
                    new_secondary,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn add_flags(&self, flags: u64) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let current_flags = secondary & FLAGS_MASK;
            let vb_count = (secondary & VB_COUNT_MASK) >> VB_COUNT_SHIFT;
            let ct_count = (secondary & CT_COUNT_MASK) >> CT_COUNT_SHIFT;

            let new_secondary = (vb_count << VB_COUNT_SHIFT)
                | (ct_count << CT_COUNT_SHIFT)
                | ((current_flags | flags) & FLAGS_MASK);

            if self
                .secondary
                .compare_exchange_weak(
                    secondary,
                    new_secondary,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }
}

impl Default for KgpuRenderPipelineCapsule {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for KgpuRenderPipelineCapsule {}
unsafe impl Sync for KgpuRenderPipelineCapsule {}

impl core::fmt::Debug for KgpuRenderPipelineCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KgpuRenderPipelineCapsule")
            .field("state", &self.state())
            .field("stage_count", &self.stage_count())
            .field("vertex_buffer_count", &self.vertex_buffer_count())
            .field("primitive_topology", &self.primitive_topology())
            .field("cull_mode", &self.cull_mode())
            .field("depth_compare", &self.depth_compare())
            .field("generation", &self.generation())
            .field("hash", &format_args!("0x{:016X}", self.hash()))
            .finish()
    }
}

// ============================================================================
// KgpuComputePipelineCapsule
// ============================================================================

/// GPU Compute Pipeline Capsule with Lockfree Atomics
///
/// Compute pipeline state for GPGPU workloads.
///
/// # Tier: T1 (Atomic)
/// # Size: 256B (cache-aligned)
#[repr(C, align(256))]
pub struct KgpuComputePipelineCapsule {
    /// Resource handle
    handle: KgpuHandle<ComputePipeline>,

    /// Primary: state(8) | workgroup_x(8) | workgroup_y(8) | workgroup_z(8) | generation(32)
    primary: AtomicU64,

    /// Secondary: shared_memory_size(32) | flags(32)
    secondary: AtomicU64,

    /// Compute shader handle
    compute_shader: AtomicU64,

    /// Bind group layout handles (4 slots, 8B each = 32B)
    bind_group_layouts: [AtomicU64; MAX_BIND_GROUPS],

    /// Reference count
    ref_count: AtomicU32,

    /// Padding to 256B
    /// 64 + 8 + 8 + 8 + 32 + 4 = 124
    /// 256 - 124 = 132
    _padding: [u8; 132],
}

const _: () = {
    assert!(core::mem::size_of::<KgpuComputePipelineCapsule>() == 256);
    assert!(core::mem::align_of::<KgpuComputePipelineCapsule>() == 256);
};

impl KgpuComputePipelineCapsule {
    /// Create a new compute pipeline in Building state.
    pub fn new() -> Self {
        let primary = ((PipelineState::Building as u64) << STATE_SHIFT) | 1;

        Self {
            handle: KgpuHandle::new(0, 1),
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(0),
            compute_shader: AtomicU64::new(0),
            bind_group_layouts: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            ref_count: AtomicU32::new(1),
            _padding: [0; 132],
        }
    }

    // ========================================================================
    // Shader Configuration
    // ========================================================================

    /// Set the compute shader.
    pub fn set_compute_shader(&self, handle: u64) {
        self.compute_shader.store(handle, Ordering::Release);
    }

    /// Get compute shader handle.
    #[inline]
    pub fn compute_shader(&self) -> u64 {
        self.compute_shader.load(Ordering::Acquire)
    }

    // ========================================================================
    // Workgroup Configuration
    // ========================================================================

    /// Set workgroup size.
    pub fn set_workgroup_size(&self, x: u8, y: u8, z: u8) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let generation = primary & COMPUTE_GEN_MASK;

            let new_primary = (state << STATE_SHIFT)
                | ((x as u64) << WORKGROUP_X_SHIFT)
                | ((y as u64) << WORKGROUP_Y_SHIFT)
                | ((z as u64) << WORKGROUP_Z_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Get workgroup size.
    #[inline]
    pub fn workgroup_size(&self) -> (u8, u8, u8) {
        let primary = self.primary.load(Ordering::Acquire);
        let x = ((primary & WORKGROUP_X_MASK) >> WORKGROUP_X_SHIFT) as u8;
        let y = ((primary & WORKGROUP_Y_MASK) >> WORKGROUP_Y_SHIFT) as u8;
        let z = ((primary & WORKGROUP_Z_MASK) >> WORKGROUP_Z_SHIFT) as u8;
        (x, y, z)
    }

    /// Set shared memory size.
    pub fn set_shared_memory_size(&self, size: u32) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let flags = secondary & FLAGS_MASK;

            let new_secondary = ((size as u64) << SHARED_MEM_SHIFT) | flags;

            if self
                .secondary
                .compare_exchange_weak(
                    secondary,
                    new_secondary,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Get shared memory size.
    #[inline]
    pub fn shared_memory_size(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & SHARED_MEM_MASK) >> SHARED_MEM_SHIFT) as u32
    }

    // ========================================================================
    // Bind Group Layout Configuration
    // ========================================================================

    /// Set bind group layout.
    pub fn set_bind_group_layout(&self, index: u8, layout_id: u64) -> PipelineResult<()> {
        if index as usize >= MAX_BIND_GROUPS {
            return Err(PipelineError::IndexOutOfRange);
        }

        self.bind_group_layouts[index as usize].store(layout_id, Ordering::Release);
        Ok(())
    }

    /// Get bind group layout.
    pub fn get_bind_group_layout(&self, index: u8) -> Option<u64> {
        if index as usize >= MAX_BIND_GROUPS {
            return None;
        }
        Some(self.bind_group_layouts[index as usize].load(Ordering::Acquire))
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Check if pipeline is valid.
    pub fn is_valid(&self) -> bool {
        self.compute_shader() != 0
    }

    /// Get current state.
    #[inline]
    pub fn state(&self) -> PipelineState {
        let primary = self.primary.load(Ordering::Acquire);
        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;
        PipelineState::from_u8(state).unwrap_or(PipelineState::Destroyed)
    }

    /// Get generation counter.
    #[inline]
    pub fn generation(&self) -> u32 {
        let primary = self.primary.load(Ordering::Acquire);
        (primary & COMPUTE_GEN_MASK) as u32
    }

    /// Get flags.
    #[inline]
    pub fn flags(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & FLAGS_MASK) as u32
    }

    /// Get handle reference.
    #[inline]
    pub fn handle(&self) -> &KgpuHandle<ComputePipeline> {
        &self.handle
    }

    // ========================================================================
    // Hash for Pipeline Caching
    // ========================================================================

    /// Compute a hash of the pipeline state for caching.
    pub fn hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;

        let mix = |h: u64, val: u64| -> u64 {
            let h = h ^ val;
            h.wrapping_mul(0x100000001b3)
        };

        hash = mix(hash, self.compute_shader());
        let (x, y, z) = self.workgroup_size();
        hash = mix(hash, x as u64 | ((y as u64) << 8) | ((z as u64) << 16));
        hash = mix(hash, self.shared_memory_size() as u64);

        hash
    }

    // ========================================================================
    // State Transitions
    // ========================================================================

    /// Finalize the pipeline (Building -> Ready).
    pub fn finalize(&self) -> PipelineResult<()> {
        if !self.is_valid() {
            return Err(PipelineError::MissingComputeShader);
        }

        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

            if state != PipelineState::Building as u8 {
                return Err(PipelineError::InvalidState);
            }

            let workgroup_x = (primary & WORKGROUP_X_MASK) >> WORKGROUP_X_SHIFT;
            let workgroup_y = (primary & WORKGROUP_Y_MASK) >> WORKGROUP_Y_SHIFT;
            let workgroup_z = (primary & WORKGROUP_Z_MASK) >> WORKGROUP_Z_SHIFT;
            let generation = (primary & COMPUTE_GEN_MASK) + 1;

            let new_primary = ((PipelineState::Ready as u64) << STATE_SHIFT)
                | (workgroup_x << WORKGROUP_X_SHIFT)
                | (workgroup_y << WORKGROUP_Y_SHIFT)
                | (workgroup_z << WORKGROUP_Z_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Reference Counting
    // ========================================================================

    /// Increment reference count.
    #[inline]
    pub fn increment_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement reference count. Returns true if should destroy.
    #[inline]
    pub fn decrement_ref(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::AcqRel) == 1
    }

    /// Get reference count.
    #[inline]
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Acquire)
    }
}

impl Default for KgpuComputePipelineCapsule {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for KgpuComputePipelineCapsule {}
unsafe impl Sync for KgpuComputePipelineCapsule {}

impl core::fmt::Debug for KgpuComputePipelineCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (x, y, z) = self.workgroup_size();
        f.debug_struct("KgpuComputePipelineCapsule")
            .field("state", &self.state())
            .field("workgroup_size", &format_args!("({}, {}, {})", x, y, z))
            .field("shared_memory_size", &self.shared_memory_size())
            .field("generation", &self.generation())
            .field("hash", &format_args!("0x{:016X}", self.hash()))
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Size and Alignment Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_size() {
        assert_eq!(
            core::mem::size_of::<KgpuRenderPipelineCapsule>(),
            512,
            "KgpuRenderPipelineCapsule must be 512 bytes"
        );
    }

    #[test]
    fn test_render_pipeline_alignment() {
        assert_eq!(
            core::mem::align_of::<KgpuRenderPipelineCapsule>(),
            512,
            "KgpuRenderPipelineCapsule must have 512-byte alignment"
        );
    }

    #[test]
    fn test_compute_pipeline_size() {
        assert_eq!(
            core::mem::size_of::<KgpuComputePipelineCapsule>(),
            256,
            "KgpuComputePipelineCapsule must be 256 bytes"
        );
    }

    #[test]
    fn test_compute_pipeline_alignment() {
        assert_eq!(
            core::mem::align_of::<KgpuComputePipelineCapsule>(),
            256,
            "KgpuComputePipelineCapsule must have 256-byte alignment"
        );
    }

    #[test]
    fn test_vertex_layout_slot_size() {
        assert_eq!(core::mem::size_of::<VertexLayoutSlot>(), 16);
    }

    #[test]
    fn test_blend_state_size() {
        assert_eq!(core::mem::size_of::<BlendState>(), 8);
    }

    // ========================================================================
    // Render Pipeline Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_new() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        assert_eq!(pipeline.state(), PipelineState::Building);
        assert_eq!(pipeline.generation(), 1);
        assert_eq!(pipeline.ref_count(), 1);
        assert!(!pipeline.is_valid());
    }

    #[test]
    fn test_render_pipeline_set_shaders() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        pipeline.set_vertex_shader(0x1234);
        pipeline.set_fragment_shader(0x5678);

        assert_eq!(pipeline.vertex_shader(), 0x1234);
        assert_eq!(pipeline.fragment_shader(), 0x5678);
        assert_eq!(pipeline.stage_count(), 2);
        assert!(pipeline.is_valid());
    }

    #[test]
    fn test_render_pipeline_vertex_layout() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        pipeline
            .set_vertex_layout(0, 32, VertexStepMode::Vertex, 3)
            .unwrap();
        pipeline
            .set_vertex_layout(1, 16, VertexStepMode::Instance, 2)
            .unwrap();

        let (stride, mode, count) = pipeline.get_vertex_layout(0).unwrap();
        assert_eq!(stride, 32);
        assert_eq!(mode, VertexStepMode::Vertex);
        assert_eq!(count, 3);

        let (stride, mode, count) = pipeline.get_vertex_layout(1).unwrap();
        assert_eq!(stride, 16);
        assert_eq!(mode, VertexStepMode::Instance);
        assert_eq!(count, 2);

        assert_eq!(pipeline.vertex_buffer_count(), 2);
    }

    #[test]
    fn test_render_pipeline_primitive_state() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        pipeline.set_primitive_topology(PrimitiveTopology::TriangleStrip);
        pipeline.set_front_face(FrontFace::Cw);
        pipeline.set_cull_mode(CullMode::Back);

        assert_eq!(pipeline.primitive_topology(), PrimitiveTopology::TriangleStrip);
        assert_eq!(pipeline.front_face(), FrontFace::Cw);
        assert_eq!(pipeline.cull_mode(), CullMode::Back);
    }

    #[test]
    fn test_render_pipeline_depth_state() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        pipeline.set_depth_state(CompareFunction::Less, true);

        assert_eq!(pipeline.depth_compare(), CompareFunction::Less);
        assert!(pipeline.depth_write_enabled());
        assert!((pipeline.flags() & PIPELINE_FLAG_DEPTH_TEST) != 0);
        assert!((pipeline.flags() & PIPELINE_FLAG_DEPTH_WRITE) != 0);
    }

    #[test]
    fn test_render_pipeline_alpha_blend() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        pipeline.enable_alpha_blend(0).unwrap();

        let state = pipeline.get_blend_state(0).unwrap();
        assert!(state.is_alpha_blend());
        assert!((pipeline.flags() & PIPELINE_FLAG_BLEND) != 0);
    }

    #[test]
    fn test_render_pipeline_finalize() {
        let pipeline = KgpuRenderPipelineCapsule::new();
        pipeline.set_vertex_shader(0x1234);

        pipeline.finalize().unwrap();

        assert_eq!(pipeline.state(), PipelineState::Ready);
        assert_eq!(pipeline.generation(), 2);
    }

    #[test]
    fn test_render_pipeline_finalize_missing_shader() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        let result = pipeline.finalize();

        assert_eq!(result, Err(PipelineError::MissingVertexShader));
    }

    #[test]
    fn test_render_pipeline_hash_deterministic() {
        let pipeline1 = KgpuRenderPipelineCapsule::new();
        pipeline1.set_vertex_shader(0x1234);
        pipeline1.set_fragment_shader(0x5678);
        pipeline1.set_primitive_topology(PrimitiveTopology::TriangleList);

        let pipeline2 = KgpuRenderPipelineCapsule::new();
        pipeline2.set_vertex_shader(0x1234);
        pipeline2.set_fragment_shader(0x5678);
        pipeline2.set_primitive_topology(PrimitiveTopology::TriangleList);

        assert_eq!(pipeline1.hash(), pipeline2.hash());
    }

    #[test]
    fn test_render_pipeline_hash_different() {
        let pipeline1 = KgpuRenderPipelineCapsule::new();
        pipeline1.set_vertex_shader(0x1234);

        let pipeline2 = KgpuRenderPipelineCapsule::new();
        pipeline2.set_vertex_shader(0x5678);

        assert_ne!(pipeline1.hash(), pipeline2.hash());
    }

    // ========================================================================
    // Compute Pipeline Tests
    // ========================================================================

    #[test]
    fn test_compute_pipeline_new() {
        let pipeline = KgpuComputePipelineCapsule::new();

        assert_eq!(pipeline.state(), PipelineState::Building);
        assert_eq!(pipeline.generation(), 1);
        assert_eq!(pipeline.ref_count(), 1);
        assert!(!pipeline.is_valid());
    }

    #[test]
    fn test_compute_pipeline_set_shader() {
        let pipeline = KgpuComputePipelineCapsule::new();

        pipeline.set_compute_shader(0xABCD);

        assert_eq!(pipeline.compute_shader(), 0xABCD);
        assert!(pipeline.is_valid());
    }

    #[test]
    fn test_compute_pipeline_workgroup_size() {
        let pipeline = KgpuComputePipelineCapsule::new();

        pipeline.set_workgroup_size(16, 8, 4);

        let (x, y, z) = pipeline.workgroup_size();
        assert_eq!(x, 16);
        assert_eq!(y, 8);
        assert_eq!(z, 4);
    }

    #[test]
    fn test_compute_pipeline_shared_memory() {
        let pipeline = KgpuComputePipelineCapsule::new();

        pipeline.set_shared_memory_size(16384);

        assert_eq!(pipeline.shared_memory_size(), 16384);
    }

    #[test]
    fn test_compute_pipeline_finalize() {
        let pipeline = KgpuComputePipelineCapsule::new();
        pipeline.set_compute_shader(0xABCD);
        pipeline.set_workgroup_size(64, 1, 1);

        pipeline.finalize().unwrap();

        assert_eq!(pipeline.state(), PipelineState::Ready);
    }

    #[test]
    fn test_compute_pipeline_finalize_missing_shader() {
        let pipeline = KgpuComputePipelineCapsule::new();

        let result = pipeline.finalize();

        assert_eq!(result, Err(PipelineError::MissingComputeShader));
    }

    #[test]
    fn test_compute_pipeline_hash() {
        let pipeline1 = KgpuComputePipelineCapsule::new();
        pipeline1.set_compute_shader(0xABCD);
        pipeline1.set_workgroup_size(64, 1, 1);

        let pipeline2 = KgpuComputePipelineCapsule::new();
        pipeline2.set_compute_shader(0xABCD);
        pipeline2.set_workgroup_size(64, 1, 1);

        assert_eq!(pipeline1.hash(), pipeline2.hash());
    }

    // ========================================================================
    // Reference Counting Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_refcount() {
        let pipeline = KgpuRenderPipelineCapsule::new();
        assert_eq!(pipeline.ref_count(), 1);

        pipeline.increment_ref();
        assert_eq!(pipeline.ref_count(), 2);

        assert!(!pipeline.decrement_ref());
        assert_eq!(pipeline.ref_count(), 1);

        assert!(pipeline.decrement_ref());
        assert_eq!(pipeline.ref_count(), 0);
    }

    #[test]
    fn test_compute_pipeline_refcount() {
        let pipeline = KgpuComputePipelineCapsule::new();
        assert_eq!(pipeline.ref_count(), 1);

        pipeline.increment_ref();
        assert_eq!(pipeline.ref_count(), 2);

        assert!(!pipeline.decrement_ref());
        assert!(pipeline.decrement_ref());
    }

    // ========================================================================
    // Enum Tests
    // ========================================================================

    #[test]
    fn test_primitive_topology_from_u8() {
        assert_eq!(
            PrimitiveTopology::from_u8(0),
            Some(PrimitiveTopology::PointList)
        );
        assert_eq!(
            PrimitiveTopology::from_u8(3),
            Some(PrimitiveTopology::TriangleList)
        );
        assert_eq!(PrimitiveTopology::from_u8(100), None);
    }

    #[test]
    fn test_compare_function_from_u8() {
        assert_eq!(CompareFunction::from_u8(0), Some(CompareFunction::Never));
        assert_eq!(CompareFunction::from_u8(1), Some(CompareFunction::Less));
        assert_eq!(CompareFunction::from_u8(7), Some(CompareFunction::Always));
        assert_eq!(CompareFunction::from_u8(100), None);
    }

    #[test]
    fn test_blend_factor_from_u8() {
        assert_eq!(BlendFactor::from_u8(0), Some(BlendFactor::Zero));
        assert_eq!(BlendFactor::from_u8(1), Some(BlendFactor::One));
        assert_eq!(BlendFactor::from_u8(6), Some(BlendFactor::SrcAlpha));
        assert_eq!(BlendFactor::from_u8(100), None);
    }

    // ========================================================================
    // Thread Safety Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuRenderPipelineCapsule>();
    }

    #[test]
    fn test_compute_pipeline_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuComputePipelineCapsule>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_shader_updates() {
        use std::sync::Arc;
        use std::thread;

        let pipeline = Arc::new(KgpuRenderPipelineCapsule::new());

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = Arc::clone(&pipeline);
                thread::spawn(move || {
                    for j in 0..100 {
                        if i % 2 == 0 {
                            p.set_vertex_shader((i * 1000 + j) as u64);
                        } else {
                            p.set_fragment_shader((i * 1000 + j) as u64);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // No panics = success
    }

    // ========================================================================
    // Debug Format Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_debug() {
        let pipeline = KgpuRenderPipelineCapsule::new();
        pipeline.set_vertex_shader(0x1234);
        let debug_str = format!("{:?}", pipeline);

        assert!(debug_str.contains("KgpuRenderPipelineCapsule"));
        assert!(debug_str.contains("Building"));
    }

    #[test]
    fn test_compute_pipeline_debug() {
        let pipeline = KgpuComputePipelineCapsule::new();
        pipeline.set_compute_shader(0x1234);
        pipeline.set_workgroup_size(64, 1, 1);
        let debug_str = format!("{:?}", pipeline);

        assert!(debug_str.contains("KgpuComputePipelineCapsule"));
        assert!(debug_str.contains("(64, 1, 1)"));
    }

    // ========================================================================
    // Full Workflow Tests
    // ========================================================================

    #[test]
    fn test_render_pipeline_full_workflow() {
        let pipeline = KgpuRenderPipelineCapsule::new();

        // Configure shaders
        pipeline.set_vertex_shader(0x1000);
        pipeline.set_fragment_shader(0x2000);

        // Configure vertex layout
        pipeline
            .set_vertex_layout(0, 32, VertexStepMode::Vertex, 3)
            .unwrap();

        // Configure primitive state
        pipeline.set_primitive_topology(PrimitiveTopology::TriangleList);
        pipeline.set_cull_mode(CullMode::Back);

        // Configure depth
        pipeline.set_depth_state(CompareFunction::Less, true);

        // Configure blend
        pipeline.enable_alpha_blend(0).unwrap();

        // Configure bind groups
        pipeline.set_bind_group_layout(0, 0x100).unwrap();

        // Finalize
        pipeline.finalize().unwrap();

        // Verify
        assert_eq!(pipeline.state(), PipelineState::Ready);
        assert_eq!(pipeline.stage_count(), 2);
        assert_eq!(pipeline.vertex_buffer_count(), 1);
        assert!(pipeline.is_valid());
    }

    #[test]
    fn test_compute_pipeline_full_workflow() {
        let pipeline = KgpuComputePipelineCapsule::new();

        // Configure shader
        pipeline.set_compute_shader(0x3000);

        // Configure workgroup
        pipeline.set_workgroup_size(64, 4, 1); // 256 threads total (64*4*1)
        pipeline.set_shared_memory_size(32768);

        // Configure bind groups
        pipeline.set_bind_group_layout(0, 0x200).unwrap();
        pipeline.set_bind_group_layout(1, 0x201).unwrap();

        // Finalize
        pipeline.finalize().unwrap();

        // Verify
        assert_eq!(pipeline.state(), PipelineState::Ready);
        assert!(pipeline.is_valid());
        assert_eq!(pipeline.workgroup_size(), (64, 4, 1));
        assert_eq!(pipeline.shared_memory_size(), 32768);
    }
}
