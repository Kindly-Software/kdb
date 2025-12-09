//! KgpuCommandEncoderCapsule - T4 Batch Tier Type-State Command Encoder
//!
//! **Tier**: T4 Batch (10-100x speedup via batching)
//! **Size**: 512B (cache-aligned)
//! **Purpose**: Command encoder with compile-time enforced recording states
//! **Target**: <50ns per command recording
//!
//! # Key Innovation
//!
//! The encoder STATE is in the TYPE SYSTEM, preventing invalid command recording
//! at compile time. This eliminates an entire class of runtime errors:
//!
//! - Cannot record commands on a finished encoder (compile error)
//! - Cannot finish an empty encoder without beginning (compile error)
//! - Cannot begin an already recording encoder (compile error)
//!
//! # Type-State Pattern
//!
//! ```text
//! KgpuCommandEncoderCapsule<Empty>
//!            |
//!            | begin()
//!            v
//! KgpuCommandEncoderCapsule<Recording>
//!            |
//!            | record(), copy_buffer_to_buffer(), draw(), dispatch(), etc.
//!            |
//!            | finish()
//!            v
//! KgpuCommandEncoderCapsule<Finished>
//!            |
//!            | mark_submitted() (internal)
//!            v
//! KgpuCommandEncoderCapsule<Submitted>
//!            |
//!            | reset()
//!            v
//! KgpuCommandEncoderCapsule<Empty> (reuse)
//! ```
//!
//! # Memory Layout (512B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       Primary (state|command_count|generation)
//! 8       8       Secondary (batch_id|flags)
//! 16      256     Command ring buffer (16 slots x 16B)
//! 272     2       Write index
//! 274     2       Read index
//! 276     4       Reserved
//! 280     8       Label hash
//! 288     8       Device generation
//! 296     216     Padding to 512B
//! ```
//!
//! # Performance (B32 Targets)
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | begin() | <20ns | State transition only |
//! | record() | <50ns | Ring buffer append |
//! | finish() | <20ns | State transition only |
//! | command_count() | <5ns | Atomic load |
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q1-Q34 systematic discovery, Q10 T4 tier selection
//! - **Chaos**: 100% lockfree (zero mutex/RwLock), cache-aligned (512B)
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **B32**: Fair baselines, 95% CI targets
//! - **T28**: Unit/Property/Integration tests included
//! - **I20**: Zero breaking changes, feature-gated
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::gpu::kgpu::command::{
//!     KgpuCommandEncoderCapsule, Empty, Recording, Finished,
//!     CommandSlot, CommandType,
//! };
//!
//! // Create empty encoder
//! let encoder: KgpuCommandEncoderCapsule<Empty> = KgpuCommandEncoderCapsule::new();
//!
//! // Begin recording (consumes Empty, returns Recording)
//! let mut encoder: KgpuCommandEncoderCapsule<Recording> = encoder.begin();
//!
//! // Record commands
//! encoder.copy_buffer_to_buffer(0, 1024, 4096).unwrap();
//! encoder.set_pipeline(42).unwrap();
//! encoder.draw(36, 1, 0, 0).unwrap();
//!
//! // Finish recording (consumes Recording, returns Finished)
//! let encoder: KgpuCommandEncoderCapsule<Finished> = encoder.finish();
//!
//! // Query command count
//! assert_eq!(encoder.command_count(), 3);
//!
//! // Compile-time error: cannot record on Finished encoder!
//! // encoder.draw(36, 1, 0, 0); // ERROR: method not found
//! ```

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};

use super::device::KgpuError;

// ============================================================================
// Sealed Trait Pattern for Type-States
// ============================================================================

mod sealed {
    /// Sealed trait to prevent external implementation of EncoderState
    ///
    /// # ASSUM Safety
    /// - #ASSUME_SEALED_COMPLETE: Only Empty, Recording, Finished implement this
    /// - #VERIFY: No external types can implement EncoderState
    pub trait Sealed {}
}

/// Marker trait for encoder states
///
/// This trait is sealed - only the three states (Empty, Recording, Finished)
/// can implement it. This ensures type-safety at compile time.
///
/// # ASSUM Safety
/// - #ASSUME_STATES_EXHAUSTIVE: All valid states are enumerated
/// - #ASSUME_THREAD_SAFE: All states are Send + Sync
pub trait EncoderState: sealed::Sealed + Send + Sync {}

// ============================================================================
// Type-State Markers
// ============================================================================

/// Empty state - encoder has not started recording
///
/// In this state, the encoder can only call `begin()` to start recording.
/// All command recording methods are unavailable (compile-time enforced).
#[derive(Debug, Clone, Copy, Default)]
pub struct Empty;

/// Recording state - encoder is actively recording commands
///
/// In this state, the encoder can:
/// - Record commands (copy, draw, dispatch, etc.)
/// - Call `finish()` to complete recording
///
/// The `begin()` method is unavailable (compile-time enforced).
#[derive(Debug, Clone, Copy, Default)]
pub struct Recording;

/// Finished state - encoder has completed recording
///
/// In this state, the encoder is immutable and can only:
/// - Query command count
/// - Access recorded commands
/// - Transition to Submitted state (via mark_submitted, internal)
///
/// All recording methods are unavailable (compile-time enforced).
#[derive(Debug, Clone, Copy, Default)]
pub struct Finished;

/// Submitted state - encoder has been submitted to a queue
///
/// In this state, the encoder is fully immutable and represents a command
/// buffer that has been sent to the GPU. It can be:
/// - Queried for status
/// - Reset back to Empty for reuse
///
/// This is the terminal state before reset.
#[derive(Debug, Clone, Copy, Default)]
pub struct Submitted;

// Implement sealed trait
impl sealed::Sealed for Empty {}
impl sealed::Sealed for Recording {}
impl sealed::Sealed for Finished {}
impl sealed::Sealed for Submitted {}

// Implement EncoderState
impl EncoderState for Empty {}
impl EncoderState for Recording {}
impl EncoderState for Finished {}
impl EncoderState for Submitted {}

// ============================================================================
// Command Types
// ============================================================================

/// GPU command types supported by the encoder
///
/// Each command type maps to a specific GPU operation.
/// The discriminant values match common GPU API conventions.
///
/// # ASSUM Safety
/// - #ASSUME_COMMAND_TYPES_COMPLETE: All common GPU commands enumerated
/// - #VERIFY: Values 0-22 are valid, others reserved
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CommandType {
    /// No operation (placeholder)
    #[default]
    Noop = 0,

    /// Copy data between buffers
    CopyBufferToBuffer = 1,

    /// Copy buffer data to texture
    CopyBufferToTexture = 2,

    /// Copy texture data to buffer
    CopyTextureToBuffer = 3,

    /// Copy between textures
    CopyTextureToTexture = 4,

    /// Clear buffer memory
    ClearBuffer = 5,

    /// Clear texture memory
    ClearTexture = 6,

    /// Set active render/compute pipeline
    SetPipeline = 7,

    /// Bind descriptor set / bind group
    SetBindGroup = 8,

    /// Set vertex buffer binding
    SetVertexBuffer = 9,

    /// Set index buffer binding
    SetIndexBuffer = 10,

    /// Draw vertices
    Draw = 11,

    /// Draw indexed vertices
    DrawIndexed = 12,

    /// Draw using indirect buffer
    DrawIndirect = 13,

    /// Dispatch compute shader
    Dispatch = 14,

    /// Dispatch using indirect buffer
    DispatchIndirect = 15,

    /// Begin render pass
    BeginRenderPass = 16,

    /// End render pass
    EndRenderPass = 17,

    /// Begin compute pass
    BeginComputePass = 18,

    /// End compute pass
    EndComputePass = 19,

    /// Insert debug marker
    InsertDebugMarker = 20,

    /// Push debug group
    PushDebugGroup = 21,

    /// Pop debug group
    PopDebugGroup = 22,
}

impl CommandType {
    /// Convert u8 to CommandType, returning Noop for invalid values
    ///
    /// # ASSUM Safety
    /// - #ASSUME_INVALID_TO_NOOP: Invalid discriminants safely map to Noop
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Noop,
            1 => Self::CopyBufferToBuffer,
            2 => Self::CopyBufferToTexture,
            3 => Self::CopyTextureToBuffer,
            4 => Self::CopyTextureToTexture,
            5 => Self::ClearBuffer,
            6 => Self::ClearTexture,
            7 => Self::SetPipeline,
            8 => Self::SetBindGroup,
            9 => Self::SetVertexBuffer,
            10 => Self::SetIndexBuffer,
            11 => Self::Draw,
            12 => Self::DrawIndexed,
            13 => Self::DrawIndirect,
            14 => Self::Dispatch,
            15 => Self::DispatchIndirect,
            16 => Self::BeginRenderPass,
            17 => Self::EndRenderPass,
            18 => Self::BeginComputePass,
            19 => Self::EndComputePass,
            20 => Self::InsertDebugMarker,
            21 => Self::PushDebugGroup,
            22 => Self::PopDebugGroup,
            _ => Self::Noop,
        }
    }
}

// ============================================================================
// Command Slot
// ============================================================================

/// Single command slot in the ring buffer (16 bytes)
///
/// Each slot holds one recorded command with its parameters.
/// The layout is designed for cache-efficient access.
///
/// # Memory Layout (16B)
///
/// ```text
/// Offset  Size  Field
/// 0       1     cmd_type (CommandType discriminant)
/// 1       1     flags (command-specific flags)
/// 2       2     param1 (first 16-bit parameter)
/// 4       4     param2 (32-bit parameter)
/// 8       8     data (64-bit data payload)
/// ```
///
/// # ASSUM Safety
/// - #ASSUME_SLOT_LAYOUT_STABLE: 16B size is fixed for ring buffer math
/// - #VERIFY: size_of::<CommandSlot>() == 16
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandSlot {
    /// Command type (discriminant of CommandType)
    pub cmd_type: u8,

    /// Command-specific flags
    /// - Bit 0: Synchronous (requires fence)
    /// - Bit 1: Debug (emit debug info)
    /// - Bits 2-7: Reserved
    pub flags: u8,

    /// First 16-bit parameter
    /// Usage varies by command type:
    /// - SetBindGroup: bind group index
    /// - SetVertexBuffer: slot index
    /// - Draw: first_vertex (low 16 bits)
    pub param1: u16,

    /// 32-bit parameter
    /// Usage varies by command type:
    /// - Copy*: source offset or size
    /// - SetPipeline: pipeline ID
    /// - Draw: vertex_count
    /// - Dispatch: x workgroup count
    pub param2: u32,

    /// 64-bit data payload
    /// Usage varies by command type:
    /// - Copy*: destination offset or additional size
    /// - Draw: packed (first_instance:16, instance_count:16, first_vertex_high:16, reserved:16)
    /// - Dispatch: packed (y:16, z:16, reserved:32)
    pub data: u64,
}

impl CommandSlot {
    /// Create a new command slot
    #[inline]
    pub const fn new(cmd_type: CommandType, flags: u8, param1: u16, param2: u32, data: u64) -> Self {
        Self {
            cmd_type: cmd_type as u8,
            flags,
            param1,
            param2,
            data,
        }
    }

    /// Create a Noop command slot
    #[inline]
    pub const fn noop() -> Self {
        Self {
            cmd_type: CommandType::Noop as u8,
            flags: 0,
            param1: 0,
            param2: 0,
            data: 0,
        }
    }

    /// Get the command type
    #[inline]
    pub const fn command_type(&self) -> CommandType {
        CommandType::from_u8(self.cmd_type)
    }

    /// Check if command requires synchronization
    #[inline]
    pub const fn is_synchronous(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    /// Check if command has debug flag
    #[inline]
    pub const fn is_debug(&self) -> bool {
        (self.flags & 0x02) != 0
    }
}

// Compile-time verification of CommandSlot size
const _: () = {
    assert!(core::mem::size_of::<CommandSlot>() == 16);
};

// ============================================================================
// Bit Field Masks
// ============================================================================

/// State field in primary: bits [63:56] (8 bits)
const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;

/// Command count field in primary: bits [55:40] (16 bits)
const COUNT_SHIFT: u64 = 40;
const COUNT_MASK: u64 = 0xFFFF << COUNT_SHIFT;

/// Generation field in primary: bits [39:0] (40 bits)
const GENERATION_MASK: u64 = 0x000000FF_FFFFFFFF;

/// Batch ID field in secondary: bits [63:32] (32 bits)
const BATCH_ID_SHIFT: u64 = 32;
const BATCH_ID_MASK: u64 = 0xFFFFFFFF << BATCH_ID_SHIFT;

/// Flags field in secondary: bits [31:0] (32 bits)
const FLAGS_MASK: u64 = 0x00000000_FFFFFFFF;

/// State values
const STATE_EMPTY: u8 = 0;
const STATE_RECORDING: u8 = 1;
const STATE_FINISHED: u8 = 2;
const STATE_SUBMITTED: u8 = 3;

/// Maximum commands in ring buffer
pub const MAX_COMMANDS: usize = 16;

// ============================================================================
// Error Type Extension
// ============================================================================

/// Error type for command encoder operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// Ring buffer is full
    BufferFull,

    /// Invalid command parameters
    InvalidParameters,

    /// Device error from underlying layer
    DeviceError(KgpuError),
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferFull => write!(f, "Command buffer is full (max {} commands)", MAX_COMMANDS),
            Self::InvalidParameters => write!(f, "Invalid command parameters"),
            Self::DeviceError(e) => write!(f, "Device error: {}", e),
        }
    }
}

/// Result type for command encoder operations
pub type CommandResult<T> = core::result::Result<T, CommandError>;

// ============================================================================
// KgpuCommandEncoderCapsule<S>
// ============================================================================

/// KGPU Command Encoder with Type-State Safety
///
/// A command encoder that uses Rust's type system to enforce valid state
/// transitions at compile time. Invalid operations are compilation errors,
/// not runtime panics.
///
/// # Tier: T4 Batch (10-100x speedup via batching)
/// # Size: 512B (cache-aligned)
/// # Target: <50ns per command record
///
/// # Type States
///
/// - `Empty`: Initial state, can only call `begin()`
/// - `Recording`: Active recording, can record commands or call `finish()`
/// - `Finished`: Immutable, can only query commands
///
/// # ASSUM Safety
///
/// - #ASSUME_STATE_IN_TYPE: State transitions enforce by type system
/// - #ASSUME_RING_BUFFER_LOCKFREE: Ring buffer uses atomic indices
/// - #ASSUME_GENERATION_ABA_SAFE: 40-bit generation prevents ABA
/// - #ASSUME_CACHE_ALIGNED: 512B alignment prevents false sharing
/// - #ASSUME_COMMAND_ORDERING: Commands recorded in strict order
#[repr(C, align(512))]
pub struct KgpuCommandEncoderCapsule<S: EncoderState> {
    /// Primary coordination: state(8) | command_count(16) | generation(40)
    ///
    /// - Bits [63:56]: Internal state (matches type state for debugging)
    /// - Bits [55:40]: Number of commands recorded
    /// - Bits [39:0]: Generation counter (ABA prevention)
    primary: AtomicU64,

    /// Secondary coordination: batch_id(32) | flags(32)
    ///
    /// - Bits [63:32]: Batch identifier for grouping
    /// - Bits [31:0]: Encoder flags
    secondary: AtomicU64,

    /// Command ring buffer (16 slots x 16B = 256B)
    ///
    /// Thread-local recording ensures no contention during command append.
    /// Ring buffer allows efficient wraparound if needed.
    commands: [CommandSlot; MAX_COMMANDS],

    /// Write index for next command (0-15, wraps)
    write_index: AtomicU16,

    /// Read index for command iteration (0-15)
    read_index: AtomicU16,

    /// Reserved for alignment
    _reserved: u32,

    /// Encoder label hash (for debugging/profiling)
    label_hash: AtomicU64,

    /// Associated device generation (for validation)
    device_generation: AtomicU64,

    /// Type-state marker (zero-sized)
    _state: PhantomData<S>,

    /// Padding to reach 512B total
    /// 8 + 8 + 256 + 2 + 2 + 4 + 8 + 8 + 0 = 296
    /// 512 - 296 = 216
    _padding: [u8; 216],
}

// Compile-time size and alignment verification (Q33 mandate)
const _: () = {
    assert!(core::mem::size_of::<KgpuCommandEncoderCapsule<Empty>>() == 512);
    assert!(core::mem::align_of::<KgpuCommandEncoderCapsule<Empty>>() == 512);
    assert!(core::mem::size_of::<KgpuCommandEncoderCapsule<Recording>>() == 512);
    assert!(core::mem::size_of::<KgpuCommandEncoderCapsule<Finished>>() == 512);
    assert!(core::mem::size_of::<KgpuCommandEncoderCapsule<Submitted>>() == 512);
    assert!(core::mem::align_of::<KgpuCommandEncoderCapsule<Submitted>>() == 512);
};

// ============================================================================
// Empty State Implementation
// ============================================================================

impl KgpuCommandEncoderCapsule<Empty> {
    /// Create a new command encoder in Empty state
    ///
    /// # Performance
    ///
    /// - Latency: O(1) constant time initialization
    /// - Memory: 512B stack allocation
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_INITIAL_STATE_EMPTY: Encoder starts in Empty state
    /// - #VERIFY: All fields initialized to zero/default
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let encoder: KgpuCommandEncoderCapsule<Empty> = KgpuCommandEncoderCapsule::new();
    /// assert!(!encoder.has_commands());
    /// ```
    pub const fn new() -> Self {
        Self {
            // Primary: state=Empty(0), count=0, generation=0
            primary: AtomicU64::new(0),
            // Secondary: batch_id=0, flags=0
            secondary: AtomicU64::new(0),
            // Initialize all command slots to Noop
            commands: [CommandSlot::noop(); MAX_COMMANDS],
            write_index: AtomicU16::new(0),
            read_index: AtomicU16::new(0),
            _reserved: 0,
            label_hash: AtomicU64::new(0),
            device_generation: AtomicU64::new(0),
            _state: PhantomData,
            _padding: [0; 216],
        }
    }

    /// Create encoder with a specific batch ID
    ///
    /// Batch IDs group related encoders for debugging and profiling.
    ///
    /// # Performance
    ///
    /// - Latency: O(1) constant time
    pub const fn with_batch_id(batch_id: u32) -> Self {
        let secondary = (batch_id as u64) << BATCH_ID_SHIFT;
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(secondary),
            commands: [CommandSlot::noop(); MAX_COMMANDS],
            write_index: AtomicU16::new(0),
            read_index: AtomicU16::new(0),
            _reserved: 0,
            label_hash: AtomicU64::new(0),
            device_generation: AtomicU64::new(0),
            _state: PhantomData,
            _padding: [0; 216],
        }
    }

    /// Begin recording commands
    ///
    /// Transitions from Empty to Recording state. This method consumes
    /// the Empty encoder and returns a Recording encoder.
    ///
    /// # Type-State Transition
    ///
    /// `KgpuCommandEncoderCapsule<Empty>` -> `KgpuCommandEncoderCapsule<Recording>`
    ///
    /// # Performance
    ///
    /// - Latency: <20ns (state update only)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_BEGIN_TRANSITIONS: Consumes self, returns new type
    /// - #VERIFY: Generation incremented on begin
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let empty: KgpuCommandEncoderCapsule<Empty> = KgpuCommandEncoderCapsule::new();
    /// let recording: KgpuCommandEncoderCapsule<Recording> = empty.begin();
    /// // empty is now consumed, cannot be used
    /// ```
    #[inline]
    pub fn begin(self) -> KgpuCommandEncoderCapsule<Recording> {
        // Load current primary to get generation
        let old_primary = self.primary.load(Ordering::Acquire);
        let generation = (old_primary & GENERATION_MASK) + 1;

        // Build new primary with Recording state
        let new_primary = ((STATE_RECORDING as u64) << STATE_SHIFT) | generation;

        // Store updated state (no CAS needed - we own the encoder)
        self.primary.store(new_primary, Ordering::Release);

        // Transmute to Recording state
        // SAFETY: Layout is identical, only PhantomData type changes
        // #ASSUME_LAYOUT_IDENTICAL: All states have same memory layout
        unsafe { core::mem::transmute(self) }
    }
}

impl Default for KgpuCommandEncoderCapsule<Empty> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Recording State Implementation
// ============================================================================

impl KgpuCommandEncoderCapsule<Recording> {
    /// Record a raw command slot
    ///
    /// Low-level method to record any command. Higher-level methods like
    /// `copy_buffer_to_buffer()` use this internally.
    ///
    /// # Performance
    ///
    /// - Latency: <50ns (ring buffer append)
    ///
    /// # Errors
    ///
    /// Returns `BufferFull` if all 16 command slots are used.
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_RECORD_ATOMIC: Write index is atomic for thread-safety
    /// - #ASSUME_RING_BOUND: Index always in 0..MAX_COMMANDS
    #[inline]
    pub fn record(&mut self, cmd: CommandSlot) -> CommandResult<()> {
        // Get current write index
        let idx = self.write_index.load(Ordering::Acquire) as usize;

        // Check if buffer is full
        if idx >= MAX_COMMANDS {
            return Err(CommandError::BufferFull);
        }

        // Write command to slot
        // SAFETY: idx < MAX_COMMANDS checked above
        self.commands[idx] = cmd;

        // Increment write index
        self.write_index.store((idx + 1) as u16, Ordering::Release);

        // Update command count in primary
        self.increment_command_count();

        Ok(())
    }

    /// Copy buffer to buffer
    ///
    /// Records a buffer-to-buffer copy command.
    ///
    /// # Arguments
    ///
    /// - `src_offset`: Source buffer offset in bytes
    /// - `dst_offset`: Destination buffer offset in bytes
    /// - `size`: Number of bytes to copy
    ///
    /// # Performance
    ///
    /// - Latency: <50ns
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// encoder.copy_buffer_to_buffer(0, 1024, 4096)?;
    /// ```
    #[inline]
    pub fn copy_buffer_to_buffer(
        &mut self,
        src_offset: u32,
        dst_offset: u32,
        size: u32,
    ) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::CopyBufferToBuffer,
            0,
            0,
            size,
            ((dst_offset as u64) << 32) | (src_offset as u64),
        );
        self.record(cmd)
    }

    /// Clear buffer
    ///
    /// Records a buffer clear command.
    ///
    /// # Arguments
    ///
    /// - `offset`: Start offset in bytes
    /// - `size`: Number of bytes to clear
    #[inline]
    pub fn clear_buffer(&mut self, offset: u32, size: u32) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::ClearBuffer,
            0,
            0,
            size,
            offset as u64,
        );
        self.record(cmd)
    }

    /// Set render/compute pipeline
    ///
    /// # Arguments
    ///
    /// - `pipeline_id`: ID of the pipeline to bind
    #[inline]
    pub fn set_pipeline(&mut self, pipeline_id: u32) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::SetPipeline,
            0,
            0,
            pipeline_id,
            0,
        );
        self.record(cmd)
    }

    /// Set bind group / descriptor set
    ///
    /// # Arguments
    ///
    /// - `index`: Bind group slot index (0-7)
    /// - `bind_group_id`: ID of the bind group to bind
    #[inline]
    pub fn set_bind_group(&mut self, index: u8, bind_group_id: u32) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::SetBindGroup,
            0,
            index as u16,
            bind_group_id,
            0,
        );
        self.record(cmd)
    }

    /// Set vertex buffer
    ///
    /// # Arguments
    ///
    /// - `slot`: Vertex buffer slot (0-15)
    /// - `buffer_id`: ID of the buffer to bind
    /// - `offset`: Offset into buffer in bytes
    #[inline]
    pub fn set_vertex_buffer(&mut self, slot: u8, buffer_id: u32, offset: u64) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::SetVertexBuffer,
            0,
            slot as u16,
            buffer_id,
            offset,
        );
        self.record(cmd)
    }

    /// Set index buffer
    ///
    /// # Arguments
    ///
    /// - `buffer_id`: ID of the index buffer
    /// - `offset`: Offset into buffer in bytes
    /// - `format`: Index format (0=u16, 1=u32)
    #[inline]
    pub fn set_index_buffer(&mut self, buffer_id: u32, offset: u64, format: u8) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::SetIndexBuffer,
            format,
            0,
            buffer_id,
            offset,
        );
        self.record(cmd)
    }

    /// Draw vertices
    ///
    /// # Arguments
    ///
    /// - `vertex_count`: Number of vertices to draw
    /// - `instance_count`: Number of instances to draw
    /// - `first_vertex`: Index of first vertex
    /// - `first_instance`: Index of first instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Draw 36 vertices (12 triangles), 1 instance
    /// encoder.draw(36, 1, 0, 0)?;
    /// ```
    #[inline]
    pub fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> CommandResult<()> {
        // Pack parameters into data field
        // data = first_instance(16) | instance_count(16) | first_vertex(32)
        let data = ((first_instance as u64) << 48)
            | ((instance_count as u64) << 32)
            | (first_vertex as u64);

        let cmd = CommandSlot::new(
            CommandType::Draw,
            0,
            0,
            vertex_count,
            data,
        );
        self.record(cmd)
    }

    /// Draw indexed vertices
    ///
    /// # Arguments
    ///
    /// - `index_count`: Number of indices to draw
    /// - `instance_count`: Number of instances
    /// - `first_index`: First index in the index buffer
    /// - `base_vertex`: Value added to each index before reading vertex
    /// - `first_instance`: First instance to draw
    #[inline]
    pub fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) -> CommandResult<()> {
        // Pack into data: first_instance(16) | instance_count(16) | base_vertex(32)
        let data = ((first_instance as u64) << 48)
            | ((instance_count as u64) << 32)
            | ((base_vertex as u32) as u64);

        let cmd = CommandSlot::new(
            CommandType::DrawIndexed,
            0,
            (first_index >> 16) as u16, // High bits of first_index
            index_count,
            data | ((first_index as u64 & 0xFFFF) << 16), // Adjust packing
        );
        self.record(cmd)
    }

    /// Dispatch compute shader
    ///
    /// # Arguments
    ///
    /// - `x`: Number of workgroups in X dimension
    /// - `y`: Number of workgroups in Y dimension
    /// - `z`: Number of workgroups in Z dimension
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Dispatch 64x64x1 workgroups
    /// encoder.dispatch(64, 64, 1)?;
    /// ```
    #[inline]
    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) -> CommandResult<()> {
        // Pack y and z into data field
        let data = ((z as u64) << 16) | (y as u64);

        let cmd = CommandSlot::new(
            CommandType::Dispatch,
            0,
            0,
            x,
            data,
        );
        self.record(cmd)
    }

    /// Begin render pass
    ///
    /// # Arguments
    ///
    /// - `render_pass_id`: ID of the render pass configuration
    #[inline]
    pub fn begin_render_pass(&mut self, render_pass_id: u32) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::BeginRenderPass,
            0,
            0,
            render_pass_id,
            0,
        );
        self.record(cmd)
    }

    /// End render pass
    #[inline]
    pub fn end_render_pass(&mut self) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::EndRenderPass,
            0,
            0,
            0,
            0,
        );
        self.record(cmd)
    }

    /// Begin compute pass
    #[inline]
    pub fn begin_compute_pass(&mut self) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::BeginComputePass,
            0,
            0,
            0,
            0,
        );
        self.record(cmd)
    }

    /// End compute pass
    #[inline]
    pub fn end_compute_pass(&mut self) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::EndComputePass,
            0,
            0,
            0,
            0,
        );
        self.record(cmd)
    }

    /// Insert debug marker
    ///
    /// # Arguments
    ///
    /// - `marker_hash`: Hash of the marker string (for fast lookup)
    #[inline]
    pub fn insert_debug_marker(&mut self, marker_hash: u64) -> CommandResult<()> {
        let cmd = CommandSlot::new(
            CommandType::InsertDebugMarker,
            0x02, // Debug flag
            0,
            0,
            marker_hash,
        );
        self.record(cmd)
    }

    /// Finish recording commands
    ///
    /// Transitions from Recording to Finished state. The encoder becomes
    /// immutable and ready for submission.
    ///
    /// # Type-State Transition
    ///
    /// `KgpuCommandEncoderCapsule<Recording>` -> `KgpuCommandEncoderCapsule<Finished>`
    ///
    /// # Performance
    ///
    /// - Latency: <20ns (state update only)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_FINISH_IMMUTABLE: Finished encoder cannot record more commands
    /// - #VERIFY: State in primary matches type state
    #[inline]
    pub fn finish(self) -> KgpuCommandEncoderCapsule<Finished> {
        // Load current primary
        let old_primary = self.primary.load(Ordering::Acquire);
        let count = (old_primary & COUNT_MASK) >> COUNT_SHIFT;
        let generation = (old_primary & GENERATION_MASK) + 1;

        // Build new primary with Finished state
        let new_primary = ((STATE_FINISHED as u64) << STATE_SHIFT)
            | (count << COUNT_SHIFT)
            | generation;

        // Store updated state
        self.primary.store(new_primary, Ordering::Release);

        // Transmute to Finished state
        // SAFETY: Layout is identical, only PhantomData type changes
        unsafe { core::mem::transmute(self) }
    }

    /// Get current command count
    #[inline]
    fn increment_command_count(&self) {
        loop {
            let old = self.primary.load(Ordering::Acquire);
            let state = (old & STATE_MASK) >> STATE_SHIFT;
            let count = ((old & COUNT_MASK) >> COUNT_SHIFT) + 1;
            let generation = old & GENERATION_MASK;

            let new = (state << STATE_SHIFT) | (count << COUNT_SHIFT) | generation;

            if self.primary.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return;
            }
            core::hint::spin_loop();
        }
    }
}

// ============================================================================
// Finished State Implementation
// ============================================================================

impl KgpuCommandEncoderCapsule<Finished> {
    /// Get the number of recorded commands
    ///
    /// # Performance
    ///
    /// - Latency: <5ns (atomic load)
    #[inline]
    pub fn command_count(&self) -> u16 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & COUNT_MASK) >> COUNT_SHIFT) as u16
    }

    /// Get a slice of all recorded commands
    ///
    /// # Performance
    ///
    /// - Latency: O(1) (slice creation)
    #[inline]
    pub fn commands(&self) -> &[CommandSlot] {
        let count = self.command_count() as usize;
        &self.commands[..count.min(MAX_COMMANDS)]
    }

    /// Iterate over recorded commands
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &CommandSlot> {
        self.commands().iter()
    }

    /// Get command at specific index
    ///
    /// Returns `None` if index is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&CommandSlot> {
        if index < self.command_count() as usize {
            Some(&self.commands[index])
        } else {
            None
        }
    }

    /// Check if encoder has any commands
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.command_count() == 0
    }

    /// Mark encoder as submitted to a queue (internal use only)
    ///
    /// Transitions from Finished to Submitted state. This method is intended
    /// for internal queue submission logic and should not be called directly
    /// by user code.
    ///
    /// # Type-State Transition
    ///
    /// `KgpuCommandEncoderCapsule<Finished>` -> `KgpuCommandEncoderCapsule<Submitted>`
    ///
    /// # Performance
    ///
    /// - Latency: <20ns (state update only)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_SUBMIT_ONCE: Encoder should only be submitted once
    /// - #ASSUME_QUEUE_OWNERSHIP: Queue takes ownership during submit
    /// - #VERIFY: Generation incremented on submit
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Internal queue submission path
    /// let finished: KgpuCommandEncoderCapsule<Finished> = encoder.finish();
    /// let submitted: KgpuCommandEncoderCapsule<Submitted> = finished.mark_submitted();
    /// ```
    #[inline]
    pub(crate) fn mark_submitted(self) -> KgpuCommandEncoderCapsule<Submitted> {
        // Load current primary
        let old_primary = self.primary.load(Ordering::Acquire);
        let count = (old_primary & COUNT_MASK) >> COUNT_SHIFT;
        let generation = (old_primary & GENERATION_MASK) + 1;

        // Build new primary with Submitted state
        let new_primary = ((STATE_SUBMITTED as u64) << STATE_SHIFT)
            | (count << COUNT_SHIFT)
            | generation;

        // Store updated state
        self.primary.store(new_primary, Ordering::Release);

        // Transmute to Submitted state
        // SAFETY: Layout is identical, only PhantomData type changes
        unsafe { core::mem::transmute(self) }
    }
}

// ============================================================================
// Submitted State Implementation
// ============================================================================

impl KgpuCommandEncoderCapsule<Submitted> {
    /// Get the number of recorded commands
    ///
    /// # Performance
    ///
    /// - Latency: <5ns (atomic load)
    #[inline]
    pub fn command_count(&self) -> u16 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & COUNT_MASK) >> COUNT_SHIFT) as u16
    }

    /// Get a slice of all recorded commands
    ///
    /// # Performance
    ///
    /// - Latency: O(1) (slice creation)
    #[inline]
    pub fn commands(&self) -> &[CommandSlot] {
        let count = self.command_count() as usize;
        &self.commands[..count.min(MAX_COMMANDS)]
    }

    /// Iterate over recorded commands
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &CommandSlot> {
        self.commands().iter()
    }

    /// Get command at specific index
    ///
    /// Returns `None` if index is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&CommandSlot> {
        if index < self.command_count() as usize {
            Some(&self.commands[index])
        } else {
            None
        }
    }

    /// Check if encoder has any commands
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.command_count() == 0
    }

    /// Reset encoder back to Empty state for reuse
    ///
    /// Transitions from Submitted to Empty state, allowing the encoder to be
    /// reused for a new recording session. This is useful for pooling command
    /// encoders to avoid repeated allocations.
    ///
    /// # Type-State Transition
    ///
    /// `KgpuCommandEncoderCapsule<Submitted>` -> `KgpuCommandEncoderCapsule<Empty>`
    ///
    /// # Performance
    ///
    /// - Latency: <50ns (zero write index, state transition)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME_RESET_SAFE: GPU has finished executing commands before reset
    /// - #ASSUME_GENERATION_OVERFLOW_SAFE: 40-bit generation won't overflow
    /// - #VERIFY: Write index reset to 0, state set to Empty
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Reuse submitted encoder
    /// let submitted: KgpuCommandEncoderCapsule<Submitted> = queue.submit(finished);
    /// // ... wait for GPU completion ...
    /// let empty: KgpuCommandEncoderCapsule<Empty> = submitted.reset();
    /// let recording = empty.begin();
    /// ```
    #[inline]
    pub fn reset(self) -> KgpuCommandEncoderCapsule<Empty> {
        // Load current primary to get generation
        let old_primary = self.primary.load(Ordering::Acquire);
        let generation = (old_primary & GENERATION_MASK) + 1;

        // Build new primary with Empty state, zero command count
        let new_primary = ((STATE_EMPTY as u64) << STATE_SHIFT) | generation;

        // Store updated state
        self.primary.store(new_primary, Ordering::Release);

        // Reset write/read indices
        self.write_index.store(0, Ordering::Release);
        self.read_index.store(0, Ordering::Release);

        // Transmute to Empty state
        // SAFETY: Layout is identical, only PhantomData type changes
        unsafe { core::mem::transmute(self) }
    }
}

// ============================================================================
// Common Implementation for All States
// ============================================================================

impl<S: EncoderState> KgpuCommandEncoderCapsule<S> {
    /// Get the generation counter
    ///
    /// # Performance
    ///
    /// - Latency: <5ns (atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Get the batch ID
    ///
    /// # Performance
    ///
    /// - Latency: <5ns (atomic load)
    #[inline]
    pub fn batch_id(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & BATCH_ID_MASK) >> BATCH_ID_SHIFT) as u32
    }

    /// Get the encoder flags
    #[inline]
    pub fn flags(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & FLAGS_MASK) as u32
    }

    /// Get the label hash
    #[inline]
    pub fn label_hash(&self) -> u64 {
        self.label_hash.load(Ordering::Relaxed)
    }

    /// Set the label hash
    #[inline]
    pub fn set_label_hash(&self, hash: u64) {
        self.label_hash.store(hash, Ordering::Relaxed);
    }

    /// Get the associated device generation
    #[inline]
    pub fn device_generation(&self) -> u64 {
        self.device_generation.load(Ordering::Acquire)
    }

    /// Set the associated device generation
    #[inline]
    pub fn set_device_generation(&self, gen: u64) {
        self.device_generation.store(gen, Ordering::Release);
    }

    /// Get internal state value (for debugging)
    #[inline]
    pub fn internal_state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }
}

// ============================================================================
// Send + Sync
// ============================================================================

// SAFETY: KgpuCommandEncoderCapsule only contains atomic types and arrays of Copy types.
// No raw pointers or references to non-thread-safe data.
// #ASSUME_ATOMIC_THREAD_SAFE: AtomicU64/AtomicU16 are thread-safe by definition.
// #ASSUME_COMMAND_SLOT_COPY: CommandSlot is Copy, no shared references.
unsafe impl<S: EncoderState> Send for KgpuCommandEncoderCapsule<S> {}
unsafe impl<S: EncoderState> Sync for KgpuCommandEncoderCapsule<S> {}

// ============================================================================
// Debug
// ============================================================================

impl<S: EncoderState> core::fmt::Debug for KgpuCommandEncoderCapsule<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KgpuCommandEncoderCapsule")
            .field("state", &self.internal_state())
            .field("generation", &self.generation())
            .field("batch_id", &self.batch_id())
            .field("write_index", &self.write_index.load(Ordering::Relaxed))
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
    fn test_encoder_size_is_512_bytes() {
        assert_eq!(
            core::mem::size_of::<KgpuCommandEncoderCapsule<Empty>>(),
            512,
            "Empty encoder must be exactly 512 bytes"
        );
        assert_eq!(
            core::mem::size_of::<KgpuCommandEncoderCapsule<Recording>>(),
            512,
            "Recording encoder must be exactly 512 bytes"
        );
        assert_eq!(
            core::mem::size_of::<KgpuCommandEncoderCapsule<Finished>>(),
            512,
            "Finished encoder must be exactly 512 bytes"
        );
    }

    #[test]
    fn test_encoder_alignment_is_512_bytes() {
        assert_eq!(
            core::mem::align_of::<KgpuCommandEncoderCapsule<Empty>>(),
            512,
            "Empty encoder must have 512-byte alignment"
        );
        assert_eq!(
            core::mem::align_of::<KgpuCommandEncoderCapsule<Recording>>(),
            512,
            "Recording encoder must have 512-byte alignment"
        );
        assert_eq!(
            core::mem::align_of::<KgpuCommandEncoderCapsule<Finished>>(),
            512,
            "Finished encoder must have 512-byte alignment"
        );
    }

    #[test]
    fn test_command_slot_size_is_16_bytes() {
        assert_eq!(
            core::mem::size_of::<CommandSlot>(),
            16,
            "CommandSlot must be exactly 16 bytes"
        );
    }

    // ========================================================================
    // Type-State Transition Tests
    // ========================================================================

    #[test]
    fn test_empty_to_recording_transition() {
        let empty: KgpuCommandEncoderCapsule<Empty> = KgpuCommandEncoderCapsule::new();
        assert_eq!(empty.internal_state(), STATE_EMPTY);

        let recording: KgpuCommandEncoderCapsule<Recording> = empty.begin();
        assert_eq!(recording.internal_state(), STATE_RECORDING);
    }

    #[test]
    fn test_recording_to_finished_transition() {
        let empty = KgpuCommandEncoderCapsule::new();
        let recording = empty.begin();
        assert_eq!(recording.internal_state(), STATE_RECORDING);

        let finished = recording.finish();
        assert_eq!(finished.internal_state(), STATE_FINISHED);
    }

    #[test]
    fn test_full_lifecycle() {
        // Empty -> Recording -> Finished
        let encoder = KgpuCommandEncoderCapsule::new();
        assert_eq!(encoder.generation(), 0);

        let encoder = encoder.begin();
        assert_eq!(encoder.generation(), 1);

        let encoder = encoder.finish();
        assert_eq!(encoder.generation(), 2);
    }

    // ========================================================================
    // Command Recording Tests
    // ========================================================================

    #[test]
    fn test_record_single_command() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.copy_buffer_to_buffer(0, 1024, 4096).unwrap();

        let encoder = encoder.finish();
        assert_eq!(encoder.command_count(), 1);

        let cmd = encoder.get(0).unwrap();
        assert_eq!(cmd.command_type(), CommandType::CopyBufferToBuffer);
    }

    #[test]
    fn test_record_multiple_commands() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.set_pipeline(42).unwrap();
        encoder.set_bind_group(0, 100).unwrap();
        encoder.draw(36, 1, 0, 0).unwrap();

        let encoder = encoder.finish();
        assert_eq!(encoder.command_count(), 3);

        // Verify command types in order
        let commands: Vec<_> = encoder.iter().map(|c| c.command_type()).collect();
        assert_eq!(commands[0], CommandType::SetPipeline);
        assert_eq!(commands[1], CommandType::SetBindGroup);
        assert_eq!(commands[2], CommandType::Draw);
    }

    #[test]
    fn test_record_all_command_types() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.copy_buffer_to_buffer(0, 100, 200).unwrap();
        encoder.clear_buffer(0, 1024).unwrap();
        encoder.set_pipeline(1).unwrap();
        encoder.set_bind_group(0, 2).unwrap();
        encoder.set_vertex_buffer(0, 3, 0).unwrap();
        encoder.set_index_buffer(4, 0, 0).unwrap();
        encoder.draw(36, 1, 0, 0).unwrap();
        encoder.dispatch(64, 64, 1).unwrap();
        encoder.begin_render_pass(5).unwrap();
        encoder.end_render_pass().unwrap();

        let encoder = encoder.finish();
        assert_eq!(encoder.command_count(), 10);
    }

    #[test]
    fn test_buffer_full_error() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        // Fill buffer
        for _ in 0..MAX_COMMANDS {
            encoder.set_pipeline(0).unwrap();
        }

        // Next record should fail
        let result = encoder.set_pipeline(0);
        assert_eq!(result, Err(CommandError::BufferFull));
    }

    // ========================================================================
    // Finished State Tests
    // ========================================================================

    #[test]
    fn test_finished_command_count() {
        let encoder = KgpuCommandEncoderCapsule::new()
            .begin();
        let encoder = encoder.finish();

        assert_eq!(encoder.command_count(), 0);
        assert!(encoder.is_empty());
    }

    #[test]
    fn test_finished_commands_slice() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.set_pipeline(1).unwrap();
        encoder.set_pipeline(2).unwrap();
        encoder.set_pipeline(3).unwrap();

        let encoder = encoder.finish();

        let commands = encoder.commands();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].param2, 1);
        assert_eq!(commands[1].param2, 2);
        assert_eq!(commands[2].param2, 3);
    }

    #[test]
    fn test_finished_get_out_of_bounds() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();
        encoder.set_pipeline(0).unwrap();
        let encoder = encoder.finish();

        assert!(encoder.get(0).is_some());
        assert!(encoder.get(1).is_none());
        assert!(encoder.get(100).is_none());
    }

    // ========================================================================
    // Command Slot Tests
    // ========================================================================

    #[test]
    fn test_command_slot_creation() {
        let cmd = CommandSlot::new(
            CommandType::Draw,
            0x03,
            100,
            36,
            0x1234_5678,
        );

        assert_eq!(cmd.command_type(), CommandType::Draw);
        assert!(cmd.is_synchronous());
        assert!(cmd.is_debug());
        assert_eq!(cmd.param1, 100);
        assert_eq!(cmd.param2, 36);
        assert_eq!(cmd.data, 0x1234_5678);
    }

    #[test]
    fn test_command_slot_noop() {
        let cmd = CommandSlot::noop();

        assert_eq!(cmd.command_type(), CommandType::Noop);
        assert!(!cmd.is_synchronous());
        assert!(!cmd.is_debug());
        assert_eq!(cmd.param1, 0);
        assert_eq!(cmd.param2, 0);
        assert_eq!(cmd.data, 0);
    }

    #[test]
    fn test_command_type_from_u8() {
        assert_eq!(CommandType::from_u8(0), CommandType::Noop);
        assert_eq!(CommandType::from_u8(1), CommandType::CopyBufferToBuffer);
        assert_eq!(CommandType::from_u8(11), CommandType::Draw);
        assert_eq!(CommandType::from_u8(14), CommandType::Dispatch);
        assert_eq!(CommandType::from_u8(22), CommandType::PopDebugGroup);

        // Invalid values map to Noop
        assert_eq!(CommandType::from_u8(23), CommandType::Noop);
        assert_eq!(CommandType::from_u8(255), CommandType::Noop);
    }

    // ========================================================================
    // Batch ID and Generation Tests
    // ========================================================================

    #[test]
    fn test_batch_id() {
        let encoder = KgpuCommandEncoderCapsule::<Empty>::with_batch_id(12345);
        assert_eq!(encoder.batch_id(), 12345);

        let encoder = encoder.begin();
        assert_eq!(encoder.batch_id(), 12345);

        let encoder = encoder.finish();
        assert_eq!(encoder.batch_id(), 12345);
    }

    #[test]
    fn test_generation_increments() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let gen0 = encoder.generation();

        let encoder = encoder.begin();
        let gen1 = encoder.generation();
        assert_eq!(gen1, gen0 + 1);

        let encoder = encoder.finish();
        let gen2 = encoder.generation();
        assert_eq!(gen2, gen1 + 1);
    }

    // ========================================================================
    // Label and Device Generation Tests
    // ========================================================================

    #[test]
    fn test_label_hash() {
        let encoder = KgpuCommandEncoderCapsule::new();
        assert_eq!(encoder.label_hash(), 0);

        encoder.set_label_hash(0xDEADBEEF);
        assert_eq!(encoder.label_hash(), 0xDEADBEEF);
    }

    #[test]
    fn test_device_generation() {
        let encoder = KgpuCommandEncoderCapsule::new();
        assert_eq!(encoder.device_generation(), 0);

        encoder.set_device_generation(42);
        assert_eq!(encoder.device_generation(), 42);
    }

    // ========================================================================
    // Debug Format Test
    // ========================================================================

    #[test]
    fn test_debug_format() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let debug_str = format!("{:?}", encoder);

        assert!(debug_str.contains("KgpuCommandEncoderCapsule"));
        assert!(debug_str.contains("state"));
        assert!(debug_str.contains("generation"));
        assert!(debug_str.contains("batch_id"));
    }

    // ========================================================================
    // Submitted State Tests
    // ========================================================================

    #[test]
    fn test_finished_to_submitted_transition() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();
        encoder.set_pipeline(1).unwrap();
        let encoder = encoder.finish();
        assert_eq!(encoder.internal_state(), STATE_FINISHED);

        let encoder = encoder.mark_submitted();
        assert_eq!(encoder.internal_state(), STATE_SUBMITTED);
    }

    #[test]
    fn test_submitted_to_empty_reset() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();
        encoder.draw(36, 1, 0, 0).unwrap();
        let encoder = encoder.finish();
        let encoder = encoder.mark_submitted();
        assert_eq!(encoder.internal_state(), STATE_SUBMITTED);
        assert_eq!(encoder.command_count(), 1);

        let encoder = encoder.reset();
        assert_eq!(encoder.internal_state(), STATE_EMPTY);
        assert_eq!(encoder.generation(), 4); // begin(1) + finish(2) + submit(3) + reset(4)
    }

    #[test]
    fn test_full_lifecycle_with_reuse() {
        // First lifecycle: Empty -> Recording -> Finished -> Submitted
        let encoder = KgpuCommandEncoderCapsule::new();
        let gen0 = encoder.generation();

        let mut encoder = encoder.begin();
        encoder.set_pipeline(1).unwrap();
        let gen1 = encoder.generation();

        let encoder = encoder.finish();
        let gen2 = encoder.generation();

        let encoder = encoder.mark_submitted();
        let gen3 = encoder.generation();

        // Reset for reuse
        let encoder = encoder.reset();
        let gen4 = encoder.generation();
        assert_eq!(encoder.internal_state(), STATE_EMPTY);

        // Second lifecycle: Empty -> Recording -> Finished
        let mut encoder = encoder.begin();
        encoder.set_pipeline(2).unwrap();
        let encoder = encoder.finish();

        assert_eq!(encoder.command_count(), 1);
        assert!(gen4 > gen3);
        assert!(gen3 > gen2);
        assert!(gen2 > gen1);
        assert!(gen1 > gen0);
    }

    #[test]
    fn test_submitted_read_only_access() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();
        encoder.set_pipeline(42).unwrap();
        encoder.draw(36, 1, 0, 0).unwrap();
        let encoder = encoder.finish();
        let encoder = encoder.mark_submitted();

        // Can read commands
        assert_eq!(encoder.command_count(), 2);
        assert!(!encoder.is_empty());

        let cmd0 = encoder.get(0).unwrap();
        assert_eq!(cmd0.command_type(), CommandType::SetPipeline);
        assert_eq!(cmd0.param2, 42);

        let cmd1 = encoder.get(1).unwrap();
        assert_eq!(cmd1.command_type(), CommandType::Draw);

        // Can iterate
        let commands: Vec<_> = encoder.iter().map(|c| c.command_type()).collect();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_submitted_state_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<KgpuCommandEncoderCapsule<Submitted>>(),
            512,
            "Submitted encoder must be exactly 512 bytes"
        );
        assert_eq!(
            core::mem::align_of::<KgpuCommandEncoderCapsule<Submitted>>(),
            512,
            "Submitted encoder must have 512-byte alignment"
        );
    }

    // ========================================================================
    // Thread Safety Smoke Test
    // ========================================================================

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuCommandEncoderCapsule<Empty>>();
        assert_send_sync::<KgpuCommandEncoderCapsule<Recording>>();
        assert_send_sync::<KgpuCommandEncoderCapsule<Finished>>();
        assert_send_sync::<KgpuCommandEncoderCapsule<Submitted>>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_reads_finished() {
        use std::sync::Arc;
        use std::thread;

        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();
        encoder.set_pipeline(1).unwrap();
        encoder.draw(36, 1, 0, 0).unwrap();
        let encoder = Arc::new(encoder.finish());

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let enc = Arc::clone(&encoder);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = enc.command_count();
                        let _ = enc.generation();
                        let _ = enc.batch_id();
                        let _ = enc.commands();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Verify state unchanged
        assert_eq!(encoder.command_count(), 2);
    }

    // ========================================================================
    // Performance Target Validation (manual benchmarking hint)
    // ========================================================================

    #[test]
    fn test_command_recording_is_fast() {
        // This is a smoke test - actual benchmarks should use criterion
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        // Record MAX_COMMANDS commands
        for i in 0..MAX_COMMANDS {
            encoder.set_pipeline(i as u32).unwrap();
        }

        let encoder = encoder.finish();
        assert_eq!(encoder.command_count(), MAX_COMMANDS as u16);
    }

    // ========================================================================
    // Draw Command Parameter Verification
    // ========================================================================

    #[test]
    fn test_draw_parameters() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.draw(36, 10, 100, 5).unwrap();

        let encoder = encoder.finish();
        let cmd = encoder.get(0).unwrap();

        assert_eq!(cmd.command_type(), CommandType::Draw);
        assert_eq!(cmd.param2, 36); // vertex_count

        // Unpack data field
        let data = cmd.data;
        let first_instance = (data >> 48) as u32;
        let instance_count = ((data >> 32) & 0xFFFF) as u32;
        let first_vertex = (data & 0xFFFF_FFFF) as u32;

        assert_eq!(first_instance, 5);
        assert_eq!(instance_count, 10);
        assert_eq!(first_vertex, 100);
    }

    // ========================================================================
    // Dispatch Command Parameter Verification
    // ========================================================================

    #[test]
    fn test_dispatch_parameters() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.dispatch(64, 32, 16).unwrap();

        let encoder = encoder.finish();
        let cmd = encoder.get(0).unwrap();

        assert_eq!(cmd.command_type(), CommandType::Dispatch);
        assert_eq!(cmd.param2, 64); // x

        // Unpack y and z from data
        let y = (cmd.data & 0xFFFF) as u32;
        let z = ((cmd.data >> 16) & 0xFFFF) as u32;

        assert_eq!(y, 32);
        assert_eq!(z, 16);
    }

    // ========================================================================
    // Copy Buffer Parameters Verification
    // ========================================================================

    #[test]
    fn test_copy_buffer_parameters() {
        let encoder = KgpuCommandEncoderCapsule::new();
        let mut encoder = encoder.begin();

        encoder.copy_buffer_to_buffer(100, 200, 4096).unwrap();

        let encoder = encoder.finish();
        let cmd = encoder.get(0).unwrap();

        assert_eq!(cmd.command_type(), CommandType::CopyBufferToBuffer);
        assert_eq!(cmd.param2, 4096); // size

        // Unpack offsets from data
        let src_offset = (cmd.data & 0xFFFF_FFFF) as u32;
        let dst_offset = (cmd.data >> 32) as u32;

        assert_eq!(src_offset, 100);
        assert_eq!(dst_offset, 200);
    }
}
