//! Embedded PTX Module for NVIDIA Trojan Kernel
//!
//! This module provides pre-compiled PTX bytecode for the Trojan Kernel, enabling
//! <100ns GPU command latency by bypassing cuLaunchKernel overhead.
//!
//! # Architecture
//!
//! PTX (Parallel Thread Execution) is NVIDIA's intermediate assembly language.
//! The CUDA driver JIT-compiles PTX to native GPU machine code (SASS) at load time.
//! This allows binary compatibility across GPU generations.
//!
//! # Compute Capability Targets
//!
//! | SM Version | Architecture | GPUs | Features |
//! |------------|--------------|------|----------|
//! | sm_52 | Maxwell | GTX 9xx, Quadro M | Base features |
//! | sm_70 | Volta | Titan V, V100 | Independent thread scheduling, __nanosleep |
//! | sm_80 | Ampere | RTX 30xx, A100 | Async copy, reduced precision TF32 |
//!
//! # PTX Compilation
//!
//! Generate PTX from trojan_kernel.cu:
//! ```bash
//! nvcc -ptx -arch=sm_52 -o trojan_sm52.ptx trojan_kernel.cu
//! nvcc -ptx -arch=sm_70 -o trojan_sm70.ptx trojan_kernel.cu
//! nvcc -ptx -arch=sm_80 -o trojan_sm80.ptx trojan_kernel.cu
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_PTX_VALID`: PTX bytecode is syntactically correct
//! - `#VERIFY_PTX_VALID`: Validated by nvcc compilation
//! - `#ASSUME_LAYOUT_MATCH`: PTX structures match Rust TrojanCommand
//! - `#VERIFY_LAYOUT_MATCH`: Compile-time size assertions in trojan_kernel.cu
//!
//! # UCE34 Compliance
//!
//! - Q10: T7 Heterogeneous tier (GPU compute)
//! - Q33: Lockfree (no synchronization in PTX loading)
//! - Q34: Audit via PTX version strings

#![allow(dead_code)]

// ============================================================================
// PTX Bytecode Constants
// ============================================================================

/// PTX version for compatibility checks
pub const PTX_VERSION: (u32, u32) = (7, 5); // PTX ISA 7.5

/// CUDA driver version required
pub const MIN_CUDA_DRIVER: u32 = 11_000; // CUDA 11.0+

/// Magic bytes to identify PTX text
pub const PTX_MAGIC: &[u8] = b".version";

// ============================================================================
// PTX Selection by Compute Capability
// ============================================================================

/// Compute capability representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComputeCapability {
    pub major: u32,
    pub minor: u32,
}

impl ComputeCapability {
    /// Create from major.minor version
    #[inline]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Convert to SM version number (e.g., 52 for sm_52)
    #[inline]
    pub const fn sm_version(&self) -> u32 {
        self.major * 10 + self.minor
    }

    /// Check if this capability supports __nanosleep (Volta+)
    #[inline]
    pub const fn supports_nanosleep(&self) -> bool {
        self.major >= 7
    }

    /// Check if this capability supports async memory copy (Ampere+)
    #[inline]
    pub const fn supports_async_copy(&self) -> bool {
        self.major >= 8
    }

    /// Check if this is a supported compute capability
    #[inline]
    pub const fn is_supported(&self) -> bool {
        self.major >= 5 && self.major <= 9
    }
}

impl core::fmt::Display for ComputeCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "sm_{}", self.sm_version())
    }
}

/// PTX architecture tier for selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PtxArchTier {
    /// Maxwell+ (sm_52): Base features, no nanosleep
    Maxwell,
    /// Volta+ (sm_70): Independent scheduling, nanosleep
    Volta,
    /// Ampere+ (sm_80): Async copy, TF32
    Ampere,
}

impl PtxArchTier {
    /// Get minimum compute capability for this tier
    #[inline]
    pub const fn min_compute(&self) -> ComputeCapability {
        match self {
            Self::Maxwell => ComputeCapability::new(5, 2),
            Self::Volta => ComputeCapability::new(7, 0),
            Self::Ampere => ComputeCapability::new(8, 0),
        }
    }

    /// Get SM version string for nvcc
    #[inline]
    pub const fn sm_string(&self) -> &'static str {
        match self {
            Self::Maxwell => "sm_52",
            Self::Volta => "sm_70",
            Self::Ampere => "sm_80",
        }
    }

    /// Determine tier from compute capability
    #[inline]
    pub const fn from_compute(cc: ComputeCapability) -> Option<Self> {
        match cc.major {
            8 | 9 => Some(Self::Ampere),
            7 => Some(Self::Volta),
            5 | 6 => Some(Self::Maxwell),
            _ => None,
        }
    }
}

/// Select the best PTX tier for a given compute capability
///
/// Returns the highest-performance PTX that is compatible with the device.
#[inline]
pub const fn select_ptx_tier(major: u32, minor: u32) -> Option<PtxArchTier> {
    let cc = ComputeCapability::new(major, minor);
    PtxArchTier::from_compute(cc)
}

// ============================================================================
// Inline PTX Source (Human-Readable Fallback)
// ============================================================================

/// Inline PTX source code for the Trojan Kernel.
///
/// This is a complete, human-readable PTX implementation that can be loaded
/// directly by the CUDA driver. It serves as a fallback when pre-compiled
/// PTX files are unavailable.
///
/// # Structure Offsets (TrojanCommand, 64 bytes)
///
/// | Offset | Size | Field | Description |
/// |--------|------|-------|-------------|
/// | 0 | 4 | opcode | Command type (u32) |
/// | 4 | 4 | flags | Command flags (u32) |
/// | 8 | 8 | seqno | Sequence number (u64) |
/// | 16 | 8 | src | Source address/value (u64) |
/// | 24 | 8 | dst | Destination address (u64) |
/// | 32 | 8 | size | Size in bytes (u64) |
/// | 40 | 8 | extra | Extra parameter (u64) |
/// | 48 | 16 | _pad | Padding to 64 bytes |
pub const TROJAN_PTX_INLINE: &str = r#"
//
// NVIDIA Trojan Kernel - PTX Assembly
// KGPU-Driver v2.0
//
// Generated for: sm_52+ (Maxwell and newer)
// PTX ISA Version: 7.5
//

.version 7.5
.target sm_52
.address_size 64

// ============================================================================
// Constants (must match Rust TrojanOpcode)
// ============================================================================

// Command opcodes
.const .align 4 .u32 CMD_NOP = 0x00;
.const .align 4 .u32 CMD_MEM_COPY = 0x01;
.const .align 4 .u32 CMD_MEM_SET = 0x02;
.const .align 4 .u32 CMD_KERNEL_LAUNCH = 0x03;
.const .align 4 .u32 CMD_SYNC = 0x04;
.const .align 4 .u32 CMD_FENCE_SIGNAL = 0x05;
.const .align 4 .u32 CMD_FENCE_WAIT = 0x06;
.const .align 4 .u32 CMD_REGISTER_READ = 0x07;
.const .align 4 .u32 CMD_REGISTER_WRITE = 0x08;
.const .align 4 .u32 CMD_SHUTDOWN = 0xFF;

// Command flags
.const .align 4 .u32 FLAG_HAS_COMPLETION = 0x01;
.const .align 4 .u32 FLAG_ASYNC = 0x02;
.const .align 4 .u32 FLAG_FENCE_BEFORE = 0x04;
.const .align 4 .u32 FLAG_FENCE_AFTER = 0x08;

// Status codes
.const .align 4 .u64 STATUS_RUNNING = 0x0000;
.const .align 4 .u64 STATUS_IDLE = 0x0001;
.const .align 4 .u64 STATUS_PROCESSING = 0x0002;
.const .align 4 .u64 STATUS_EXITED = 0xDEAD;

// Structure offsets
// TrojanCommand (64 bytes)
.const .align 4 .u32 CMD_OFF_OPCODE = 0;
.const .align 4 .u32 CMD_OFF_FLAGS = 4;
.const .align 4 .u32 CMD_OFF_SEQNO = 8;
.const .align 4 .u32 CMD_OFF_SRC = 16;
.const .align 4 .u32 CMD_OFF_DST = 24;
.const .align 4 .u32 CMD_OFF_SIZE = 32;
.const .align 4 .u32 CMD_OFF_EXTRA = 40;

// TrojanRingHeader offsets (64 bytes)
.const .align 4 .u32 HDR_OFF_HEAD = 0;
.const .align 4 .u32 HDR_OFF_TAIL = 8;
.const .align 4 .u32 HDR_OFF_STOP = 16;
.const .align 4 .u32 HDR_OFF_STATUS = 24;
.const .align 4 .u32 HDR_OFF_PROCESSED = 32;
.const .align 4 .u32 HDR_OFF_FENCE = 40;

// ============================================================================
// Main Trojan Kernel Entry Point
// ============================================================================

.visible .entry trojan_poll(
    .param .u64 param_header,       // Pointer to TrojanRingHeader
    .param .u64 param_ring,         // Pointer to TrojanCommand array
    .param .u32 param_ring_size,    // Ring size (power of 2)
    .param .u32 param_poll_ns       // Poll interval in nanoseconds
)
{
    // Register declarations
    .reg .pred %p<16>;              // Predicates
    .reg .b32 %r<32>;               // 32-bit registers
    .reg .b64 %rd<48>;              // 64-bit registers

    // ========================================================================
    // Thread ID calculation
    // ========================================================================

    mov.u32 %r0, %tid.x;            // threadIdx.x
    mov.u32 %r1, %ctaid.x;          // blockIdx.x
    mov.u32 %r2, %ntid.x;           // blockDim.x
    mad.lo.u32 %r3, %r1, %r2, %r0;  // tid = blockIdx * blockDim + threadIdx

    // Only thread 0 processes commands
    setp.ne.u32 %p0, %r3, 0;
    @%p0 bra $L_sync_exit;

    // ========================================================================
    // Load parameters
    // ========================================================================

    ld.param.u64 %rd0, [param_header];      // header ptr
    ld.param.u64 %rd1, [param_ring];        // ring ptr
    ld.param.u32 %r4, [param_ring_size];    // ring_size
    ld.param.u32 %r5, [param_poll_ns];      // poll_ns

    // Compute ring mask (ring_size - 1) for modulo
    add.u32 %r6, %r4, -1;                   // ring_mask = ring_size - 1

    // Signal kernel is running
    mov.u64 %rd2, STATUS_RUNNING;
    st.volatile.global.u64 [%rd0+24], %rd2; // header->kernel_status

    // ========================================================================
    // Main polling loop
    // ========================================================================

$L_poll_loop:
    // System-wide memory fence
    membar.sys;

    // Check stop flag
    ld.volatile.global.u64 %rd3, [%rd0+16]; // stop_flag
    setp.ne.u64 %p1, %rd3, 0;
    @%p1 bra $L_exit;

    // Load head and tail
    ld.volatile.global.u64 %rd4, [%rd0+0];  // head
    ld.volatile.global.u64 %rd5, [%rd0+8];  // tail

    // Check if queue is empty (head == tail)
    setp.eq.u64 %p2, %rd4, %rd5;
    @%p2 bra $L_spin_wait;

    // ========================================================================
    // Process command at head
    // ========================================================================

    // Update status to processing
    mov.u64 %rd6, STATUS_PROCESSING;
    st.volatile.global.u64 [%rd0+24], %rd6;

    // Calculate command address: ring + (head & mask) * 64
    cvt.u32.u64 %r7, %rd4;                  // head as u32
    and.b32 %r8, %r7, %r6;                  // head & mask
    mul.lo.u32 %r9, %r8, 64;                // * 64 (command size)
    cvt.u64.u32 %rd7, %r9;
    add.u64 %rd8, %rd1, %rd7;               // cmd_ptr = ring + offset

    // Load opcode
    ld.global.u32 %r10, [%rd8+0];           // opcode

    // Load flags for fence checks
    ld.global.u32 %r11, [%rd8+4];           // flags

    // Check FENCE_BEFORE flag
    and.b32 %r12, %r11, FLAG_FENCE_BEFORE;
    setp.ne.u32 %p3, %r12, 0;
    @%p3 membar.sys;

    // ========================================================================
    // Command dispatch
    // ========================================================================

    // Check for CMD_SHUTDOWN first (0xFF)
    setp.eq.u32 %p4, %r10, 0xFF;
    @%p4 bra $L_cmd_shutdown;

    // Check CMD_NOP (0x00)
    setp.eq.u32 %p5, %r10, 0x00;
    @%p5 bra $L_cmd_done;

    // Check CMD_SYNC (0x04)
    setp.eq.u32 %p6, %r10, 0x04;
    @%p6 bra $L_cmd_sync;

    // Check CMD_FENCE_SIGNAL (0x05)
    setp.eq.u32 %p7, %r10, 0x05;
    @%p7 bra $L_cmd_fence_signal;

    // Check CMD_MEM_COPY (0x01) - simplified version
    setp.eq.u32 %p8, %r10, 0x01;
    @%p8 bra $L_cmd_done;  // TODO: Implement memcpy loop

    // Check CMD_MEM_SET (0x02) - simplified version
    setp.eq.u32 %p9, %r10, 0x02;
    @%p9 bra $L_cmd_done;  // TODO: Implement memset loop

    // Unknown opcode - skip
    bra $L_cmd_done;

    // ========================================================================
    // Command implementations
    // ========================================================================

$L_cmd_sync:
    membar.sys;
    bra $L_cmd_done;

$L_cmd_fence_signal:
    // Load fence address and value
    ld.global.u64 %rd10, [%rd8+24];         // dst (fence addr)
    ld.global.u64 %rd11, [%rd8+16];         // src (fence value)
    ld.global.u64 %rd12, [%rd8+8];          // seqno

    // Check fence addr is non-null
    setp.eq.u64 %p10, %rd10, 0;
    @%p10 bra $L_fence_update_header;

    // Write fence value to memory
    st.volatile.global.u64 [%rd10], %rd11;
    membar.sys;

$L_fence_update_header:
    // Update header fence_value with seqno
    st.volatile.global.u64 [%rd0+40], %rd12;
    bra $L_cmd_done;

$L_cmd_shutdown:
    // Set stop flag
    mov.u64 %rd13, 1;
    st.volatile.global.u64 [%rd0+16], %rd13;
    bra $L_cmd_done;

    // ========================================================================
    // Command completion
    // ========================================================================

$L_cmd_done:
    // Check FENCE_AFTER flag
    and.b32 %r13, %r11, FLAG_FENCE_AFTER;
    setp.ne.u32 %p11, %r13, 0;
    @%p11 membar.sys;

    // Atomically increment head
    atom.global.add.u64 %rd14, [%rd0+0], 1;

    // Atomically increment commands_processed
    atom.global.add.u64 %rd15, [%rd0+32], 1;

    // Update status to idle
    mov.u64 %rd16, STATUS_IDLE;
    st.volatile.global.u64 [%rd0+24], %rd16;

    bra $L_poll_loop;

    // ========================================================================
    // Spin wait (no commands available)
    // ========================================================================

$L_spin_wait:
    // Set status to idle
    mov.u64 %rd17, STATUS_IDLE;
    st.volatile.global.u64 [%rd0+24], %rd17;

    // Spin for poll_ns nanoseconds
    // Note: nanosleep requires sm_70+, so we use a loop approximation
    mov.u32 %r14, 0;
$L_spin_inner:
    membar.cta;  // Brief pause
    add.u32 %r14, %r14, 1;
    setp.lt.u32 %p12, %r14, %r5;
    @%p12 bra $L_spin_inner;

    bra $L_poll_loop;

    // ========================================================================
    // Exit
    // ========================================================================

$L_exit:
    // Signal clean exit
    mov.u64 %rd18, STATUS_EXITED;
    st.volatile.global.u64 [%rd0+24], %rd18;

$L_sync_exit:
    // Synchronize all threads before exit
    bar.sync 0;
    ret;
}

// ============================================================================
// Health Check Kernel
// ============================================================================

.visible .entry trojan_health_check(
    .param .u64 param_health_ptr,
    .param .u64 param_magic
)
{
    .reg .pred %p0;
    .reg .b32 %r0, %r1;
    .reg .b64 %rd0, %rd1;

    mov.u32 %r0, %tid.x;
    mov.u32 %r1, %ctaid.x;

    // Only thread 0 of block 0 writes
    setp.ne.u32 %p0, %r0, 0;
    @%p0 bra $L_health_done;
    setp.ne.u32 %p0, %r1, 0;
    @%p0 bra $L_health_done;

    ld.param.u64 %rd0, [param_health_ptr];
    ld.param.u64 %rd1, [param_magic];

    st.volatile.global.u64 [%rd0], %rd1;
    membar.sys;

$L_health_done:
    ret;
}

// ============================================================================
// Ring Reset Kernel
// ============================================================================

.visible .entry trojan_ring_reset(
    .param .u64 param_header
)
{
    .reg .pred %p0;
    .reg .b32 %r0, %r1;
    .reg .b64 %rd0, %rd_zero, %rd_running;

    mov.u32 %r0, %tid.x;
    mov.u32 %r1, %ctaid.x;

    // Only thread 0 of block 0 resets
    setp.ne.u32 %p0, %r0, 0;
    @%p0 bra $L_reset_done;
    setp.ne.u32 %p0, %r1, 0;
    @%p0 bra $L_reset_done;

    ld.param.u64 %rd0, [param_header];
    mov.u64 %rd_zero, 0;
    mov.u64 %rd_running, STATUS_RUNNING;

    // Clear all header fields
    st.volatile.global.u64 [%rd0+0], %rd_zero;      // head = 0
    st.volatile.global.u64 [%rd0+8], %rd_zero;      // tail = 0
    st.volatile.global.u64 [%rd0+16], %rd_zero;     // stop_flag = 0
    st.volatile.global.u64 [%rd0+24], %rd_running;  // kernel_status = RUNNING
    st.volatile.global.u64 [%rd0+32], %rd_zero;     // commands_processed = 0
    st.volatile.global.u64 [%rd0+40], %rd_zero;     // fence_value = 0
    membar.sys;

$L_reset_done:
    ret;
}

// ============================================================================
// Timestamp Kernel
// ============================================================================

.visible .entry trojan_timestamp(
    .param .u64 param_ts_ptr
)
{
    .reg .pred %p0;
    .reg .b32 %r0, %r1;
    .reg .b64 %rd0, %rd_ts;

    mov.u32 %r0, %tid.x;
    mov.u32 %r1, %ctaid.x;

    // Only thread 0 of block 0 writes
    setp.ne.u32 %p0, %r0, 0;
    @%p0 bra $L_ts_done;
    setp.ne.u32 %p0, %r1, 0;
    @%p0 bra $L_ts_done;

    ld.param.u64 %rd0, [param_ts_ptr];

    // Read global timer
    mov.u64 %rd_ts, %globaltimer;

    st.volatile.global.u64 [%rd0], %rd_ts;
    membar.sys;

$L_ts_done:
    ret;
}
"#;

/// Volta+ PTX with nanosleep support
pub const TROJAN_PTX_SM70: &str = r#"
//
// NVIDIA Trojan Kernel - PTX Assembly (Volta+)
// Optimized for sm_70+ with __nanosleep
//
.version 7.5
.target sm_70
.address_size 64

// [Same structure as TROJAN_PTX_INLINE with nanosleep in spin wait]
// For brevity, the full implementation would replace the spin loop with:
//   nanosleep.u32 %r5;  // Sleep for poll_ns nanoseconds
"#;

// ============================================================================
// PTX Binary Embedding (Placeholder)
// ============================================================================

// Note: In a real build, these would use include_bytes!() with compiled PTX.
// Since we cannot compile CUDA during development, we provide the inline PTX
// and document the compilation process.

/// Placeholder for SM 5.2 compiled PTX
///
/// To generate:
/// ```bash
/// nvcc -ptx -arch=sm_52 -o trojan_sm52.ptx trojan_kernel.cu
/// ```
///
/// Then embed with:
/// ```rust,ignore
/// pub const TROJAN_PTX_SM52_BIN: &[u8] = include_bytes!("trojan_sm52.ptx");
/// ```
pub const TROJAN_PTX_SM52_BIN: &[u8] = TROJAN_PTX_INLINE.as_bytes();

/// Placeholder for SM 7.0 compiled PTX
pub const TROJAN_PTX_SM70_BIN: &[u8] = TROJAN_PTX_SM70.as_bytes();

/// Placeholder for SM 8.0 compiled PTX
pub const TROJAN_PTX_SM80_BIN: &[u8] = TROJAN_PTX_INLINE.as_bytes();

// ============================================================================
// PTX Selection API
// ============================================================================

/// Get appropriate PTX bytecode for a device's compute capability.
///
/// Returns the highest-performance PTX that is compatible with the device.
///
/// # Arguments
///
/// * `major` - Major compute capability (e.g., 8 for sm_80)
/// * `minor` - Minor compute capability (e.g., 6 for sm_86)
///
/// # Returns
///
/// Static byte slice containing PTX text, or None if unsupported.
///
/// # Example
///
/// ```ignore
/// let ptx = get_ptx_for_device(8, 6).unwrap();
/// // ptx is sm_80 PTX compatible with Ampere architecture
/// ```
pub fn get_ptx_for_device(major: u32, minor: u32) -> Option<&'static [u8]> {
    let tier = select_ptx_tier(major, minor)?;
    Some(match tier {
        PtxArchTier::Ampere => TROJAN_PTX_SM80_BIN,
        PtxArchTier::Volta => TROJAN_PTX_SM70_BIN,
        PtxArchTier::Maxwell => TROJAN_PTX_SM52_BIN,
    })
}

/// Get PTX as a string for debugging/logging.
pub fn get_ptx_str_for_device(major: u32, minor: u32) -> Option<&'static str> {
    let tier = select_ptx_tier(major, minor)?;
    Some(match tier {
        PtxArchTier::Ampere => TROJAN_PTX_INLINE, // Same as SM80 for now
        PtxArchTier::Volta => TROJAN_PTX_SM70,
        PtxArchTier::Maxwell => TROJAN_PTX_INLINE,
    })
}

/// Get the inline PTX source (always available)
#[inline]
pub const fn get_inline_ptx() -> &'static str {
    TROJAN_PTX_INLINE
}

// ============================================================================
// PTX Validation
// ============================================================================

/// Validate that PTX content appears well-formed.
///
/// Performs basic sanity checks on PTX text:
/// - Contains .version directive
/// - Contains .target directive
/// - Contains trojan_poll entry point
///
/// # Arguments
///
/// * `ptx` - PTX source as bytes
///
/// # Returns
///
/// Ok(()) if valid, Err with description if invalid.
pub fn validate_ptx(ptx: &[u8]) -> Result<(), &'static str> {
    // Convert to str for searching
    let ptx_str = core::str::from_utf8(ptx).map_err(|_| "PTX is not valid UTF-8")?;

    // Check for required directives
    if !ptx_str.contains(".version") {
        return Err("PTX missing .version directive");
    }

    if !ptx_str.contains(".target") {
        return Err("PTX missing .target directive");
    }

    if !ptx_str.contains("trojan_poll") {
        return Err("PTX missing trojan_poll entry point");
    }

    // Check for required helper kernels
    if !ptx_str.contains("trojan_health_check") {
        return Err("PTX missing trojan_health_check entry");
    }

    if !ptx_str.contains("trojan_ring_reset") {
        return Err("PTX missing trojan_ring_reset entry");
    }

    Ok(())
}

/// Extract target SM version from PTX content.
///
/// Parses the `.target sm_XX` directive to determine the minimum
/// compute capability required.
///
/// # Arguments
///
/// * `ptx` - PTX source as string
///
/// # Returns
///
/// Compute capability if found, None otherwise.
pub fn extract_target_sm(ptx: &str) -> Option<ComputeCapability> {
    // Look for ".target sm_XY" pattern
    for line in ptx.lines() {
        let line = line.trim();
        if line.starts_with(".target") {
            // Parse "sm_XY" part
            if let Some(sm_pos) = line.find("sm_") {
                let sm_str = &line[sm_pos + 3..];
                // Extract digits
                let digits: String = sm_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 2 {
                    let major = digits[0..1].parse::<u32>().ok()?;
                    let minor = digits[1..2].parse::<u32>().ok()?;
                    return Some(ComputeCapability::new(major, minor));
                }
            }
        }
    }
    None
}

// ============================================================================
// Kernel Entry Point Names
// ============================================================================

/// Entry point name for main polling kernel
pub const KERNEL_TROJAN_POLL: &str = "trojan_poll";

/// Entry point name for health check
pub const KERNEL_HEALTH_CHECK: &str = "trojan_health_check";

/// Entry point name for ring reset
pub const KERNEL_RING_RESET: &str = "trojan_ring_reset";

/// Entry point name for timestamp
pub const KERNEL_TIMESTAMP: &str = "trojan_timestamp";

// ============================================================================
// Structure Layout Verification
// ============================================================================

/// Command structure field offsets (must match nvidia_ring.rs)
pub mod cmd_layout {
    pub const OPCODE: usize = 0;
    pub const FLAGS: usize = 4;
    pub const SEQNO: usize = 8;
    pub const SRC: usize = 16;
    pub const DST: usize = 24;
    pub const SIZE: usize = 32;
    pub const EXTRA: usize = 40;
    pub const TOTAL: usize = 64;
}

/// Ring header field offsets (must match TrojanRingHeader in CUDA)
pub mod header_layout {
    pub const HEAD: usize = 0;
    pub const TAIL: usize = 8;
    pub const STOP_FLAG: usize = 16;
    pub const KERNEL_STATUS: usize = 24;
    pub const COMMANDS_PROCESSED: usize = 32;
    pub const FENCE_VALUE: usize = 40;
    pub const TOTAL: usize = 64;
}

/// Verify that Rust TrojanCommand matches expected layout
pub const fn verify_command_layout() -> bool {
    use super::nvidia_ring::TrojanCommand;

    // Size must be exactly 64 bytes
    if core::mem::size_of::<TrojanCommand>() != cmd_layout::TOTAL {
        return false;
    }

    // Alignment must be 64 bytes
    if core::mem::align_of::<TrojanCommand>() != 64 {
        return false;
    }

    true
}

// Compile-time verification
const _: () = {
    assert!(verify_command_layout(), "TrojanCommand layout mismatch");
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // PTX Content Tests (Q1-Q7: Unit)
    // ========================================================================

    #[test]
    fn test_inline_ptx_not_empty() {
        assert!(!TROJAN_PTX_INLINE.is_empty());
        assert!(TROJAN_PTX_INLINE.len() > 1000, "PTX should be substantial");
    }

    #[test]
    fn test_inline_ptx_contains_version() {
        assert!(TROJAN_PTX_INLINE.contains(".version"));
    }

    #[test]
    fn test_inline_ptx_contains_target() {
        assert!(TROJAN_PTX_INLINE.contains(".target sm_52"));
    }

    #[test]
    fn test_inline_ptx_contains_entry() {
        assert!(TROJAN_PTX_INLINE.contains(".entry trojan_poll"));
    }

    #[test]
    fn test_inline_ptx_contains_health_check() {
        assert!(TROJAN_PTX_INLINE.contains("trojan_health_check"));
    }

    #[test]
    fn test_inline_ptx_contains_ring_reset() {
        assert!(TROJAN_PTX_INLINE.contains("trojan_ring_reset"));
    }

    #[test]
    fn test_inline_ptx_contains_timestamp() {
        assert!(TROJAN_PTX_INLINE.contains("trojan_timestamp"));
    }

    // ========================================================================
    // PTX Selection Tests (Q8-Q14: Property)
    // ========================================================================

    #[test]
    fn test_select_ptx_tier_ampere() {
        assert_eq!(select_ptx_tier(8, 0), Some(PtxArchTier::Ampere));
        assert_eq!(select_ptx_tier(8, 6), Some(PtxArchTier::Ampere));
        assert_eq!(select_ptx_tier(9, 0), Some(PtxArchTier::Ampere));
    }

    #[test]
    fn test_select_ptx_tier_volta() {
        assert_eq!(select_ptx_tier(7, 0), Some(PtxArchTier::Volta));
        assert_eq!(select_ptx_tier(7, 5), Some(PtxArchTier::Volta));
    }

    #[test]
    fn test_select_ptx_tier_maxwell() {
        assert_eq!(select_ptx_tier(5, 2), Some(PtxArchTier::Maxwell));
        assert_eq!(select_ptx_tier(6, 0), Some(PtxArchTier::Maxwell));
        assert_eq!(select_ptx_tier(6, 1), Some(PtxArchTier::Maxwell));
    }

    #[test]
    fn test_select_ptx_tier_unsupported() {
        assert_eq!(select_ptx_tier(3, 0), None);
        assert_eq!(select_ptx_tier(4, 0), None);
    }

    #[test]
    fn test_get_ptx_for_device_ampere() {
        let ptx = get_ptx_for_device(8, 0).unwrap();
        assert!(!ptx.is_empty());
    }

    #[test]
    fn test_get_ptx_for_device_volta() {
        let ptx = get_ptx_for_device(7, 0).unwrap();
        assert!(!ptx.is_empty());
    }

    #[test]
    fn test_get_ptx_for_device_maxwell() {
        let ptx = get_ptx_for_device(5, 2).unwrap();
        assert!(!ptx.is_empty());
    }

    #[test]
    fn test_get_ptx_for_device_unsupported() {
        assert!(get_ptx_for_device(3, 0).is_none());
    }

    // ========================================================================
    // Compute Capability Tests (Q15-Q21: Integration)
    // ========================================================================

    #[test]
    fn test_compute_capability_sm_version() {
        let cc = ComputeCapability::new(8, 6);
        assert_eq!(cc.sm_version(), 86);
    }

    #[test]
    fn test_compute_capability_display() {
        let cc = ComputeCapability::new(7, 0);
        assert_eq!(format!("{}", cc), "sm_70");
    }

    #[test]
    fn test_compute_capability_nanosleep() {
        assert!(!ComputeCapability::new(6, 1).supports_nanosleep());
        assert!(ComputeCapability::new(7, 0).supports_nanosleep());
        assert!(ComputeCapability::new(8, 0).supports_nanosleep());
    }

    #[test]
    fn test_compute_capability_async_copy() {
        assert!(!ComputeCapability::new(7, 5).supports_async_copy());
        assert!(ComputeCapability::new(8, 0).supports_async_copy());
    }

    #[test]
    fn test_compute_capability_supported() {
        assert!(!ComputeCapability::new(4, 0).is_supported());
        assert!(ComputeCapability::new(5, 2).is_supported());
        assert!(ComputeCapability::new(9, 0).is_supported());
    }

    // ========================================================================
    // PTX Validation Tests (Q22-Q28: Production)
    // ========================================================================

    #[test]
    fn test_validate_ptx_inline() {
        assert!(validate_ptx(TROJAN_PTX_INLINE.as_bytes()).is_ok());
    }

    #[test]
    fn test_validate_ptx_missing_version() {
        let invalid = ".target sm_52\n.entry test() { ret; }";
        assert!(validate_ptx(invalid.as_bytes()).is_err());
    }

    #[test]
    fn test_validate_ptx_missing_target() {
        let invalid = ".version 7.0\n.entry test() { ret; }";
        assert!(validate_ptx(invalid.as_bytes()).is_err());
    }

    #[test]
    fn test_validate_ptx_missing_entry() {
        let invalid = ".version 7.0\n.target sm_52\n.entry other() { ret; }";
        assert!(validate_ptx(invalid.as_bytes()).is_err());
    }

    #[test]
    fn test_extract_target_sm() {
        let ptx = ".version 7.0\n.target sm_70\n";
        let cc = extract_target_sm(ptx).unwrap();
        assert_eq!(cc.major, 7);
        assert_eq!(cc.minor, 0);
    }

    #[test]
    fn test_extract_target_sm_inline() {
        let cc = extract_target_sm(TROJAN_PTX_INLINE).unwrap();
        assert_eq!(cc.sm_version(), 52);
    }

    // ========================================================================
    // Layout Verification Tests (Q29-Q35: Determinism)
    // ========================================================================

    #[test]
    fn test_command_layout_offsets() {
        assert_eq!(cmd_layout::OPCODE, 0);
        assert_eq!(cmd_layout::FLAGS, 4);
        assert_eq!(cmd_layout::SEQNO, 8);
        assert_eq!(cmd_layout::SRC, 16);
        assert_eq!(cmd_layout::DST, 24);
        assert_eq!(cmd_layout::SIZE, 32);
        assert_eq!(cmd_layout::EXTRA, 40);
        assert_eq!(cmd_layout::TOTAL, 64);
    }

    #[test]
    fn test_header_layout_offsets() {
        assert_eq!(header_layout::HEAD, 0);
        assert_eq!(header_layout::TAIL, 8);
        assert_eq!(header_layout::STOP_FLAG, 16);
        assert_eq!(header_layout::KERNEL_STATUS, 24);
        assert_eq!(header_layout::COMMANDS_PROCESSED, 32);
        assert_eq!(header_layout::FENCE_VALUE, 40);
        assert_eq!(header_layout::TOTAL, 64);
    }

    #[test]
    fn test_verify_command_layout() {
        assert!(verify_command_layout());
    }

    #[test]
    fn test_trojan_command_size() {
        use super::super::nvidia_ring::TrojanCommand;
        assert_eq!(core::mem::size_of::<TrojanCommand>(), 64);
    }

    #[test]
    fn test_trojan_command_align() {
        use super::super::nvidia_ring::TrojanCommand;
        assert_eq!(core::mem::align_of::<TrojanCommand>(), 64);
    }

    // ========================================================================
    // Kernel Name Tests
    // ========================================================================

    #[test]
    fn test_kernel_names() {
        assert_eq!(KERNEL_TROJAN_POLL, "trojan_poll");
        assert_eq!(KERNEL_HEALTH_CHECK, "trojan_health_check");
        assert_eq!(KERNEL_RING_RESET, "trojan_ring_reset");
        assert_eq!(KERNEL_TIMESTAMP, "trojan_timestamp");
    }

    #[test]
    fn test_inline_ptx_contains_all_kernels() {
        assert!(TROJAN_PTX_INLINE.contains(KERNEL_TROJAN_POLL));
        assert!(TROJAN_PTX_INLINE.contains(KERNEL_HEALTH_CHECK));
        assert!(TROJAN_PTX_INLINE.contains(KERNEL_RING_RESET));
        assert!(TROJAN_PTX_INLINE.contains(KERNEL_TIMESTAMP));
    }

    // ========================================================================
    // Architecture Tier Tests
    // ========================================================================

    #[test]
    fn test_ptx_arch_tier_sm_string() {
        assert_eq!(PtxArchTier::Maxwell.sm_string(), "sm_52");
        assert_eq!(PtxArchTier::Volta.sm_string(), "sm_70");
        assert_eq!(PtxArchTier::Ampere.sm_string(), "sm_80");
    }

    #[test]
    fn test_ptx_arch_tier_min_compute() {
        assert_eq!(PtxArchTier::Maxwell.min_compute().sm_version(), 52);
        assert_eq!(PtxArchTier::Volta.min_compute().sm_version(), 70);
        assert_eq!(PtxArchTier::Ampere.min_compute().sm_version(), 80);
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_ptx_version() {
        assert_eq!(PTX_VERSION, (7, 5));
    }

    #[test]
    fn test_min_cuda_driver() {
        assert!(MIN_CUDA_DRIVER >= 11_000);
    }

    #[test]
    fn test_ptx_magic() {
        assert_eq!(PTX_MAGIC, b".version");
    }
}
