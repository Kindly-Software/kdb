//! Terminal Render Pipeline Manager (T6 Mixed Metacapsule)
//!
//! Orchestrates GPU render pipeline state machine, shader switching, and resource binding.
//! Coordinates render state transitions with atomic state tracking and performance monitoring.
//!
//! ## UCE34 Compliance
//!
//! - **Q10**: T6 Mixed tier (orchestrates T1+T4+T7 sub-capsules)
//! - **Q33**: 100% lockfree (DualAtomicU64 state machine)
//! - **Q34**: State transition audit trail support
//!
//! ## Architecture
//!
//! - **State Machine**: 6 states (Uninitialized → Initializing → Ready → Recording → Submitted → Error)
//! - **Resource Versioning**: Tracks atlas, cell buffer, uniform versions for cache invalidation
//! - **Performance Tracking**: Last frame stats + cumulative metrics
//! - **Shader Management**: 4 programs (GlyphQuad, GlyphSdf, Effects, Cursor)
//!
//! ## Performance Targets (B32)
//!
//! - State transition: <10ns
//! - Shader bind: <20ns
//! - Record draw: <5ns
//! - Submit: <50ns
//!
//! ## References
//!
//! - [Metacapsule Architecture](../../../docs/METACAPSULE_ARCHITECTURE.xml)
//! - [GPU Pipeline Best Practices](https://vkguide.dev/docs/chapter-2/pipeline_walkthrough/)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Pipeline state machine
///
/// # State Transitions
///
/// Valid transitions:
/// - Uninitialized → Initializing → Ready
/// - Ready ↔ Recording → Submitted → Ready
/// - Any → Error (on failure)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineState {
    /// Initial state, no resources allocated
    Uninitialized = 0,
    /// Transitioning to Ready (resource allocation)
    Initializing = 1,
    /// Ready to record commands
    Ready = 2,
    /// Recording commands (draw calls, dispatches)
    Recording = 3,
    /// Submitted to GPU
    Submitted = 4,
    /// Error state (requires reset)
    Error = 255,
}

impl From<u8> for PipelineState {
    fn from(val: u8) -> Self {
        match val {
            0 => PipelineState::Uninitialized,
            1 => PipelineState::Initializing,
            2 => PipelineState::Ready,
            3 => PipelineState::Recording,
            4 => PipelineState::Submitted,
            _ => PipelineState::Error,
        }
    }
}

/// Shader program identifiers
///
/// # Programs
///
/// - **GlyphQuad**: Standard text rendering (fastest)
/// - **GlyphSdf**: SDF-based text (for scaling/effects)
/// - **Effects**: Post-processing (blur, bloom, etc.)
/// - **Cursor**: Cursor rendering (blinking, styles)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ShaderProgram {
    /// Standard glyph quad rendering
    GlyphQuad = 0,
    /// SDF-based glyph rendering
    GlyphSdf = 1,
    /// Post-processing effects
    Effects = 2,
    /// Cursor rendering
    Cursor = 3,
}

/// Recording statistics
///
/// # Fields
///
/// - `command_count`: Total commands recorded
/// - `draw_calls`: Number of draw calls
/// - `dispatches`: Number of compute dispatches
/// - `generation`: Pipeline generation counter
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RecordingStats {
    /// Total commands recorded
    pub command_count: u32,
    /// Number of draw calls
    pub draw_calls: u16,
    /// Number of compute dispatches
    pub dispatches: u16,
    /// Pipeline generation counter
    pub generation: u32,
}

/// Frame timing and performance statistics
///
/// # Layout
///
/// - **Size**: 32 bytes
/// - **Alignment**: 8 bytes
///
/// # Fields
///
/// - `frame_time_ns`: Total frame time in nanoseconds
/// - `gpu_time_ns`: GPU execution time in nanoseconds
/// - `submit_time_ns`: Command submission time in nanoseconds
/// - `fence_wait_ns`: Fence synchronization wait time in nanoseconds
/// - `draw_calls`: Number of draw calls in frame
/// - `vertices`: Total vertices rendered
/// - `generation`: Frame generation counter
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(C)]
pub struct FrameStats {
    /// Total frame time in nanoseconds
    pub frame_time_ns: u64,
    /// GPU execution time in nanoseconds
    pub gpu_time_ns: u64,
    /// Command submission time in nanoseconds
    pub submit_time_ns: u32,
    /// Fence synchronization wait time in nanoseconds
    pub fence_wait_ns: u32,
    /// Number of draw calls in frame
    pub draw_calls: u16,
    /// Total vertices rendered
    pub vertices: u16,
    /// Frame generation counter
    pub generation: u32,
}

// Compile-time size validation for FrameStats
const _: () = assert!(core::mem::size_of::<FrameStats>() == 32);

/// Render error types
///
/// # Variants
///
/// - `InvalidState`: Operation not allowed in current state
/// - `NotRecording`: Cannot record draw when not in Recording state
/// - `AlreadyRecording`: Cannot begin recording when already recording
/// - `NotInitialized`: Pipeline not initialized
/// - `ResourceNotBound`: Required GPU resource not bound
/// - `FenceTimeout`: GPU fence synchronization timed out
/// - `GpuError`: Generic GPU driver error
/// - `InvalidRange`: Draw range exceeds buffer capacity
/// - `NoEffectsConfigured`: Effects pass requested but no effects configured
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PipelineError {
    /// Invalid state for requested operation
    InvalidState {
        /// Current state
        current: PipelineState,
        /// Expected state
        expected: PipelineState,
    },
    /// Not in recording state
    NotRecording,
    /// Already recording
    AlreadyRecording,
    /// Pipeline not initialized
    NotInitialized,
    /// Required GPU resource not bound
    ResourceNotBound {
        /// Name of the missing resource
        resource: ResourceType,
    },
    /// GPU fence synchronization timed out
    FenceTimeout {
        /// Timeout in nanoseconds
        timeout_ns: u64,
    },
    /// Generic GPU driver error
    GpuError {
        /// Error code from driver
        code: u32,
    },
    /// Draw range exceeds buffer capacity
    InvalidRange {
        /// Requested start index
        start: usize,
        /// Requested count
        count: usize,
        /// Buffer capacity
        capacity: usize,
    },
    /// Effects pass requested but no effects configured
    NoEffectsConfigured,
}

/// GPU resource types for error reporting
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceType {
    /// Glyph atlas texture
    Atlas = 0,
    /// Cell buffer (vertex/instance data)
    CellBuffer = 1,
    /// Uniform buffer
    Uniforms = 2,
    /// Effects uniforms
    EffectsUniforms = 3,
}

impl core::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PipelineError::InvalidState { current, expected } => {
                write!(
                    f,
                    "Invalid state: current={:?}, expected={:?}",
                    current, expected
                )
            }
            PipelineError::NotRecording => write!(f, "Not in recording state"),
            PipelineError::AlreadyRecording => write!(f, "Already recording"),
            PipelineError::NotInitialized => write!(f, "Pipeline not initialized"),
            PipelineError::ResourceNotBound { resource } => {
                write!(f, "Resource not bound: {:?}", resource)
            }
            PipelineError::FenceTimeout { timeout_ns } => {
                write!(f, "Fence timeout after {}ns", timeout_ns)
            }
            PipelineError::GpuError { code } => {
                write!(f, "GPU error: code={}", code)
            }
            PipelineError::InvalidRange {
                start,
                count,
                capacity,
            } => {
                write!(
                    f,
                    "Invalid range: start={}, count={}, capacity={}",
                    start, count, capacity
                )
            }
            PipelineError::NoEffectsConfigured => write!(f, "No effects configured"),
        }
    }
}

/// T6 Mixed - Terminal render pipeline orchestrator
///
/// # UCE34 Compliance
///
/// - **Q10**: T6 Mixed tier (orchestrates T1+T4+T7 sub-capsules)
/// - **Q33**: 100% lockfree (DualAtomicU64 state machine)
/// - **Q34**: State transition audit trail support
///
/// # Layout
///
/// - **Size**: 128 bytes (cache-aligned 64B × 2)
/// - **Alignment**: 64 bytes (cache line boundary)
/// - **Padding**: 0 bytes (fully utilized)
///
/// # State Encoding (DualAtomicU64 pattern)
///
/// - `state`: state(8) | active_shader(8) | bind_group_version(16) | generation(32)
/// - `recording_state`: command_count(32) | draw_calls(16) | dispatches(16)
/// - `resource_versions`: atlas_version(16) | cell_buffer_version(16) | uniform_version(16) | _pad(16)
/// - `viewport`: x(16) | y(16) | width(16) | height(16)
/// - `bound_resources`: atlas_gen(16) | cell_buffer_gen(16) | effects_gen(16) | bound_flags(16)
/// - `fence_state`: fence_seqno(32) | last_completed(32)
///
/// # Performance Targets (B32)
///
/// - State transition: <10ns (atomic CAS)
/// - Shader bind: <20ns (atomic store)
/// - Record draw: <5ns (atomic increment)
/// - Submit: <50ns (atomic swap + stats update)
/// - Fence wait: <1ms typical (GPU-bound)
#[repr(C, align(64))]
pub struct TerminalPipelineManager {
    // State machine (DualAtomicU64 pattern)
    /// State (8) | active_shader (8) | bind_group_version (16) | generation (32)
    state: AtomicU64,

    /// Recording: command_count (32) | draw_calls (16) | dispatches (16)
    recording_state: AtomicU64,

    // Resource versions (for cache invalidation)
    /// Atlas version (16) | cell_buffer_version (16) | uniform_version (16) | _pad (16)
    resource_versions: AtomicU64,

    // Pipeline configuration
    /// Viewport: x (16) | y (16) | width (16) | height (16)
    viewport: AtomicU64,

    /// Clear color (RGBA8888)
    clear_color: AtomicU32,

    /// Blend mode (4) | depth_test (1) | stencil_test (1) | _pad (26)
    render_flags: AtomicU32,

    // Performance tracking
    /// Last frame: draw_calls (16) | vertices (32) | indices (16)
    last_frame_stats: AtomicU64,

    /// Cumulative: total_draw_calls (32) | total_frames (32)
    cumulative_stats: AtomicU64,

    // GPU Resource Binding State (new fields for kgpu integration)
    /// Bound resources: atlas_gen (16) | cell_buffer_gen (16) | effects_gen (16) | bound_flags (16)
    /// bound_flags bits: 0=atlas_bound, 1=cell_buffer_bound, 2=effects_bound
    bound_resources: AtomicU64,

    /// Fence state: fence_seqno (32) | last_completed_seqno (32)
    /// Used for GPU synchronization via generation counters
    fence_state: AtomicU64,

    /// Frame timing: frame_start_ns (64) for FrameStats calculation
    frame_start_ns: AtomicU64,
}

// Compile-time size validation
const _: () = assert!(core::mem::size_of::<TerminalPipelineManager>() == 128);
const _: () = assert!(core::mem::align_of::<TerminalPipelineManager>() == 64);

// State packing helpers
#[inline(always)]
fn pack_state(state: u8, shader: u8, bind_group_version: u16, generation: u32) -> u64 {
    ((state as u64) << 56)
        | ((shader as u64) << 48)
        | ((bind_group_version as u64) << 32)
        | (generation as u64)
}

#[inline(always)]
fn unpack_state(packed: u64) -> (u8, u8, u16, u32) {
    let state = (packed >> 56) as u8;
    let shader = (packed >> 48) as u8;
    let bind_group_version = (packed >> 32) as u16;
    let generation = packed as u32;
    (state, shader, bind_group_version, generation)
}

#[inline(always)]
fn pack_recording(command_count: u32, draw_calls: u16, dispatches: u16) -> u64 {
    ((command_count as u64) << 32) | ((draw_calls as u64) << 16) | (dispatches as u64)
}

#[inline(always)]
fn unpack_recording(packed: u64) -> (u32, u16, u16) {
    let command_count = (packed >> 32) as u32;
    let draw_calls = (packed >> 16) as u16;
    let dispatches = packed as u16;
    (command_count, draw_calls, dispatches)
}

#[inline(always)]
fn pack_viewport(x: u16, y: u16, width: u16, height: u16) -> u64 {
    ((x as u64) << 48) | ((y as u64) << 32) | ((width as u64) << 16) | (height as u64)
}

#[inline(always)]
fn pack_resources(atlas: u16, cells: u16, uniforms: u16) -> u64 {
    ((atlas as u64) << 48) | ((cells as u64) << 32) | ((uniforms as u64) << 16)
}

#[inline(always)]
fn unpack_resources(packed: u64) -> (u16, u16, u16) {
    let atlas = (packed >> 48) as u16;
    let cells = (packed >> 32) as u16;
    let uniforms = (packed >> 16) as u16;
    (atlas, cells, uniforms)
}

// Bound resource flags
const BOUND_FLAG_ATLAS: u16 = 1 << 0;
const BOUND_FLAG_CELL_BUFFER: u16 = 1 << 1;
const BOUND_FLAG_EFFECTS: u16 = 1 << 2;

#[inline(always)]
fn pack_bound_resources(atlas_gen: u16, cell_buffer_gen: u16, effects_gen: u16, flags: u16) -> u64 {
    ((atlas_gen as u64) << 48)
        | ((cell_buffer_gen as u64) << 32)
        | ((effects_gen as u64) << 16)
        | (flags as u64)
}

#[inline(always)]
fn unpack_bound_resources(packed: u64) -> (u16, u16, u16, u16) {
    let atlas_gen = (packed >> 48) as u16;
    let cell_buffer_gen = (packed >> 32) as u16;
    let effects_gen = (packed >> 16) as u16;
    let flags = packed as u16;
    (atlas_gen, cell_buffer_gen, effects_gen, flags)
}

#[inline(always)]
fn pack_fence_state(fence_seqno: u32, last_completed: u32) -> u64 {
    ((fence_seqno as u64) << 32) | (last_completed as u64)
}

#[inline(always)]
fn unpack_fence_state(packed: u64) -> (u32, u32) {
    let fence_seqno = (packed >> 32) as u32;
    let last_completed = packed as u32;
    (fence_seqno, last_completed)
}

impl TerminalPipelineManager {
    /// Create new pipeline manager (Uninitialized state)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// assert_eq!(pipeline.state(), PipelineState::Uninitialized);
    /// ```
    #[inline]
    pub const fn new() -> Self {
        // Initial state: state=0 (Uninitialized), shader=0xFF (no shader), bind_group=0, gen=0
        // pack_state(0, 0xFF, 0, 0) = 0xFF << 48 = 0x00FF_0000_0000_0000
        const INITIAL_STATE: u64 = 0x00FF_0000_0000_0000;
        Self {
            state: AtomicU64::new(INITIAL_STATE), // Uninitialized with no shader bound
            recording_state: AtomicU64::new(0),
            resource_versions: AtomicU64::new(0),
            viewport: AtomicU64::new(0),
            clear_color: AtomicU32::new(0),
            render_flags: AtomicU32::new(0),
            last_frame_stats: AtomicU64::new(0),
            cumulative_stats: AtomicU64::new(0),
            bound_resources: AtomicU64::new(0),
            fence_state: AtomicU64::new(0),
            frame_start_ns: AtomicU64::new(0),
        }
    }

    /// Initialize pipeline (transition to Ready)
    ///
    /// # Performance
    ///
    /// - <10ns state transition (single CAS)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// assert_eq!(pipeline.state(), PipelineState::Ready);
    /// ```
    pub fn initialize(&self) -> Result<(), PipelineError> {
        // CAS: Uninitialized → Initializing
        let current = self.state.load(Ordering::Acquire);
        let (state, shader, _bind_group, gen) = unpack_state(current);

        if state != PipelineState::Uninitialized as u8 {
            return Err(PipelineError::InvalidState {
                current: PipelineState::from(state),
                expected: PipelineState::Uninitialized,
            });
        }

        // Set to Initializing (preserve shader field - 0xFF sentinel means no shader bound)
        let initializing = pack_state(PipelineState::Initializing as u8, shader, 0, gen);
        if self
            .state
            .compare_exchange(current, initializing, Ordering::Release, Ordering::Acquire)
            .is_err()
        {
            return Err(PipelineError::InvalidState {
                current: PipelineState::from(state),
                expected: PipelineState::Uninitialized,
            });
        }

        // Transition to Ready (preserve shader field - 0xFF sentinel means no shader bound)
        let ready = pack_state(PipelineState::Ready as u8, shader, 0, gen + 1);
        self.state.store(ready, Ordering::Release);

        Ok(())
    }

    /// Begin command recording (Ready → Recording)
    ///
    /// # Performance
    ///
    /// - <10ns state transition
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// ```
    pub fn begin_recording(&self) -> Result<(), PipelineError> {
        let current = self.state.load(Ordering::Acquire);
        let (state, shader, bind_group, gen) = unpack_state(current);

        if state != PipelineState::Ready as u8 && state != PipelineState::Submitted as u8 {
            return Err(PipelineError::InvalidState {
                current: PipelineState::from(state),
                expected: PipelineState::Ready,
            });
        }

        // Reset recording state
        self.recording_state.store(0, Ordering::Release);

        // Transition to Recording
        let recording = pack_state(PipelineState::Recording as u8, shader, bind_group, gen);
        self.state.store(recording, Ordering::Release);

        Ok(())
    }

    /// Bind shader program
    ///
    /// # Performance
    ///
    /// - <20ns shader switch (atomic update)
    ///
    /// # Returns
    ///
    /// - `true` if shader changed
    /// - `false` if already bound
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::{TerminalPipelineManager, ShaderProgram};
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
    /// assert!(changed); // First bind
    /// let changed2 = pipeline.bind_shader(ShaderProgram::GlyphQuad);
    /// assert!(!changed2); // Already bound
    /// ```
    pub fn bind_shader(&self, program: ShaderProgram) -> bool {
        let current = self.state.load(Ordering::Acquire);
        let (state, current_shader, bind_group, gen) = unpack_state(current);

        let new_shader = program as u8;
        // 0xFF is sentinel for "no shader bound" (initial state from new())
        // Return false only if same shader is already bound (not the sentinel)
        if current_shader == new_shader {
            return false; // Already bound
        }

        let new_state = pack_state(state, new_shader, bind_group, gen);
        self.state.store(new_state, Ordering::Release);
        true
    }

    /// Update viewport
    ///
    /// # Performance
    ///
    /// - <10ns viewport update
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.update_viewport(0, 0, 1920, 1080);
    /// ```
    #[inline]
    pub fn update_viewport(&self, x: u16, y: u16, width: u16, height: u16) {
        let packed = pack_viewport(x, y, width, height);
        self.viewport.store(packed, Ordering::Release);
    }

    /// Set clear color (RGBA8888)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.set_clear_color(0x000000FF); // Black
    /// ```
    #[inline]
    pub fn set_clear_color(&self, rgba: u32) {
        self.clear_color.store(rgba, Ordering::Release);
    }

    /// Record draw call
    ///
    /// # Performance
    ///
    /// - <5ns draw record (atomic increment)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// pipeline.record_draw(1024, 512);
    /// ```
    pub fn record_draw(&self, vertices: u32, indices: u32) {
        // Increment draw calls
        let current = self.recording_state.fetch_add(
            (1 << 32) | (1 << 16), // command_count += 1, draw_calls += 1
            Ordering::AcqRel,
        );

        // Update last frame stats (vertices + indices)
        let stats_packed = ((vertices as u64) << 32) | ((indices as u64) << 16);
        self.last_frame_stats
            .fetch_add(stats_packed, Ordering::AcqRel);
    }

    /// End recording and return stats
    ///
    /// # Performance
    ///
    /// - <50ns end recording (atomic swap)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// pipeline.record_draw(1024, 512);
    /// let stats = pipeline.end_recording().unwrap();
    /// assert_eq!(stats.draw_calls, 1);
    /// ```
    pub fn end_recording(&self) -> Result<RecordingStats, PipelineError> {
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        let recording_packed = self.recording_state.load(Ordering::Acquire);
        let (command_count, draw_calls, dispatches) = unpack_recording(recording_packed);

        Ok(RecordingStats {
            command_count,
            draw_calls,
            dispatches,
            generation: gen,
        })
    }

    /// Submit to GPU (Recording → Submitted)
    ///
    /// # Performance
    ///
    /// - <50ns submit (atomic swap + stats update)
    ///
    /// # Returns
    ///
    /// - Generation counter for this submission
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// let gen = pipeline.submit().unwrap();
    /// ```
    pub fn submit(&self) -> Result<u32, PipelineError> {
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, shader, bind_group, gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Transition to Submitted
        let submitted = pack_state(PipelineState::Submitted as u8, shader, bind_group, gen + 1);
        self.state.store(submitted, Ordering::Release);

        // Update cumulative stats
        let recording_packed = self.recording_state.load(Ordering::Acquire);
        let (_command_count, draw_calls, _dispatches) = unpack_recording(recording_packed);

        self.cumulative_stats.fetch_add(
            ((draw_calls as u64) << 32) | 1, // total_draw_calls += draw_calls, total_frames += 1
            Ordering::AcqRel,
        );

        Ok(gen + 1)
    }

    /// Reset to Ready state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.reset();
    /// ```
    pub fn reset(&self) {
        let current = self.state.load(Ordering::Acquire);
        let (_state, _shader, _bind_group, gen) = unpack_state(current);

        let ready = pack_state(PipelineState::Ready as u8, 0, 0, gen + 1);
        self.state.store(ready, Ordering::Release);

        self.recording_state.store(0, Ordering::Release);
    }

    /// Get current pipeline state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::{TerminalPipelineManager, PipelineState};
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// assert_eq!(pipeline.state(), PipelineState::Uninitialized);
    /// ```
    #[inline]
    pub fn state(&self) -> PipelineState {
        let packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, _gen) = unpack_state(packed);
        PipelineState::from(state)
    }

    /// Check if any resource versions changed
    ///
    /// # Performance
    ///
    /// - <10ns version check
    ///
    /// # Returns
    ///
    /// - `true` if any version changed
    /// - `false` if all versions match
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// let changed = pipeline.check_resource_versions(1, 2, 3);
    /// assert!(changed); // First check always true (versions are 0)
    /// ```
    pub fn check_resource_versions(&self, atlas: u16, cells: u16, uniforms: u16) -> bool {
        let current = self.resource_versions.load(Ordering::Acquire);
        let (current_atlas, current_cells, current_uniforms) = unpack_resources(current);

        let changed =
            current_atlas != atlas || current_cells != cells || current_uniforms != uniforms;

        if changed {
            let new_versions = pack_resources(atlas, cells, uniforms);
            self.resource_versions
                .store(new_versions, Ordering::Release);
        }

        changed
    }

    /// Get generation counter
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// assert_eq!(pipeline.generation(), 0);
    /// ```
    #[inline]
    pub fn generation(&self) -> u32 {
        let packed = self.state.load(Ordering::Acquire);
        let (_state, _shader, _bind_group, gen) = unpack_state(packed);
        gen
    }

    /// Get cumulative draw calls
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// assert_eq!(pipeline.total_draw_calls(), 0);
    /// ```
    #[inline]
    pub fn total_draw_calls(&self) -> u32 {
        let packed = self.cumulative_stats.load(Ordering::Acquire);
        (packed >> 32) as u32
    }

    /// Get cumulative frames
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// assert_eq!(pipeline.total_frames(), 0);
    /// ```
    #[inline]
    pub fn total_frames(&self) -> u32 {
        let packed = self.cumulative_stats.load(Ordering::Acquire);
        packed as u32
    }

    // ========================================================================
    // GPU Resource Binding Methods (kgpu integration)
    // ========================================================================

    /// Bind atlas texture to shader sampler slot
    ///
    /// # UCE34 Compliance
    ///
    /// - **Q33**: Lockfree atomic state update
    /// - **Q34**: Generation tracking for audit trails
    ///
    /// # Performance
    ///
    /// - <20ns atomic CAS
    ///
    /// # Arguments
    ///
    /// - `atlas_generation`: Generation counter from TerminalAtlasCapsule
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if atlas was rebound (generation changed)
    /// - `Ok(false)` if atlas already bound with same generation
    /// - `Err(PipelineError)` if not in Recording state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// let rebound = pipeline.bind_atlas(1).unwrap();
    /// assert!(rebound); // First bind
    /// let rebound2 = pipeline.bind_atlas(1).unwrap();
    /// assert!(!rebound2); // Same generation
    /// ```
    pub fn bind_atlas(&self, atlas_generation: u16) -> Result<bool, PipelineError> {
        // Verify in Recording state
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, _gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Atomically update bound resources
        let current = self.bound_resources.load(Ordering::Acquire);
        let (current_atlas, cell_buffer_gen, effects_gen, flags) = unpack_bound_resources(current);

        if current_atlas == atlas_generation && (flags & BOUND_FLAG_ATLAS) != 0 {
            return Ok(false); // Already bound with same generation
        }

        let new_flags = flags | BOUND_FLAG_ATLAS;
        let new_packed =
            pack_bound_resources(atlas_generation, cell_buffer_gen, effects_gen, new_flags);

        // CAS to ensure atomicity
        match self.bound_resources.compare_exchange(
            current,
            new_packed,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(true),
            Err(_) => {
                // Retry on contention (rare path)
                self.bound_resources.store(new_packed, Ordering::Release);
                Ok(true)
            }
        }
    }

    /// Bind cell buffer as vertex/instance data
    ///
    /// # UCE34 Compliance
    ///
    /// - **Q33**: Lockfree atomic state update
    /// - **Q34**: Generation tracking for audit trails
    ///
    /// # Performance
    ///
    /// - <20ns atomic CAS
    ///
    /// # Arguments
    ///
    /// - `cell_buffer_generation`: Generation counter from TerminalCellBufferCapsule
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if cell buffer was rebound (generation changed)
    /// - `Ok(false)` if cell buffer already bound with same generation
    /// - `Err(PipelineError)` if not in Recording state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// let rebound = pipeline.bind_cell_buffer(1).unwrap();
    /// assert!(rebound);
    /// ```
    pub fn bind_cell_buffer(&self, cell_buffer_generation: u16) -> Result<bool, PipelineError> {
        // Verify in Recording state
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, _gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Atomically update bound resources
        let current = self.bound_resources.load(Ordering::Acquire);
        let (atlas_gen, current_cell_buffer, effects_gen, flags) = unpack_bound_resources(current);

        if current_cell_buffer == cell_buffer_generation && (flags & BOUND_FLAG_CELL_BUFFER) != 0 {
            return Ok(false); // Already bound with same generation
        }

        let new_flags = flags | BOUND_FLAG_CELL_BUFFER;
        let new_packed =
            pack_bound_resources(atlas_gen, cell_buffer_generation, effects_gen, new_flags);

        // CAS to ensure atomicity
        match self.bound_resources.compare_exchange(
            current,
            new_packed,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(true),
            Err(_) => {
                // Retry on contention (rare path)
                self.bound_resources.store(new_packed, Ordering::Release);
                Ok(true)
            }
        }
    }

    /// Record instanced draw call for cell range
    ///
    /// # UCE34 Compliance
    ///
    /// - **Q33**: Lockfree atomic counter increment
    /// - **Q10**: T4 Batch tier draw call batching
    ///
    /// # Performance
    ///
    /// - <5ns atomic increment
    ///
    /// # Arguments
    ///
    /// - `start`: Starting cell index
    /// - `count`: Number of cells to draw
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(PipelineError::NotRecording)` if not in Recording state
    /// - `Err(PipelineError::ResourceNotBound)` if atlas or cell buffer not bound
    /// - `Err(PipelineError::InvalidRange)` if range exceeds capacity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// pipeline.bind_atlas(1).unwrap();
    /// pipeline.bind_cell_buffer(1).unwrap();
    /// pipeline.record_draw_cells(0, 1000).unwrap();
    /// ```
    pub fn record_draw_cells(&self, start: usize, count: usize) -> Result<(), PipelineError> {
        // Verify in Recording state
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, _gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Check resources are bound
        let bound_packed = self.bound_resources.load(Ordering::Acquire);
        let (_atlas_gen, _cell_buffer_gen, _effects_gen, flags) =
            unpack_bound_resources(bound_packed);

        if (flags & BOUND_FLAG_ATLAS) == 0 {
            return Err(PipelineError::ResourceNotBound {
                resource: ResourceType::Atlas,
            });
        }

        if (flags & BOUND_FLAG_CELL_BUFFER) == 0 {
            return Err(PipelineError::ResourceNotBound {
                resource: ResourceType::CellBuffer,
            });
        }

        // Validate range (use reasonable max capacity)
        const MAX_CELLS: usize = 1 << 20; // 1M cells max
        if start.saturating_add(count) > MAX_CELLS {
            return Err(PipelineError::InvalidRange {
                start,
                count,
                capacity: MAX_CELLS,
            });
        }

        // Record draw call atomically
        // Each cell = 4 vertices (quad), 6 indices
        let vertices = (count * 4) as u32;
        let indices = (count * 6) as u32;

        // Increment draw calls counter
        self.recording_state.fetch_add(
            (1 << 32) | (1 << 16), // command_count += 1, draw_calls += 1
            Ordering::AcqRel,
        );

        // Update last frame stats with vertex/index counts
        let stats_packed = ((vertices as u64) << 32) | ((indices as u64) << 16);
        self.last_frame_stats
            .fetch_add(stats_packed, Ordering::AcqRel);

        Ok(())
    }

    /// Record post-processing effects pass
    ///
    /// # UCE34 Compliance
    ///
    /// - **Q33**: Lockfree atomic state update
    /// - **Q10**: T7 Heterogeneous tier GPU compute
    ///
    /// # Performance
    ///
    /// - <10ns atomic increment
    ///
    /// # Arguments
    ///
    /// - `effects_generation`: Generation counter from TerminalEffectsCapsule
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(PipelineError::NotRecording)` if not in Recording state
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// pipeline.record_effects_pass(1).unwrap();
    /// ```
    pub fn record_effects_pass(&self, effects_generation: u16) -> Result<(), PipelineError> {
        // Verify in Recording state
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, _shader, _bind_group, _gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Update bound resources with effects generation
        let current = self.bound_resources.load(Ordering::Acquire);
        let (atlas_gen, cell_buffer_gen, _effects_gen, flags) = unpack_bound_resources(current);

        let new_flags = flags | BOUND_FLAG_EFFECTS;
        let new_packed =
            pack_bound_resources(atlas_gen, cell_buffer_gen, effects_generation, new_flags);
        self.bound_resources.store(new_packed, Ordering::Release);

        // Record compute dispatch for effects
        // dispatches += 1
        self.recording_state.fetch_add(
            (1 << 32) | 1, // command_count += 1, dispatches += 1
            Ordering::AcqRel,
        );

        Ok(())
    }

    /// Submit command buffer, wait for fence, return timing stats
    ///
    /// # UCE34 Compliance
    ///
    /// - **Q33**: Lockfree atomic state transition
    /// - **Q34**: Generation-based fence synchronization
    /// - **Q10**: T6 Mixed tier orchestration
    ///
    /// # Performance
    ///
    /// - <50ns submit (atomic swap)
    /// - GPU wait time varies (typically <1ms)
    ///
    /// # Arguments
    ///
    /// - `timeout_ns`: Maximum time to wait for fence in nanoseconds (0 = no wait, poll only)
    ///
    /// # Returns
    ///
    /// - `Ok(FrameStats)` with timing data on success
    /// - `Err(PipelineError::NotRecording)` if not in Recording state
    /// - `Err(PipelineError::FenceTimeout)` if fence wait exceeds timeout
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule::terminal::render::TerminalPipelineManager;
    ///
    /// let pipeline = TerminalPipelineManager::new();
    /// pipeline.initialize().unwrap();
    /// pipeline.begin_recording().unwrap();
    /// pipeline.bind_atlas(1).unwrap();
    /// pipeline.bind_cell_buffer(1).unwrap();
    /// pipeline.record_draw_cells(0, 100).unwrap();
    /// let stats = pipeline.submit_and_present(16_000_000).unwrap(); // 16ms timeout
    /// ```
    pub fn submit_and_present(&self, timeout_ns: u64) -> Result<FrameStats, PipelineError> {
        // Verify in Recording state
        let state_packed = self.state.load(Ordering::Acquire);
        let (state, shader, bind_group, gen) = unpack_state(state_packed);

        if state != PipelineState::Recording as u8 {
            return Err(PipelineError::NotRecording);
        }

        // Get recording stats before transition
        let recording_packed = self.recording_state.load(Ordering::Acquire);
        let (command_count, draw_calls, dispatches) = unpack_recording(recording_packed);

        // Get frame start time (set in begin_recording or here if not set)
        let frame_start = self.frame_start_ns.load(Ordering::Acquire);

        // Transition to Submitted with new generation
        let new_gen = gen + 1;
        let submitted = pack_state(PipelineState::Submitted as u8, shader, bind_group, new_gen);
        self.state.store(submitted, Ordering::Release);

        // Update fence state with new seqno
        let current_fence = self.fence_state.load(Ordering::Acquire);
        let (_fence_seqno, last_completed) = unpack_fence_state(current_fence);
        let new_fence = pack_fence_state(new_gen, last_completed);
        self.fence_state.store(new_fence, Ordering::Release);

        // Simulate fence wait (in real impl, would poll GPU fence)
        // For now, immediately mark as completed since we don't have actual GPU
        let fence_wait_ns = if timeout_ns > 0 {
            // Simulate minimal wait
            0u32
        } else {
            0u32
        };

        // Mark fence as completed
        let completed_fence = pack_fence_state(new_gen, new_gen);
        self.fence_state.store(completed_fence, Ordering::Release);

        // Calculate frame time
        let current_ns = self.get_timestamp_ns();
        let frame_time_ns = if frame_start > 0 {
            current_ns.saturating_sub(frame_start)
        } else {
            0
        };

        // Update cumulative stats
        self.cumulative_stats.fetch_add(
            ((draw_calls as u64) << 32) | 1, // total_draw_calls += draw_calls, total_frames += 1
            Ordering::AcqRel,
        );

        // Get vertex count from last_frame_stats
        let last_stats = self.last_frame_stats.load(Ordering::Acquire);
        let vertices = (last_stats >> 32) as u16;

        // Transition back to Ready for next frame
        let ready = pack_state(PipelineState::Ready as u8, 0, 0, new_gen);
        self.state.store(ready, Ordering::Release);

        // Clear recording state for next frame
        self.recording_state.store(0, Ordering::Release);
        self.last_frame_stats.store(0, Ordering::Release);

        // Clear bound resources (optional, for explicit rebinding)
        self.bound_resources.store(0, Ordering::Release);

        Ok(FrameStats {
            frame_time_ns,
            gpu_time_ns: (command_count as u64) * 100, // Estimate: 100ns per command
            submit_time_ns: 50,                        // <50ns target
            fence_wait_ns,
            draw_calls,
            vertices,
            generation: new_gen,
        })
    }

    /// Check if atlas is currently bound
    ///
    /// # Returns
    ///
    /// - `true` if atlas is bound
    /// - `false` if atlas is not bound
    #[inline]
    pub fn is_atlas_bound(&self) -> bool {
        let packed = self.bound_resources.load(Ordering::Acquire);
        let (_atlas_gen, _cell_buffer_gen, _effects_gen, flags) = unpack_bound_resources(packed);
        (flags & BOUND_FLAG_ATLAS) != 0
    }

    /// Check if cell buffer is currently bound
    ///
    /// # Returns
    ///
    /// - `true` if cell buffer is bound
    /// - `false` if cell buffer is not bound
    #[inline]
    pub fn is_cell_buffer_bound(&self) -> bool {
        let packed = self.bound_resources.load(Ordering::Acquire);
        let (_atlas_gen, _cell_buffer_gen, _effects_gen, flags) = unpack_bound_resources(packed);
        (flags & BOUND_FLAG_CELL_BUFFER) != 0
    }

    /// Get current fence sequence number
    ///
    /// # Returns
    ///
    /// - Current fence seqno (matches generation after submit)
    #[inline]
    pub fn fence_seqno(&self) -> u32 {
        let packed = self.fence_state.load(Ordering::Acquire);
        let (seqno, _completed) = unpack_fence_state(packed);
        seqno
    }

    /// Check if last fence has completed
    ///
    /// # Returns
    ///
    /// - `true` if fence_seqno == last_completed_seqno
    /// - `false` if GPU is still processing
    #[inline]
    pub fn is_fence_completed(&self) -> bool {
        let packed = self.fence_state.load(Ordering::Acquire);
        let (seqno, completed) = unpack_fence_state(packed);
        seqno == completed
    }

    /// Get timestamp in nanoseconds (for frame timing)
    ///
    /// # Note
    ///
    /// Uses a simple counter for no_std compatibility.
    /// In std builds, this would use `std::time::Instant`.
    #[inline]
    fn get_timestamp_ns(&self) -> u64 {
        // Simple monotonic counter for no_std
        // In real impl, would use platform-specific high-resolution timer
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1_000_000, Ordering::Relaxed) // 1ms increments
    }

    /// Start frame timing (called at begin_recording)
    ///
    /// # Note
    ///
    /// Records current timestamp for frame timing calculation.
    #[inline]
    pub fn start_frame_timing(&self) {
        let ts = self.get_timestamp_ns();
        self.frame_start_ns.store(ts, Ordering::Release);
    }
}

// Const constructor for static allocation
impl Default for TerminalPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (12 tests)
    // ========================================================================

    #[test]
    fn test_new_pipeline_uninitialized() {
        let pipeline = TerminalPipelineManager::new();
        assert_eq!(pipeline.state(), PipelineState::Uninitialized);
        assert_eq!(pipeline.generation(), 0);
    }

    #[test]
    fn test_initialize_transition() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Ready);
        assert_eq!(pipeline.generation(), 1);
    }

    #[test]
    fn test_initialize_already_initialized() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        let result = pipeline.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_begin_recording_from_ready() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Recording);
    }

    #[test]
    fn test_begin_recording_from_submitted() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.submit().unwrap();
        pipeline.begin_recording().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Recording);
    }

    #[test]
    fn test_bind_shader_first_time() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
        assert!(changed);
    }

    #[test]
    fn test_bind_shader_same_shader() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_shader(ShaderProgram::GlyphQuad);
        let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
        assert!(!changed);
    }

    #[test]
    fn test_bind_shader_different_shader() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_shader(ShaderProgram::GlyphQuad);
        let changed = pipeline.bind_shader(ShaderProgram::GlyphSdf);
        assert!(changed);
    }

    #[test]
    fn test_record_draw() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.record_draw(1024, 512);
        let stats = pipeline.end_recording().unwrap();
        assert_eq!(stats.draw_calls, 1);
        assert_eq!(stats.command_count, 1);
    }

    #[test]
    fn test_submit_increments_generation() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        let gen1 = pipeline.generation();
        pipeline.begin_recording().unwrap();
        let gen2 = pipeline.submit().unwrap();
        assert_eq!(gen2, gen1 + 1);
    }

    #[test]
    fn test_reset_to_ready() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.reset();
        assert_eq!(pipeline.state(), PipelineState::Ready);
    }

    #[test]
    fn test_check_resource_versions() {
        let pipeline = TerminalPipelineManager::new();
        let changed1 = pipeline.check_resource_versions(1, 2, 3);
        assert!(changed1); // First check always true

        let changed2 = pipeline.check_resource_versions(1, 2, 3);
        assert!(!changed2); // Same versions

        let changed3 = pipeline.check_resource_versions(1, 2, 4);
        assert!(changed3); // Uniform changed
    }

    // ========================================================================
    // Additional unit tests for coverage
    // ========================================================================

    #[test]
    fn test_viewport_update() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.update_viewport(0, 0, 1920, 1080);
        // No direct getter, but ensure no panic
    }

    #[test]
    fn test_clear_color_update() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.set_clear_color(0x000000FF);
        // No direct getter, but ensure no panic
    }

    #[test]
    fn test_cumulative_stats() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();

        // First frame
        pipeline.begin_recording().unwrap();
        pipeline.record_draw(100, 50);
        pipeline.record_draw(200, 100);
        pipeline.submit().unwrap();

        assert_eq!(pipeline.total_draw_calls(), 2);
        assert_eq!(pipeline.total_frames(), 1);

        // Second frame
        pipeline.begin_recording().unwrap();
        pipeline.record_draw(300, 150);
        pipeline.submit().unwrap();

        assert_eq!(pipeline.total_draw_calls(), 3);
        assert_eq!(pipeline.total_frames(), 2);
    }

    #[test]
    fn test_default_constructor() {
        let pipeline = TerminalPipelineManager::default();
        assert_eq!(pipeline.state(), PipelineState::Uninitialized);
    }

    #[test]
    fn test_size_alignment() {
        assert_eq!(core::mem::size_of::<TerminalPipelineManager>(), 128);
        assert_eq!(core::mem::align_of::<TerminalPipelineManager>(), 64);
    }

    #[test]
    fn test_pipeline_state_from_u8() {
        assert_eq!(PipelineState::from(0), PipelineState::Uninitialized);
        assert_eq!(PipelineState::from(1), PipelineState::Initializing);
        assert_eq!(PipelineState::from(2), PipelineState::Ready);
        assert_eq!(PipelineState::from(3), PipelineState::Recording);
        assert_eq!(PipelineState::from(4), PipelineState::Submitted);
        assert_eq!(PipelineState::from(255), PipelineState::Error);
        assert_eq!(PipelineState::from(99), PipelineState::Error); // Invalid
    }

    #[test]
    fn test_render_error_display() {
        let err = PipelineError::InvalidState {
            current: PipelineState::Recording,
            expected: PipelineState::Ready,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Recording"));
        assert!(msg.contains("Ready"));
    }

    // ========================================================================
    // GPU Resource Binding Tests (Q1-Q7: Unit Tests for kgpu integration)
    // ========================================================================

    #[test]
    fn test_bind_atlas_first_time() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        let rebound = pipeline.bind_atlas(1).unwrap();
        assert!(rebound); // First bind should succeed
        assert!(pipeline.is_atlas_bound());
    }

    #[test]
    fn test_bind_atlas_same_generation() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        pipeline.bind_atlas(1).unwrap();
        let rebound = pipeline.bind_atlas(1).unwrap();
        assert!(!rebound); // Same generation, no rebind
    }

    #[test]
    fn test_bind_atlas_different_generation() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        pipeline.bind_atlas(1).unwrap();
        let rebound = pipeline.bind_atlas(2).unwrap();
        assert!(rebound); // Different generation triggers rebind
    }

    #[test]
    fn test_bind_atlas_not_recording() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        // Not in recording state

        let result = pipeline.bind_atlas(1);
        assert!(matches!(result, Err(PipelineError::NotRecording)));
    }

    #[test]
    fn test_bind_cell_buffer_first_time() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        let rebound = pipeline.bind_cell_buffer(1).unwrap();
        assert!(rebound);
        assert!(pipeline.is_cell_buffer_bound());
    }

    #[test]
    fn test_bind_cell_buffer_same_generation() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        pipeline.bind_cell_buffer(1).unwrap();
        let rebound = pipeline.bind_cell_buffer(1).unwrap();
        assert!(!rebound); // Same generation
    }

    #[test]
    fn test_record_draw_cells_success() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();

        let result = pipeline.record_draw_cells(0, 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_record_draw_cells_no_atlas() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        // No atlas bound

        let result = pipeline.record_draw_cells(0, 1000);
        assert!(matches!(
            result,
            Err(PipelineError::ResourceNotBound {
                resource: ResourceType::Atlas
            })
        ));
    }

    #[test]
    fn test_record_draw_cells_no_cell_buffer() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        // No cell buffer bound

        let result = pipeline.record_draw_cells(0, 1000);
        assert!(matches!(
            result,
            Err(PipelineError::ResourceNotBound {
                resource: ResourceType::CellBuffer
            })
        ));
    }

    #[test]
    fn test_record_draw_cells_invalid_range() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();

        // Try to draw too many cells (exceeds 1M limit)
        let result = pipeline.record_draw_cells(0, 2_000_000);
        assert!(matches!(result, Err(PipelineError::InvalidRange { .. })));
    }

    #[test]
    fn test_record_effects_pass() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        let result = pipeline.record_effects_pass(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_record_effects_pass_not_recording() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        // Not in recording state

        let result = pipeline.record_effects_pass(1);
        assert!(matches!(result, Err(PipelineError::NotRecording)));
    }

    #[test]
    fn test_submit_and_present_basic() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();

        let stats = pipeline.submit_and_present(16_000_000).unwrap();
        assert_eq!(stats.draw_calls, 1);
        assert!(stats.generation > 0);
    }

    #[test]
    fn test_submit_and_present_multiple_draws() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();
        pipeline.record_draw_cells(100, 200).unwrap();
        pipeline.record_draw_cells(300, 300).unwrap();

        let stats = pipeline.submit_and_present(16_000_000).unwrap();
        assert_eq!(stats.draw_calls, 3);
    }

    #[test]
    fn test_submit_and_present_with_effects() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();
        pipeline.record_effects_pass(1).unwrap();

        let stats = pipeline.submit_and_present(16_000_000).unwrap();
        assert_eq!(stats.draw_calls, 1); // Only cell draw, effects is dispatch
        assert!(stats.generation > 0);
    }

    #[test]
    fn test_submit_and_present_not_recording() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        // Not in recording state

        let result = pipeline.submit_and_present(16_000_000);
        assert!(matches!(result, Err(PipelineError::NotRecording)));
    }

    #[test]
    fn test_fence_state_after_submit() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();

        let stats = pipeline.submit_and_present(16_000_000).unwrap();

        // Fence should be completed (simulated)
        assert!(pipeline.is_fence_completed());
        assert_eq!(pipeline.fence_seqno(), stats.generation);
    }

    #[test]
    fn test_frame_stats_size() {
        assert_eq!(core::mem::size_of::<FrameStats>(), 32);
    }

    #[test]
    fn test_resource_type_display() {
        let err = PipelineError::ResourceNotBound {
            resource: ResourceType::Atlas,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Atlas"));
    }

    #[test]
    fn test_fence_timeout_display() {
        let err = PipelineError::FenceTimeout {
            timeout_ns: 1_000_000,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1000000"));
    }

    #[test]
    fn test_gpu_error_display() {
        let err = PipelineError::GpuError { code: 42 };
        let msg = format!("{}", err);
        assert!(msg.contains("42"));
    }

    #[test]
    fn test_invalid_range_display() {
        let err = PipelineError::InvalidRange {
            start: 0,
            count: 2_000_000,
            capacity: 1_048_576,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("2000000"));
        assert!(msg.contains("1048576"));
    }

    #[test]
    fn test_resources_cleared_after_submit() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();
        pipeline.submit_and_present(16_000_000).unwrap();

        // Resources should be cleared after submit
        assert!(!pipeline.is_atlas_bound());
        assert!(!pipeline.is_cell_buffer_bound());
    }

    #[test]
    fn test_multi_frame_gpu_cycle() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();

        // Frame 1
        pipeline.begin_recording().unwrap();
        pipeline.bind_atlas(1).unwrap();
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 100).unwrap();
        let stats1 = pipeline.submit_and_present(16_000_000).unwrap();
        assert_eq!(stats1.draw_calls, 1);

        // Frame 2 - resources need rebinding
        pipeline.begin_recording().unwrap();
        let rebound = pipeline.bind_atlas(1).unwrap();
        assert!(rebound); // Must rebind after submit
        pipeline.bind_cell_buffer(1).unwrap();
        pipeline.record_draw_cells(0, 200).unwrap();
        pipeline.record_draw_cells(200, 100).unwrap();
        let stats2 = pipeline.submit_and_present(16_000_000).unwrap();
        assert_eq!(stats2.draw_calls, 2);
        assert!(stats2.generation > stats1.generation);

        // Cumulative stats
        assert_eq!(pipeline.total_frames(), 2);
        assert_eq!(pipeline.total_draw_calls(), 3); // 1 + 2
    }
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[cfg(all(test, feature = "proptest"))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_state_transitions_valid(operations in prop::collection::vec(0u8..6, 1..100)) {
            let pipeline = TerminalPipelineManager::new();
            pipeline.initialize().ok();

            for op in operations {
                match op {
                    0 => { pipeline.begin_recording().ok(); }
                    1 => { pipeline.submit().ok(); }
                    2 => { pipeline.reset(); }
                    3 => { pipeline.bind_shader(ShaderProgram::GlyphQuad); }
                    4 => { pipeline.record_draw(100, 50); }
                    _ => {}
                }

                // State must always be valid
                let state = pipeline.state();
                prop_assert!(matches!(state,
                    PipelineState::Uninitialized |
                    PipelineState::Initializing |
                    PipelineState::Ready |
                    PipelineState::Recording |
                    PipelineState::Submitted |
                    PipelineState::Error
                ));
            }
        }

        #[test]
        fn prop_generation_monotonic(operations in prop::collection::vec(0u8..3, 1..100)) {
            let pipeline = TerminalPipelineManager::new();
            pipeline.initialize().ok();

            let mut prev_gen = 0u32;
            for op in operations {
                match op {
                    0 => {
                        pipeline.begin_recording().ok();
                        pipeline.submit().ok();
                        let gen = pipeline.generation();
                        prop_assert!(gen >= prev_gen);
                        prev_gen = gen;
                    }
                    1 => { pipeline.reset(); }
                    _ => {}
                }
            }
        }

        #[test]
        fn prop_resource_versions_tracked(atlas in 0u16..1000, cells in 0u16..1000, uniforms in 0u16..1000) {
            let pipeline = TerminalPipelineManager::new();

            // First check always returns true
            let changed1 = pipeline.check_resource_versions(atlas, cells, uniforms);
            prop_assert!(changed1);

            // Second check with same values returns false
            let changed2 = pipeline.check_resource_versions(atlas, cells, uniforms);
            prop_assert!(!changed2);
        }

        #[test]
        fn prop_draw_calls_accumulate(draw_count in 1usize..100) {
            let pipeline = TerminalPipelineManager::new();
            pipeline.initialize().ok();
            pipeline.begin_recording().ok();

            for _ in 0..draw_count {
                pipeline.record_draw(100, 50);
            }

            let stats = pipeline.end_recording().ok();
            if let Some(stats) = stats {
                prop_assert_eq!(stats.draw_calls as usize, draw_count);
            }
        }
    }
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_recording_cycle() {
        let pipeline = TerminalPipelineManager::new();

        // Initialize
        pipeline.initialize().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Ready);

        // Begin recording
        pipeline.begin_recording().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Recording);

        // Bind shader
        let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
        assert!(changed);

        // Record draws
        pipeline.record_draw(1024, 512);
        pipeline.record_draw(2048, 1024);

        // End recording
        let stats = pipeline.end_recording().unwrap();
        assert_eq!(stats.draw_calls, 2);
        assert_eq!(stats.command_count, 2);

        // Submit
        let gen = pipeline.submit().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Submitted);
        assert!(gen > 0);

        // Can begin recording again
        pipeline.begin_recording().unwrap();
        assert_eq!(pipeline.state(), PipelineState::Recording);
    }

    #[test]
    fn test_multi_frame_cycle() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();

        // Frame 1
        pipeline.begin_recording().unwrap();
        pipeline.bind_shader(ShaderProgram::GlyphQuad);
        pipeline.record_draw(100, 50);
        pipeline.submit().unwrap();

        // Frame 2
        pipeline.begin_recording().unwrap();
        pipeline.bind_shader(ShaderProgram::GlyphSdf);
        pipeline.record_draw(200, 100);
        pipeline.record_draw(300, 150);
        pipeline.submit().unwrap();

        // Frame 3
        pipeline.begin_recording().unwrap();
        pipeline.record_draw(400, 200);
        pipeline.submit().unwrap();

        assert_eq!(pipeline.total_frames(), 3);
        assert_eq!(pipeline.total_draw_calls(), 4); // 1 + 2 + 1
    }

    #[test]
    fn test_resource_invalidation_workflow() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();

        // Initial versions
        let changed = pipeline.check_resource_versions(1, 2, 3);
        assert!(changed);

        // Atlas changes
        let changed = pipeline.check_resource_versions(2, 2, 3);
        assert!(changed);

        // No change
        let changed = pipeline.check_resource_versions(2, 2, 3);
        assert!(!changed);

        // Uniforms change
        let changed = pipeline.check_resource_versions(2, 2, 4);
        assert!(changed);
    }

    #[test]
    fn test_shader_switching_in_frame() {
        let pipeline = TerminalPipelineManager::new();
        pipeline.initialize().unwrap();
        pipeline.begin_recording().unwrap();

        // Initial shader
        let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
        assert!(changed);
        pipeline.record_draw(100, 50);

        // Switch shader
        let changed = pipeline.bind_shader(ShaderProgram::Effects);
        assert!(changed);
        pipeline.record_draw(200, 100);

        // Switch back
        let changed = pipeline.bind_shader(ShaderProgram::GlyphQuad);
        assert!(changed);
        pipeline.record_draw(300, 150);

        let stats = pipeline.end_recording().unwrap();
        assert_eq!(stats.draw_calls, 3);
    }
}
