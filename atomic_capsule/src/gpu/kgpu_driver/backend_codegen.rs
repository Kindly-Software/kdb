//! Backend Code Generation - Multi-Vendor GPU Code Generation
//!
//! **Tier**: T1 Atomic (256B aligned CodegenCapsule)
//! **Purpose**: Generate native GPU instructions from ShaderIR for Intel Gen, AMD GCN, and NVIDIA PTX
//!
//! # Architecture
//!
//! This module provides backend code generation for three GPU architectures:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────────────┐
//! │                           ShaderIR Input                              │
//! │   (From SPIR-V parser: types, constants, instructions)                │
//! └───────────────────────────────┬───────────────────────────────────────┘
//!                                 │
//!           ┌─────────────────────┼─────────────────────┐
//!           ▼                     ▼                     ▼
//! ┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
//! │   IntelGenBackend │ │   AmdGcnBackend   │ │  NvidiaPtxBackend │
//! │                   │ │                   │ │                   │
//! │   GenOpcode:      │ │   VopOpcode:      │ │   PTX Text:       │
//! │   - Mov           │ │   - V_MOV         │ │   - add.f32       │
//! │   - Add           │ │   - V_ADD_F32     │ │   - mul.f32       │
//! │   - Mul           │ │   - V_MUL_F32     │ │   - mad.f32       │
//! │   - Mad           │ │   - V_MAD_F32     │ │   - ld.global     │
//! │   - Send          │ │   - S_WAITCNT     │ │   - st.global     │
//! └───────────────────┘ └───────────────────┘ └───────────────────┘
//!           │                     │                     │
//!           ▼                     ▼                     ▼
//! ┌────────────────────────────────────────────────────────────────────────┐
//! │                       GeneratedCode Output                            │
//! │   - Binary machine code (Intel/AMD) or PTX text (NVIDIA)              │
//! │   - Register allocation info                                          │
//! └────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Memory Layout (CodegenCapsule - 256B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       Primary: state(8) | instructions_generated(24) | generation(32)
//! 8       8       Secondary: registers_used(16) | error_count(16) | reserved(32)
//! 16      8       ir_ptr: AtomicPtr to ShaderIR
//! 24      8       output_ptr: AtomicPtr to GeneratedCode buffer
//! 32      8       output_capacity: Maximum output size
//! 40      8       current_offset: Current write position
//! 48      4       target_vendor: GpuVendor enum
//! 52      4       optimization_level: 0-3
//! 56      200     _padding (to 256B)
//! ```
//!
//! # Supported Backends
//!
//! | Backend | Architecture | Output Format | Registers |
//! |---------|--------------|---------------|-----------|
//! | Intel Gen | Gen9+ EU | Binary MI | 128 GRF |
//! | AMD GCN | RDNA/GCN | Binary PM4 | 256 VGPR |
//! | NVIDIA PTX | sm_52+ | PTX Text | 255 |
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree state tracking)
//! - Q11: Rust transform (type-safe opcode generation)
//! - Q33: #[derive(ComputationalCapsule)] mandate
//! - Q34: Audit trail for code generation
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_IR_VALID`: ShaderIR has been validated before codegen
//! - `#ASSUME_REGISTER_SUFFICIENT`: Register count within architecture limits
//! - `#ASSUME_OPCODE_ENCODING_STABLE`: ISA opcode encodings are stable
//!
//! # Feature Flags
//!
//! - `kgpu-driver`: Enable the driver module (required)
//! - `kgpu-driver-intel`: Enable Intel Gen backend
//! - `kgpu-driver-amd`: Enable AMD GCN backend
//! - `kgpu-driver-nvidia`: Enable NVIDIA PTX backend

#![allow(dead_code)] // During development

use core::sync::atomic::{AtomicU64, AtomicPtr, AtomicU32, Ordering};
use core::marker::PhantomData;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::spirv_parser::{ShaderIr, ShaderIrOpKind};
use super::vendor::GpuVendor;

// ============================================================================
// Constants
// ============================================================================

/// Maximum code size (16 MB)
pub const MAX_CODE_SIZE: usize = 16 * 1024 * 1024;

/// Default register count per architecture
pub const DEFAULT_INTEL_REGISTERS: u32 = 128; // GRF (General Register File)
pub const DEFAULT_AMD_VGPR: u32 = 256;        // Vector GPRs
pub const DEFAULT_AMD_SGPR: u32 = 104;        // Scalar GPRs
pub const DEFAULT_NVIDIA_REGISTERS: u32 = 255; // PTX max registers

// ============================================================================
// Codegen State
// ============================================================================

/// Code generation state machine
///
/// # ASSUM Safety
/// `#ASSUME_STATE_TRANSITIONS_VALID`: State transitions follow: Idle -> Generating -> Complete/Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CodegenState {
    /// No code generation in progress
    Idle = 0,
    /// Code generation in progress
    Generating = 1,
    /// Code generation complete
    Complete = 2,
    /// Code generation failed
    Error = 3,
}

impl CodegenState {
    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Generating,
            2 => Self::Complete,
            3 => Self::Error,
            _ => Self::Error,
        }
    }
}

// ============================================================================
// Generated Code Output
// ============================================================================

/// Generated code output from a backend
///
/// Contains either binary machine code (Intel/AMD) or PTX text (NVIDIA).
#[derive(Debug, Clone)]
pub struct GeneratedCode {
    /// Binary machine code (for Intel Gen, AMD GCN)
    pub code: Vec<u8>,
    /// PTX text representation (for NVIDIA, optional for others)
    pub text: Option<String>,
    /// Number of registers used
    pub registers_used: u32,
    /// Number of instructions generated
    pub instruction_count: u32,
    /// Target architecture name
    pub target_arch: &'static str,
}

impl GeneratedCode {
    /// Create new empty generated code
    #[inline]
    pub fn new(target_arch: &'static str) -> Self {
        Self {
            code: Vec::new(),
            text: None,
            registers_used: 0,
            instruction_count: 0,
            target_arch,
        }
    }

    /// Create with pre-allocated capacity
    #[inline]
    pub fn with_capacity(target_arch: &'static str, capacity: usize) -> Self {
        Self {
            code: Vec::with_capacity(capacity),
            text: None,
            registers_used: 0,
            instruction_count: 0,
            target_arch,
        }
    }

    /// Check if code is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.code.is_empty() && self.text.is_none()
    }

    /// Get code size in bytes
    #[inline]
    pub fn size(&self) -> usize {
        self.code.len() + self.text.as_ref().map_or(0, |t| t.len())
    }
}

impl Default for GeneratedCode {
    fn default() -> Self {
        Self::new("unknown")
    }
}

// ============================================================================
// Codegen Backend Trait
// ============================================================================

/// Trait for GPU code generation backends
///
/// Each backend implements this trait to generate native code for its target architecture.
///
/// # ASSUM Safety
/// `#ASSUME_IR_VALID`: Input ShaderIR has been validated.
/// `#VERIFY_IR_VALID`: Caller must validate IR before calling generate().
pub trait CodegenBackend {
    /// Backend name for identification
    fn name(&self) -> &'static str;

    /// Generate native code from ShaderIR
    ///
    /// Returns generated code or error message.
    ///
    /// # Arguments
    /// * `ir` - Validated ShaderIR from SPIR-V parser
    ///
    /// # Returns
    /// * `Ok(GeneratedCode)` - Successfully generated code
    /// * `Err(String)` - Error description
    fn generate(&self, ir: &ShaderIr) -> Result<GeneratedCode, String>;

    /// Get maximum register count for this architecture
    fn register_count(&self) -> u32;

    /// Get target architecture description
    fn target_description(&self) -> &'static str {
        self.name()
    }
}

// ============================================================================
// Codegen Capsule (T1 Atomic, 256B aligned)
// ============================================================================

/// Code generation tracking capsule
///
/// Tracks state of code generation atomically for concurrent access.
///
/// # Tier: T1 Atomic
/// # Size: 256B (cache-line aligned)
///
/// # Memory Layout
/// ```text
/// Offset  Size    Field
/// 0       8       Primary: state(8) | instructions_generated(24) | generation(32)
/// 8       8       Secondary: registers_used(16) | error_count(16) | reserved(32)
/// 16      8       ir_ptr: AtomicPtr to ShaderIR
/// 24      8       output_ptr: AtomicPtr to output buffer
/// 32      8       output_capacity
/// 40      8       current_offset
/// 48      4       target_vendor
/// 52      4       optimization_level
/// 56      200     _padding
/// ```
#[repr(C, align(256))]
pub struct CodegenCapsule {
    /// Primary: state(8) | instructions_generated(24) | generation(32)
    primary: AtomicU64,
    /// Secondary: registers_used(16) | error_count(16) | reserved(32)
    secondary: AtomicU64,
    /// Pointer to ShaderIR input
    ir_ptr: AtomicPtr<ShaderIr>,
    /// Pointer to output buffer
    output_ptr: AtomicPtr<u8>,
    /// Output buffer capacity
    output_capacity: AtomicU64,
    /// Current write offset in output
    current_offset: AtomicU64,
    /// Target GPU vendor
    target_vendor: AtomicU32,
    /// Optimization level (0-3)
    optimization_level: AtomicU32,
    /// Padding to 256B
    _padding: [u8; 200],
    /// PhantomData for !Sync safety
    _marker: PhantomData<*const ()>,
}

// SAFETY: CodegenCapsule uses only atomic operations for all mutable state
unsafe impl Send for CodegenCapsule {}
unsafe impl Sync for CodegenCapsule {}

/// Snapshot of codegen state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodegenSnapshot {
    /// Current state
    pub state: CodegenState,
    /// Instructions generated so far
    pub instructions_generated: u32,
    /// Generation counter for ABA prevention
    pub generation: u32,
    /// Registers used
    pub registers_used: u16,
    /// Error count
    pub error_count: u16,
}

impl CodegenCapsule {
    /// Create new codegen capsule
    ///
    /// # ASSUM Safety
    /// `#ASSUME_INITIAL_STATE_VALID`: Initial state is Idle with zero counters.
    #[inline]
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            ir_ptr: AtomicPtr::new(core::ptr::null_mut()),
            output_ptr: AtomicPtr::new(core::ptr::null_mut()),
            output_capacity: AtomicU64::new(0),
            current_offset: AtomicU64::new(0),
            target_vendor: AtomicU32::new(0),
            optimization_level: AtomicU32::new(2), // Default O2
            _padding: [0u8; 200],
            _marker: PhantomData,
        }
    }

    /// Take atomic snapshot of state
    ///
    /// # ASSUM Safety
    /// `#ASSUME_SNAPSHOT_CONSISTENT`: Acquire ordering ensures visibility.
    #[inline]
    pub fn snapshot(&self) -> CodegenSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        CodegenSnapshot {
            state: CodegenState::from_u8((primary & 0xFF) as u8),
            instructions_generated: ((primary >> 8) & 0xFFFFFF) as u32,
            generation: ((primary >> 32) & 0xFFFFFFFF) as u32,
            registers_used: (secondary & 0xFFFF) as u16,
            error_count: ((secondary >> 16) & 0xFFFF) as u16,
        }
    }

    /// Set state atomically
    #[inline]
    pub fn set_state(&self, state: CodegenState) {
        let current = self.primary.load(Ordering::Acquire);
        let new_primary = (current & !0xFF) | (state as u64);
        self.primary.store(new_primary, Ordering::Release);
    }

    /// Increment generation counter
    #[inline]
    pub fn increment_generation(&self) {
        let current = self.primary.load(Ordering::Acquire);
        let gen = ((current >> 32) & 0xFFFFFFFF) + 1;
        let new_primary = (current & 0xFFFFFFFF) | (gen << 32);
        self.primary.store(new_primary, Ordering::Release);
    }

    /// Add generated instructions count
    #[inline]
    pub fn add_instructions(&self, count: u32) {
        let current = self.primary.load(Ordering::Acquire);
        let old_count = ((current >> 8) & 0xFFFFFF) as u32;
        let new_count = old_count.saturating_add(count).min(0xFFFFFF);
        let new_primary = (current & !0xFFFFFF00) | ((new_count as u64) << 8);
        self.primary.store(new_primary, Ordering::Release);
    }

    /// Set registers used
    #[inline]
    pub fn set_registers_used(&self, count: u16) {
        let current = self.secondary.load(Ordering::Acquire);
        let new_secondary = (current & !0xFFFF) | (count as u64);
        self.secondary.store(new_secondary, Ordering::Release);
    }

    /// Increment error count
    #[inline]
    pub fn increment_error_count(&self) {
        let current = self.secondary.load(Ordering::Acquire);
        let err = ((current >> 16) & 0xFFFF) + 1;
        let new_secondary = (current & !0xFFFF0000) | (err << 16);
        self.secondary.store(new_secondary, Ordering::Release);
    }

    /// Set target vendor
    #[inline]
    pub fn set_vendor(&self, vendor: GpuVendor) {
        self.target_vendor.store(vendor as u32, Ordering::Release);
    }

    /// Get target vendor
    #[inline]
    pub fn get_vendor(&self) -> GpuVendor {
        let v = self.target_vendor.load(Ordering::Acquire);
        // Map to vendor enum
        match v {
            0x8086 => GpuVendor::Intel,
            0x1002 => GpuVendor::Amd,
            0x10DE => GpuVendor::Nvidia,
            _ => GpuVendor::Unknown,
        }
    }

    /// Set optimization level
    #[inline]
    pub fn set_optimization_level(&self, level: u32) {
        self.optimization_level.store(level.min(3), Ordering::Release);
    }

    /// Get optimization level
    #[inline]
    pub fn get_optimization_level(&self) -> u32 {
        self.optimization_level.load(Ordering::Acquire)
    }
}

impl Default for CodegenCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Verify capsule size at compile time
const _: () = assert!(core::mem::size_of::<CodegenCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<CodegenCapsule>() == 256);

// ============================================================================
// Intel Gen Backend
// ============================================================================

/// Intel Gen EU opcodes
///
/// Subset of Gen9+ EU instruction opcodes for shader execution.
///
/// # ASSUM Safety
/// `#ASSUME_GEN_OPCODE_STABLE`: Intel Gen ISA opcodes are stable within Gen9+ family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GenOpcode {
    /// Move operation
    Mov = 0x01,
    /// Add (integer/float)
    Add = 0x40,
    /// Multiply (integer/float)
    Mul = 0x41,
    /// Multiply-add (MAD: a * b + c)
    Mad = 0x5D,
    /// Compare
    Cmp = 0x10,
    /// Jump
    Jmp = 0x20,
    /// Call subroutine
    Call = 0x2C,
    /// Return from subroutine
    Ret = 0x2E,
    /// Send message (sampler, render, etc.)
    Send = 0x31,
    /// No operation
    Nop = 0x7E,
}

impl GenOpcode {
    /// Get opcode name
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Mov => "mov",
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Mad => "mad",
            Self::Cmp => "cmp",
            Self::Jmp => "jmp",
            Self::Call => "call",
            Self::Ret => "ret",
            Self::Send => "send",
            Self::Nop => "nop",
        }
    }

    /// Map ShaderIrOpKind to GenOpcode
    #[inline]
    pub fn from_ir_op(kind: ShaderIrOpKind) -> Option<Self> {
        match kind {
            ShaderIrOpKind::Load | ShaderIrOpKind::Store => Some(Self::Mov),
            ShaderIrOpKind::Arithmetic => Some(Self::Add), // Simplified; real impl would check sub-op
            ShaderIrOpKind::Compare => Some(Self::Cmp),
            ShaderIrOpKind::ControlFlow => Some(Self::Jmp),
            ShaderIrOpKind::Texture => Some(Self::Send),
            _ => None,
        }
    }
}

/// Intel Gen backend for EU shader compilation
pub struct IntelGenBackend {
    /// Target Gen architecture (9, 11, 12, etc.)
    gen_version: u32,
}

impl IntelGenBackend {
    /// Create new Intel Gen backend
    #[inline]
    pub const fn new(gen_version: u32) -> Self {
        Self { gen_version }
    }

    /// Encode a Gen instruction to binary
    ///
    /// Gen instructions are variable length (1-4 words typically).
    /// This is a placeholder encoding.
    ///
    /// # ASSUM Safety
    /// `#ASSUME_GEN_ENCODING_VALID`: Encoding follows Gen ISA specification.
    fn encode_instruction(&self, opcode: GenOpcode, dst: u8, src0: u8, src1: u8) -> Vec<u8> {
        // Placeholder: Real encoding is complex (128-bit instructions)
        // This generates a simple 8-byte placeholder instruction
        let mut inst = Vec::with_capacity(8);

        // Word 0: opcode + control bits
        inst.push(opcode as u8);
        inst.push(0x00); // access mode, quarter control, etc.
        inst.push(dst);  // destination register
        inst.push(src0); // source 0

        // Word 1: source 1 + flags
        inst.push(src1);
        inst.push(0x00); // flags
        inst.push(0x00); // reserved
        inst.push(0x00); // reserved

        inst
    }

    /// Generate placeholder binary for all IR instructions
    fn generate_instructions(&self, ir: &ShaderIr) -> (Vec<u8>, u32, u32) {
        let mut code = Vec::with_capacity(ir.instructions.len() * 8);
        let mut registers_used = 0u32;
        let mut reg_counter = 0u8;

        for inst in &ir.instructions {
            if let Some(opcode) = GenOpcode::from_ir_op(inst.kind) {
                let dst = reg_counter;
                reg_counter = reg_counter.wrapping_add(1);
                let src0 = inst.operand_ids.first().map_or(0, |&id| (id & 0xFF) as u8);
                let src1 = inst.operand_ids.get(1).map_or(0, |&id| (id & 0xFF) as u8);

                let encoded = self.encode_instruction(opcode, dst, src0, src1);
                code.extend_from_slice(&encoded);
                registers_used = registers_used.max(reg_counter as u32);
            }
        }

        let instruction_count = ir.instructions.len() as u32;
        (code, registers_used, instruction_count)
    }
}

impl CodegenBackend for IntelGenBackend {
    fn name(&self) -> &'static str {
        "Intel Gen EU"
    }

    fn generate(&self, ir: &ShaderIr) -> Result<GeneratedCode, String> {
        let (code, registers_used, instruction_count) = self.generate_instructions(ir);

        if registers_used > DEFAULT_INTEL_REGISTERS {
            return Err(format!(
                "Register spill: {} used, {} available",
                registers_used, DEFAULT_INTEL_REGISTERS
            ));
        }

        Ok(GeneratedCode {
            code,
            text: None,
            registers_used,
            instruction_count,
            target_arch: "Intel Gen EU",
        })
    }

    fn register_count(&self) -> u32 {
        DEFAULT_INTEL_REGISTERS
    }

    fn target_description(&self) -> &'static str {
        match self.gen_version {
            9 => "Intel Gen9 (Skylake)",
            11 => "Intel Gen11 (Ice Lake)",
            12 => "Intel Gen12 (Xe)",
            _ => "Intel Gen (Unknown)",
        }
    }
}

// ============================================================================
// AMD GCN Backend
// ============================================================================

/// AMD GCN VOP opcodes
///
/// Subset of GCN/RDNA vector operation opcodes.
///
/// # ASSUM Safety
/// `#ASSUME_GCN_OPCODE_STABLE`: AMD GCN/RDNA opcodes are stable within architecture family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(non_camel_case_types)] // GCN convention uses uppercase with underscores
pub enum VopOpcode {
    /// Vector move
    V_MOV = 0x01,
    /// Vector add (float32)
    V_ADD_F32 = 0x03,
    /// Vector multiply (float32)
    V_MUL_F32 = 0x05,
    /// Vector multiply-add (float32)
    V_MAD_F32 = 0xD1,
    /// Vector compare
    V_CMP = 0x40,
    /// Scalar branch
    S_BRANCH = 0xA0,
    /// Scalar conditional branch
    S_CBRANCH = 0xA1,
    /// End program
    S_ENDPGM = 0xBF,
    /// Wait for memory operations
    S_WAITCNT = 0xBC,
    /// No operation
    S_NOP = 0xBE,
}

impl VopOpcode {
    /// Get opcode name
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::V_MOV => "v_mov_b32",
            Self::V_ADD_F32 => "v_add_f32",
            Self::V_MUL_F32 => "v_mul_f32",
            Self::V_MAD_F32 => "v_mad_f32",
            Self::V_CMP => "v_cmp",
            Self::S_BRANCH => "s_branch",
            Self::S_CBRANCH => "s_cbranch",
            Self::S_ENDPGM => "s_endpgm",
            Self::S_WAITCNT => "s_waitcnt",
            Self::S_NOP => "s_nop",
        }
    }

    /// Map ShaderIrOpKind to VopOpcode
    #[inline]
    pub fn from_ir_op(kind: ShaderIrOpKind) -> Option<Self> {
        match kind {
            ShaderIrOpKind::Load | ShaderIrOpKind::Store => Some(Self::V_MOV),
            ShaderIrOpKind::Arithmetic => Some(Self::V_ADD_F32), // Simplified
            ShaderIrOpKind::Compare => Some(Self::V_CMP),
            ShaderIrOpKind::ControlFlow => Some(Self::S_BRANCH),
            ShaderIrOpKind::Barrier => Some(Self::S_WAITCNT),
            _ => None,
        }
    }
}

/// AMD GCN backend for shader compilation
pub struct AmdGcnBackend {
    /// Target GCN architecture (gfx8, gfx9, gfx10, etc.)
    gfx_version: u32,
}

impl AmdGcnBackend {
    /// Create new AMD GCN backend
    #[inline]
    pub const fn new(gfx_version: u32) -> Self {
        Self { gfx_version }
    }

    /// Encode a GCN instruction to binary
    ///
    /// GCN uses multiple instruction formats (VOP1, VOP2, VOP3, etc.)
    /// This is a placeholder encoding using VOP1 format (32-bit).
    ///
    /// # ASSUM Safety
    /// `#ASSUME_GCN_ENCODING_VALID`: Encoding follows GCN ISA specification.
    fn encode_instruction(&self, opcode: VopOpcode, vdst: u8, src0: u8) -> Vec<u8> {
        // Placeholder: Real GCN encoding is format-dependent
        // VOP1 format: 32-bit [31:25=enc, 24:17=op, 16:9=vdst, 8:0=src0]
        let mut inst = Vec::with_capacity(4);

        // Simple VOP1-like encoding (placeholder)
        inst.push(src0);       // src0 [7:0]
        inst.push(vdst);       // vdst [15:8] (simplified)
        inst.push(opcode as u8); // opcode [23:16]
        inst.push(0x7E);       // VOP1 encoding marker [31:24]

        inst
    }

    /// Generate placeholder binary for all IR instructions
    fn generate_instructions(&self, ir: &ShaderIr) -> (Vec<u8>, u32, u32) {
        let mut code = Vec::with_capacity(ir.instructions.len() * 4);
        let mut vgpr_used = 0u32;
        let mut vgpr_counter = 0u8;

        for inst in &ir.instructions {
            if let Some(opcode) = VopOpcode::from_ir_op(inst.kind) {
                let vdst = vgpr_counter;
                vgpr_counter = vgpr_counter.wrapping_add(1);
                let src0 = inst.operand_ids.first().map_or(0, |&id| (id & 0xFF) as u8);

                let encoded = self.encode_instruction(opcode, vdst, src0);
                code.extend_from_slice(&encoded);
                vgpr_used = vgpr_used.max(vgpr_counter as u32);
            }
        }

        // Add S_ENDPGM at the end
        code.extend_from_slice(&self.encode_instruction(VopOpcode::S_ENDPGM, 0, 0));

        let instruction_count = ir.instructions.len() as u32 + 1; // +1 for S_ENDPGM
        (code, vgpr_used, instruction_count)
    }
}

impl CodegenBackend for AmdGcnBackend {
    fn name(&self) -> &'static str {
        "AMD GCN/RDNA"
    }

    fn generate(&self, ir: &ShaderIr) -> Result<GeneratedCode, String> {
        let (code, registers_used, instruction_count) = self.generate_instructions(ir);

        if registers_used > DEFAULT_AMD_VGPR {
            return Err(format!(
                "VGPR spill: {} used, {} available",
                registers_used, DEFAULT_AMD_VGPR
            ));
        }

        Ok(GeneratedCode {
            code,
            text: None,
            registers_used,
            instruction_count,
            target_arch: "AMD GCN/RDNA",
        })
    }

    fn register_count(&self) -> u32 {
        DEFAULT_AMD_VGPR
    }

    fn target_description(&self) -> &'static str {
        match self.gfx_version {
            8 => "AMD GCN 3 (gfx8, Polaris)",
            9 => "AMD GCN 5 (gfx9, Vega)",
            10 => "AMD RDNA (gfx10, Navi)",
            11 => "AMD RDNA 3 (gfx11)",
            _ => "AMD GCN/RDNA (Unknown)",
        }
    }
}

// ============================================================================
// NVIDIA PTX Backend
// ============================================================================

/// NVIDIA PTX backend for shader compilation
///
/// Unlike Intel and AMD backends that generate binary, PTX backend
/// generates human-readable PTX assembly text that is then compiled
/// by the NVIDIA driver (JIT) or NVCC.
pub struct NvidiaPtxBackend {
    /// Target SM version (52, 70, 75, 80, 86, 90)
    sm_version: u32,
}

impl NvidiaPtxBackend {
    /// Create new NVIDIA PTX backend
    #[inline]
    pub const fn new(sm_version: u32) -> Self {
        Self { sm_version }
    }

    /// Map ShaderIrOpKind to PTX instruction mnemonic
    #[inline]
    fn ir_op_to_ptx(kind: ShaderIrOpKind) -> Option<&'static str> {
        match kind {
            ShaderIrOpKind::Load => Some("ld.global"),
            ShaderIrOpKind::Store => Some("st.global"),
            ShaderIrOpKind::Arithmetic => Some("add.f32"),
            ShaderIrOpKind::Compare => Some("setp"),
            ShaderIrOpKind::ControlFlow => Some("bra"),
            ShaderIrOpKind::Texture => Some("tex.1d"),
            ShaderIrOpKind::Barrier => Some("bar.sync"),
            ShaderIrOpKind::Atomic => Some("atom.global.add"),
            _ => None,
        }
    }

    /// Generate PTX text from ShaderIR
    ///
    /// # ASSUM Safety
    /// `#ASSUME_PTX_SYNTAX_VALID`: Generated PTX follows NVIDIA PTX ISA spec.
    pub fn generate_ptx(&self, ir: &ShaderIr) -> (String, u32, u32) {
        let mut ptx = String::with_capacity(4096);
        let mut registers_used = 0u32;
        let mut reg_counter = 0u32;

        // PTX header
        ptx.push_str(".version 7.8\n");
        ptx.push_str(&format!(".target sm_{}\n", self.sm_version));
        ptx.push_str(".address_size 64\n\n");

        // Entry point
        ptx.push_str(".visible .entry shader_main(\n");
        ptx.push_str("    .param .u64 param_in,\n");
        ptx.push_str("    .param .u64 param_out\n");
        ptx.push_str(")\n{\n");

        // Register declarations (placeholder)
        ptx.push_str("    .reg .f32 %f<64>;\n");
        ptx.push_str("    .reg .u32 %r<64>;\n");
        ptx.push_str("    .reg .u64 %rd<16>;\n");
        ptx.push_str("    .reg .pred %p<8>;\n\n");

        // Load parameters
        ptx.push_str("    ld.param.u64 %rd0, [param_in];\n");
        ptx.push_str("    ld.param.u64 %rd1, [param_out];\n\n");

        // Generate instructions from IR
        for inst in &ir.instructions {
            if let Some(mnemonic) = Self::ir_op_to_ptx(inst.kind) {
                let dst_reg = reg_counter;
                reg_counter += 1;

                match inst.kind {
                    ShaderIrOpKind::Load => {
                        ptx.push_str(&format!("    {}.f32 %f{}, [%rd0];\n", mnemonic, dst_reg));
                    }
                    ShaderIrOpKind::Store => {
                        ptx.push_str(&format!("    st.global.f32 [%rd1], %f{};\n", dst_reg.saturating_sub(1)));
                    }
                    ShaderIrOpKind::Arithmetic => {
                        let src1 = dst_reg.saturating_sub(1);
                        let src2 = dst_reg.saturating_sub(2);
                        ptx.push_str(&format!("    add.f32 %f{}, %f{}, %f{};\n", dst_reg, src1, src2));
                    }
                    ShaderIrOpKind::Compare => {
                        let src1 = dst_reg.saturating_sub(1);
                        ptx.push_str(&format!("    setp.gt.f32 %p0, %f{}, 0.0;\n", src1));
                    }
                    ShaderIrOpKind::ControlFlow => {
                        ptx.push_str("    bra.uni L_end;\n");
                    }
                    ShaderIrOpKind::Barrier => {
                        ptx.push_str("    bar.sync 0;\n");
                    }
                    ShaderIrOpKind::Atomic => {
                        let src = dst_reg.saturating_sub(1);
                        ptx.push_str(&format!("    atom.global.add.f32 %f{}, [%rd1], %f{};\n", dst_reg, src));
                    }
                    _ => {}
                }

                registers_used = registers_used.max(reg_counter);
            }
        }

        // Epilogue
        ptx.push_str("\nL_end:\n");
        ptx.push_str("    ret;\n");
        ptx.push_str("}\n");

        let instruction_count = ir.instructions.len() as u32;
        (ptx, registers_used, instruction_count)
    }
}

impl CodegenBackend for NvidiaPtxBackend {
    fn name(&self) -> &'static str {
        "NVIDIA PTX"
    }

    fn generate(&self, ir: &ShaderIr) -> Result<GeneratedCode, String> {
        let (ptx_text, registers_used, instruction_count) = self.generate_ptx(ir);

        if registers_used > DEFAULT_NVIDIA_REGISTERS {
            return Err(format!(
                "Register spill: {} used, {} available",
                registers_used, DEFAULT_NVIDIA_REGISTERS
            ));
        }

        Ok(GeneratedCode {
            code: ptx_text.as_bytes().to_vec(),
            text: Some(ptx_text),
            registers_used,
            instruction_count,
            target_arch: "NVIDIA PTX",
        })
    }

    fn register_count(&self) -> u32 {
        DEFAULT_NVIDIA_REGISTERS
    }

    fn target_description(&self) -> &'static str {
        match self.sm_version {
            52 => "NVIDIA SM 5.2 (Maxwell)",
            60 => "NVIDIA SM 6.0 (Pascal)",
            70 => "NVIDIA SM 7.0 (Volta)",
            75 => "NVIDIA SM 7.5 (Turing)",
            80 => "NVIDIA SM 8.0 (Ampere)",
            86 => "NVIDIA SM 8.6 (Ampere GA10x)",
            90 => "NVIDIA SM 9.0 (Hopper)",
            _ => "NVIDIA PTX (Unknown SM)",
        }
    }
}

// ============================================================================
// Backend Factory
// ============================================================================

/// Create appropriate backend for vendor
///
/// # Arguments
/// * `vendor` - GPU vendor
/// * `version` - Architecture version (Gen for Intel, GFX for AMD, SM for NVIDIA)
pub fn create_backend(vendor: GpuVendor, version: u32) -> Option<Box<dyn CodegenBackend>> {
    match vendor {
        GpuVendor::Intel => Some(Box::new(IntelGenBackend::new(version))),
        GpuVendor::Amd => Some(Box::new(AmdGcnBackend::new(version))),
        GpuVendor::Nvidia => Some(Box::new(NvidiaPtxBackend::new(version))),
        GpuVendor::Unknown => None,
    }
}

/// Get default backend version for vendor
pub fn default_version(vendor: GpuVendor) -> u32 {
    match vendor {
        GpuVendor::Intel => 12,  // Gen12 (Xe)
        GpuVendor::Amd => 10,    // RDNA (gfx10)
        GpuVendor::Nvidia => 75, // SM 7.5 (Turing)
        GpuVendor::Unknown => 0,
    }
}

// ============================================================================
// Tests (35+)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // CodegenState Tests
    // ========================================================================

    #[test]
    fn test_codegen_state_from_u8() {
        assert_eq!(CodegenState::from_u8(0), CodegenState::Idle);
        assert_eq!(CodegenState::from_u8(1), CodegenState::Generating);
        assert_eq!(CodegenState::from_u8(2), CodegenState::Complete);
        assert_eq!(CodegenState::from_u8(3), CodegenState::Error);
        assert_eq!(CodegenState::from_u8(255), CodegenState::Error);
    }

    #[test]
    fn test_codegen_state_values() {
        assert_eq!(CodegenState::Idle as u8, 0);
        assert_eq!(CodegenState::Generating as u8, 1);
        assert_eq!(CodegenState::Complete as u8, 2);
        assert_eq!(CodegenState::Error as u8, 3);
    }

    // ========================================================================
    // GeneratedCode Tests
    // ========================================================================

    #[test]
    fn test_generated_code_new() {
        let code = GeneratedCode::new("test");
        assert!(code.code.is_empty());
        assert!(code.text.is_none());
        assert_eq!(code.registers_used, 0);
        assert_eq!(code.instruction_count, 0);
        assert_eq!(code.target_arch, "test");
    }

    #[test]
    fn test_generated_code_with_capacity() {
        let code = GeneratedCode::with_capacity("test", 1024);
        assert!(code.is_empty());
        assert!(code.code.capacity() >= 1024);
    }

    #[test]
    fn test_generated_code_is_empty() {
        let empty = GeneratedCode::new("test");
        assert!(empty.is_empty());

        let mut with_code = GeneratedCode::new("test");
        with_code.code = vec![0x00];
        assert!(!with_code.is_empty());

        let mut with_text = GeneratedCode::new("test");
        with_text.text = Some("hello".into());
        assert!(!with_text.is_empty());
    }

    #[test]
    fn test_generated_code_size() {
        let mut code = GeneratedCode::new("test");
        assert_eq!(code.size(), 0);

        code.code = vec![0x00, 0x01, 0x02, 0x03];
        assert_eq!(code.size(), 4);

        code.text = Some("hello".into());
        assert_eq!(code.size(), 9); // 4 bytes + 5 chars
    }

    #[test]
    fn test_generated_code_default() {
        let code = GeneratedCode::default();
        assert_eq!(code.target_arch, "unknown");
    }

    // ========================================================================
    // CodegenCapsule Tests
    // ========================================================================

    #[test]
    fn test_codegen_capsule_size() {
        assert_eq!(core::mem::size_of::<CodegenCapsule>(), 256);
        assert_eq!(core::mem::align_of::<CodegenCapsule>(), 256);
    }

    #[test]
    fn test_codegen_capsule_new() {
        let capsule = CodegenCapsule::new();
        let snap = capsule.snapshot();
        assert_eq!(snap.state, CodegenState::Idle);
        assert_eq!(snap.instructions_generated, 0);
        assert_eq!(snap.registers_used, 0);
        assert_eq!(snap.error_count, 0);
    }

    #[test]
    fn test_codegen_capsule_set_state() {
        let capsule = CodegenCapsule::new();

        capsule.set_state(CodegenState::Generating);
        assert_eq!(capsule.snapshot().state, CodegenState::Generating);

        capsule.set_state(CodegenState::Complete);
        assert_eq!(capsule.snapshot().state, CodegenState::Complete);

        capsule.set_state(CodegenState::Error);
        assert_eq!(capsule.snapshot().state, CodegenState::Error);
    }

    #[test]
    fn test_codegen_capsule_increment_generation() {
        let capsule = CodegenCapsule::new();
        let gen0 = capsule.snapshot().generation;

        capsule.increment_generation();
        let gen1 = capsule.snapshot().generation;
        assert_eq!(gen1, gen0 + 1);

        capsule.increment_generation();
        let gen2 = capsule.snapshot().generation;
        assert_eq!(gen2, gen1 + 1);
    }

    #[test]
    fn test_codegen_capsule_add_instructions() {
        let capsule = CodegenCapsule::new();

        capsule.add_instructions(10);
        assert_eq!(capsule.snapshot().instructions_generated, 10);

        capsule.add_instructions(5);
        assert_eq!(capsule.snapshot().instructions_generated, 15);
    }

    #[test]
    fn test_codegen_capsule_set_registers() {
        let capsule = CodegenCapsule::new();

        capsule.set_registers_used(64);
        assert_eq!(capsule.snapshot().registers_used, 64);

        capsule.set_registers_used(128);
        assert_eq!(capsule.snapshot().registers_used, 128);
    }

    #[test]
    fn test_codegen_capsule_increment_error() {
        let capsule = CodegenCapsule::new();

        capsule.increment_error_count();
        assert_eq!(capsule.snapshot().error_count, 1);

        capsule.increment_error_count();
        assert_eq!(capsule.snapshot().error_count, 2);
    }

    #[test]
    fn test_codegen_capsule_optimization_level() {
        let capsule = CodegenCapsule::new();

        // Default is O2
        assert_eq!(capsule.get_optimization_level(), 2);

        capsule.set_optimization_level(0);
        assert_eq!(capsule.get_optimization_level(), 0);

        capsule.set_optimization_level(3);
        assert_eq!(capsule.get_optimization_level(), 3);

        // Clamp to max 3
        capsule.set_optimization_level(10);
        assert_eq!(capsule.get_optimization_level(), 3);
    }

    #[test]
    fn test_codegen_capsule_default() {
        let capsule = CodegenCapsule::default();
        let snap = capsule.snapshot();
        assert_eq!(snap.state, CodegenState::Idle);
    }

    // ========================================================================
    // GenOpcode Tests
    // ========================================================================

    #[test]
    fn test_gen_opcode_name() {
        assert_eq!(GenOpcode::Mov.name(), "mov");
        assert_eq!(GenOpcode::Add.name(), "add");
        assert_eq!(GenOpcode::Mul.name(), "mul");
        assert_eq!(GenOpcode::Mad.name(), "mad");
        assert_eq!(GenOpcode::Cmp.name(), "cmp");
        assert_eq!(GenOpcode::Jmp.name(), "jmp");
        assert_eq!(GenOpcode::Call.name(), "call");
        assert_eq!(GenOpcode::Ret.name(), "ret");
        assert_eq!(GenOpcode::Send.name(), "send");
        assert_eq!(GenOpcode::Nop.name(), "nop");
    }

    #[test]
    fn test_gen_opcode_from_ir_op() {
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Load), Some(GenOpcode::Mov));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Store), Some(GenOpcode::Mov));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Arithmetic), Some(GenOpcode::Add));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Compare), Some(GenOpcode::Cmp));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::ControlFlow), Some(GenOpcode::Jmp));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Texture), Some(GenOpcode::Send));
        assert_eq!(GenOpcode::from_ir_op(ShaderIrOpKind::Debug), None);
    }

    #[test]
    fn test_gen_opcode_values() {
        assert_eq!(GenOpcode::Mov as u8, 0x01);
        assert_eq!(GenOpcode::Add as u8, 0x40);
        assert_eq!(GenOpcode::Mul as u8, 0x41);
        assert_eq!(GenOpcode::Mad as u8, 0x5D);
        assert_eq!(GenOpcode::Nop as u8, 0x7E);
    }

    // ========================================================================
    // IntelGenBackend Tests
    // ========================================================================

    #[test]
    fn test_intel_backend_new() {
        let backend = IntelGenBackend::new(12);
        assert_eq!(backend.gen_version, 12);
    }

    #[test]
    fn test_intel_backend_name() {
        let backend = IntelGenBackend::new(12);
        assert_eq!(backend.name(), "Intel Gen EU");
    }

    #[test]
    fn test_intel_backend_register_count() {
        let backend = IntelGenBackend::new(12);
        assert_eq!(backend.register_count(), DEFAULT_INTEL_REGISTERS);
    }

    #[test]
    fn test_intel_backend_target_description() {
        assert_eq!(IntelGenBackend::new(9).target_description(), "Intel Gen9 (Skylake)");
        assert_eq!(IntelGenBackend::new(11).target_description(), "Intel Gen11 (Ice Lake)");
        assert_eq!(IntelGenBackend::new(12).target_description(), "Intel Gen12 (Xe)");
        assert_eq!(IntelGenBackend::new(99).target_description(), "Intel Gen (Unknown)");
    }

    #[test]
    fn test_intel_backend_generate_empty_ir() {
        let backend = IntelGenBackend::new(12);
        let ir = ShaderIr::new();
        let result = backend.generate(&ir);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.code.is_empty());
        assert_eq!(code.target_arch, "Intel Gen EU");
    }

    // ========================================================================
    // VopOpcode Tests
    // ========================================================================

    #[test]
    fn test_vop_opcode_name() {
        assert_eq!(VopOpcode::V_MOV.name(), "v_mov_b32");
        assert_eq!(VopOpcode::V_ADD_F32.name(), "v_add_f32");
        assert_eq!(VopOpcode::V_MUL_F32.name(), "v_mul_f32");
        assert_eq!(VopOpcode::V_MAD_F32.name(), "v_mad_f32");
        assert_eq!(VopOpcode::V_CMP.name(), "v_cmp");
        assert_eq!(VopOpcode::S_BRANCH.name(), "s_branch");
        assert_eq!(VopOpcode::S_CBRANCH.name(), "s_cbranch");
        assert_eq!(VopOpcode::S_ENDPGM.name(), "s_endpgm");
        assert_eq!(VopOpcode::S_WAITCNT.name(), "s_waitcnt");
        assert_eq!(VopOpcode::S_NOP.name(), "s_nop");
    }

    #[test]
    fn test_vop_opcode_from_ir_op() {
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Load), Some(VopOpcode::V_MOV));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Store), Some(VopOpcode::V_MOV));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Arithmetic), Some(VopOpcode::V_ADD_F32));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Compare), Some(VopOpcode::V_CMP));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::ControlFlow), Some(VopOpcode::S_BRANCH));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Barrier), Some(VopOpcode::S_WAITCNT));
        assert_eq!(VopOpcode::from_ir_op(ShaderIrOpKind::Debug), None);
    }

    #[test]
    fn test_vop_opcode_values() {
        assert_eq!(VopOpcode::V_MOV as u8, 0x01);
        assert_eq!(VopOpcode::V_ADD_F32 as u8, 0x03);
        assert_eq!(VopOpcode::V_MUL_F32 as u8, 0x05);
        assert_eq!(VopOpcode::S_ENDPGM as u8, 0xBF);
    }

    // ========================================================================
    // AmdGcnBackend Tests
    // ========================================================================

    #[test]
    fn test_amd_backend_new() {
        let backend = AmdGcnBackend::new(10);
        assert_eq!(backend.gfx_version, 10);
    }

    #[test]
    fn test_amd_backend_name() {
        let backend = AmdGcnBackend::new(10);
        assert_eq!(backend.name(), "AMD GCN/RDNA");
    }

    #[test]
    fn test_amd_backend_register_count() {
        let backend = AmdGcnBackend::new(10);
        assert_eq!(backend.register_count(), DEFAULT_AMD_VGPR);
    }

    #[test]
    fn test_amd_backend_target_description() {
        assert_eq!(AmdGcnBackend::new(8).target_description(), "AMD GCN 3 (gfx8, Polaris)");
        assert_eq!(AmdGcnBackend::new(9).target_description(), "AMD GCN 5 (gfx9, Vega)");
        assert_eq!(AmdGcnBackend::new(10).target_description(), "AMD RDNA (gfx10, Navi)");
        assert_eq!(AmdGcnBackend::new(11).target_description(), "AMD RDNA 3 (gfx11)");
        assert_eq!(AmdGcnBackend::new(99).target_description(), "AMD GCN/RDNA (Unknown)");
    }

    #[test]
    fn test_amd_backend_generate_empty_ir() {
        let backend = AmdGcnBackend::new(10);
        let ir = ShaderIr::new();
        let result = backend.generate(&ir);
        assert!(result.is_ok());
        let code = result.unwrap();
        // Should have at least S_ENDPGM
        assert!(!code.code.is_empty());
        assert_eq!(code.instruction_count, 1); // Just S_ENDPGM
    }

    // ========================================================================
    // NvidiaPtxBackend Tests
    // ========================================================================

    #[test]
    fn test_nvidia_backend_new() {
        let backend = NvidiaPtxBackend::new(75);
        assert_eq!(backend.sm_version, 75);
    }

    #[test]
    fn test_nvidia_backend_name() {
        let backend = NvidiaPtxBackend::new(75);
        assert_eq!(backend.name(), "NVIDIA PTX");
    }

    #[test]
    fn test_nvidia_backend_register_count() {
        let backend = NvidiaPtxBackend::new(75);
        assert_eq!(backend.register_count(), DEFAULT_NVIDIA_REGISTERS);
    }

    #[test]
    fn test_nvidia_backend_target_description() {
        assert_eq!(NvidiaPtxBackend::new(52).target_description(), "NVIDIA SM 5.2 (Maxwell)");
        assert_eq!(NvidiaPtxBackend::new(60).target_description(), "NVIDIA SM 6.0 (Pascal)");
        assert_eq!(NvidiaPtxBackend::new(70).target_description(), "NVIDIA SM 7.0 (Volta)");
        assert_eq!(NvidiaPtxBackend::new(75).target_description(), "NVIDIA SM 7.5 (Turing)");
        assert_eq!(NvidiaPtxBackend::new(80).target_description(), "NVIDIA SM 8.0 (Ampere)");
        assert_eq!(NvidiaPtxBackend::new(86).target_description(), "NVIDIA SM 8.6 (Ampere GA10x)");
        assert_eq!(NvidiaPtxBackend::new(90).target_description(), "NVIDIA SM 9.0 (Hopper)");
        assert_eq!(NvidiaPtxBackend::new(99).target_description(), "NVIDIA PTX (Unknown SM)");
    }

    #[test]
    fn test_nvidia_backend_generate_empty_ir() {
        let backend = NvidiaPtxBackend::new(75);
        let ir = ShaderIr::new();
        let result = backend.generate(&ir);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.text.is_some());
        let ptx = code.text.as_ref().unwrap();
        assert!(ptx.contains(".version 7.8"));
        assert!(ptx.contains(".target sm_75"));
        assert!(ptx.contains(".entry shader_main"));
        assert!(ptx.contains("ret;"));
    }

    #[test]
    fn test_nvidia_backend_ptx_header() {
        let backend = NvidiaPtxBackend::new(80);
        let ir = ShaderIr::new();
        let (ptx, _, _) = backend.generate_ptx(&ir);
        assert!(ptx.starts_with(".version 7.8\n"));
        assert!(ptx.contains(".target sm_80"));
        assert!(ptx.contains(".address_size 64"));
    }

    #[test]
    fn test_nvidia_backend_ptx_registers() {
        let backend = NvidiaPtxBackend::new(75);
        let ir = ShaderIr::new();
        let (ptx, _, _) = backend.generate_ptx(&ir);
        assert!(ptx.contains(".reg .f32 %f<64>;"));
        assert!(ptx.contains(".reg .u32 %r<64>;"));
        assert!(ptx.contains(".reg .u64 %rd<16>;"));
        assert!(ptx.contains(".reg .pred %p<8>;"));
    }

    // ========================================================================
    // Backend Factory Tests
    // ========================================================================

    #[test]
    fn test_create_backend_intel() {
        let backend = create_backend(GpuVendor::Intel, 12);
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name(), "Intel Gen EU");
    }

    #[test]
    fn test_create_backend_amd() {
        let backend = create_backend(GpuVendor::Amd, 10);
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name(), "AMD GCN/RDNA");
    }

    #[test]
    fn test_create_backend_nvidia() {
        let backend = create_backend(GpuVendor::Nvidia, 75);
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name(), "NVIDIA PTX");
    }

    #[test]
    fn test_create_backend_unknown() {
        let backend = create_backend(GpuVendor::Unknown, 0);
        assert!(backend.is_none());
    }

    #[test]
    fn test_default_version() {
        assert_eq!(default_version(GpuVendor::Intel), 12);
        assert_eq!(default_version(GpuVendor::Amd), 10);
        assert_eq!(default_version(GpuVendor::Nvidia), 75);
        assert_eq!(default_version(GpuVendor::Unknown), 0);
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(MAX_CODE_SIZE, 16 * 1024 * 1024);
        assert_eq!(DEFAULT_INTEL_REGISTERS, 128);
        assert_eq!(DEFAULT_AMD_VGPR, 256);
        assert_eq!(DEFAULT_AMD_SGPR, 104);
        assert_eq!(DEFAULT_NVIDIA_REGISTERS, 255);
    }
}
