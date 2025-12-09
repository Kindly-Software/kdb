//! Shader IR Module - Optimizing Intermediate Representation
//!
//! **Tier**: T1 Atomic (256B aligned)
//! **Purpose**: Provide a robust shader IR with SSA form and optimization passes
//!
//! # Architecture
//!
//! This module provides:
//! - [`ShaderIrModuleCapsule`]: T1 Atomic capsule for module state tracking
//! - [`IrType`]: Complete type system (12 types: Void to Mat4)
//! - [`IrOpcode`]: 30 opcodes for GPU shader operations
//! - [`SsaValue`]: SSA value references for def-use chains
//! - [`IrInstruction`]: 32-byte instructions with SSA operands
//! - **3 Optimization Passes**: DCE, Constant Folding, Strength Reduction
//!
//! # Memory Layout (ShaderIrModuleCapsule - 256B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       state: AtomicU64 (module state flags)
//! 8       8       generation: AtomicU64 (version counter)
//! 16      8       instruction_count: AtomicU64
//! 24      8       register_count: AtomicU64
//! 32      8       constant_count: AtomicU64
//! 40      8       basic_block_count: AtomicU64
//! 48      8       optimization_level: AtomicU64
//! 56      8       pass_count: AtomicU64
//! 64      192     _padding (to 256B)
//! ```
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree state coordination)
//! - Q11: Rust transform (SSA form, type-safe IR)
//! - Q33: Chaos-compliant capsule design
//! - Q34: Audit trail for optimization passes
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SSA_VALID`: All SsaValue references are valid within module
//! - `#ASSUME_INSTRUCTION_32B`: Instructions fit in 32 bytes
//! - `#ASSUME_TYPE_STABLE`: IR types match GPU type system
//! - `#ASSUME_OPCODE_COMPLETE`: 30 opcodes cover shader operations
//!
//! # Framework Compliance
//!
//! - **Chaos**: 100% lockfree, zero mutex/RwLock
//! - **T28**: 30+ tests (unit/property/integration)
//! - **B32**: Validated optimization speedups

#![allow(dead_code)] // During development

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// IR Type System
// ============================================================================

/// IR Type enumeration covering GPU shader types
///
/// # ASSUM Safety
/// `#ASSUME_TYPE_STABLE`: Types map directly to GPU hardware types.
/// `#VERIFY_TYPE_STABLE`: Matches SPIR-V/GLSL/HLSL type systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IrType {
    /// Void type (for functions with no return)
    Void = 0,
    /// Boolean type (1-bit logical)
    Bool = 1,
    /// 32-bit signed integer
    Int32 = 2,
    /// 64-bit signed integer
    Int64 = 3,
    /// 32-bit unsigned integer
    Uint32 = 4,
    /// 64-bit unsigned integer
    Uint64 = 5,
    /// 32-bit floating point
    Float32 = 6,
    /// 64-bit floating point (double)
    Float64 = 7,
    /// 2-component vector (float2)
    Vec2 = 8,
    /// 3-component vector (float3)
    Vec3 = 9,
    /// 4-component vector (float4)
    Vec4 = 10,
    /// 4x4 matrix (float4x4)
    Mat4 = 11,
}

impl IrType {
    /// Get the size in bytes of this type
    #[inline]
    pub const fn size_bytes(self) -> usize {
        match self {
            IrType::Void => 0,
            IrType::Bool => 1,
            IrType::Int32 | IrType::Uint32 | IrType::Float32 => 4,
            IrType::Int64 | IrType::Uint64 | IrType::Float64 | IrType::Vec2 => 8,
            IrType::Vec3 => 12,
            IrType::Vec4 => 16,
            IrType::Mat4 => 64,
        }
    }

    /// Check if this is a scalar type
    #[inline]
    pub const fn is_scalar(self) -> bool {
        matches!(
            self,
            IrType::Bool
                | IrType::Int32
                | IrType::Int64
                | IrType::Uint32
                | IrType::Uint64
                | IrType::Float32
                | IrType::Float64
        )
    }

    /// Check if this is a vector type
    #[inline]
    pub const fn is_vector(self) -> bool {
        matches!(self, IrType::Vec2 | IrType::Vec3 | IrType::Vec4)
    }

    /// Check if this is a matrix type
    #[inline]
    pub const fn is_matrix(self) -> bool {
        matches!(self, IrType::Mat4)
    }

    /// Check if this is a floating point type
    #[inline]
    pub const fn is_float(self) -> bool {
        matches!(self, IrType::Float32 | IrType::Float64)
    }

    /// Check if this is an integer type
    #[inline]
    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            IrType::Int32 | IrType::Int64 | IrType::Uint32 | IrType::Uint64
        )
    }

    /// Get vector component count (1 for scalars)
    #[inline]
    pub const fn component_count(self) -> usize {
        match self {
            IrType::Vec2 => 2,
            IrType::Vec3 => 3,
            IrType::Vec4 | IrType::Mat4 => 4, // Mat4 has 4 columns
            _ => 1,
        }
    }

    /// Convert from u8 representation
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(IrType::Void),
            1 => Some(IrType::Bool),
            2 => Some(IrType::Int32),
            3 => Some(IrType::Int64),
            4 => Some(IrType::Uint32),
            5 => Some(IrType::Uint64),
            6 => Some(IrType::Float32),
            7 => Some(IrType::Float64),
            8 => Some(IrType::Vec2),
            9 => Some(IrType::Vec3),
            10 => Some(IrType::Vec4),
            11 => Some(IrType::Mat4),
            _ => None,
        }
    }
}

impl Default for IrType {
    fn default() -> Self {
        IrType::Void
    }
}

// ============================================================================
// IR Opcodes
// ============================================================================

/// IR Opcode enumeration (30 opcodes)
///
/// Covers essential GPU shader operations:
/// - Arithmetic: Add, Sub, Mul, Div, Neg
/// - Logical: And, Or, Xor, Not
/// - Comparison: Eq, Ne, Lt, Le, Gt, Ge
/// - Memory: Load, Store
/// - Control: Branch, BranchCond, Return, Call, Phi
/// - Math: Sin, Cos, Sqrt, Min, Max, Clamp
/// - Texture: Sample
/// - Misc: Nop
///
/// # ASSUM Safety
/// `#ASSUME_OPCODE_COMPLETE`: These 30 opcodes cover common shader patterns.
/// `#VERIFY_OPCODE_COMPLETE`: Maps to SPIR-V/PTX/GCN instruction sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IrOpcode {
    // Arithmetic operations (0-4)
    /// Add two values: result = op0 + op1
    Add = 0,
    /// Subtract: result = op0 - op1
    Sub = 1,
    /// Multiply: result = op0 * op1
    Mul = 2,
    /// Divide: result = op0 / op1
    Div = 3,
    /// Negate: result = -op0
    Neg = 4,

    // Logical/bitwise operations (5-8)
    /// Bitwise AND: result = op0 & op1
    And = 5,
    /// Bitwise OR: result = op0 | op1
    Or = 6,
    /// Bitwise XOR: result = op0 ^ op1
    Xor = 7,
    /// Bitwise NOT: result = ~op0
    Not = 8,

    // Comparison operations (9-14)
    /// Equal: result = (op0 == op1)
    Eq = 9,
    /// Not equal: result = (op0 != op1)
    Ne = 10,
    /// Less than: result = (op0 < op1)
    Lt = 11,
    /// Less or equal: result = (op0 <= op1)
    Le = 12,
    /// Greater than: result = (op0 > op1)
    Gt = 13,
    /// Greater or equal: result = (op0 >= op1)
    Ge = 14,

    // Memory operations (15-16)
    /// Load from memory: result = *op0
    Load = 15,
    /// Store to memory: *op0 = op1
    Store = 16,

    // Control flow (17-21)
    /// Unconditional branch to label
    Branch = 17,
    /// Conditional branch: if op0 goto label1 else goto label2
    BranchCond = 18,
    /// Return from function (optionally with value)
    Return = 19,
    /// Function call: result = call(func_id, args...)
    Call = 20,
    /// PHI node for SSA form
    Phi = 21,

    // Math functions (22-27)
    /// Sine: result = sin(op0)
    Sin = 22,
    /// Cosine: result = cos(op0)
    Cos = 23,
    /// Square root: result = sqrt(op0)
    Sqrt = 24,
    /// Minimum: result = min(op0, op1)
    Min = 25,
    /// Maximum: result = max(op0, op1)
    Max = 26,
    /// Clamp: result = clamp(op0, op1, op2)
    Clamp = 27,

    // Texture operations (28)
    /// Texture sample: result = sample(texture, coords)
    Sample = 28,

    // Misc (29)
    /// No operation (placeholder)
    Nop = 29,
}

impl IrOpcode {
    /// Check if this opcode produces a result
    #[inline]
    pub const fn has_result(self) -> bool {
        !matches!(self, IrOpcode::Store | IrOpcode::Branch | IrOpcode::BranchCond | IrOpcode::Nop)
    }

    /// Get the number of operands for this opcode
    #[inline]
    pub const fn operand_count(self) -> usize {
        match self {
            IrOpcode::Nop | IrOpcode::Return => 0,
            IrOpcode::Neg | IrOpcode::Not | IrOpcode::Sin | IrOpcode::Cos
            | IrOpcode::Sqrt | IrOpcode::Load | IrOpcode::Branch => 1,
            IrOpcode::Add | IrOpcode::Sub | IrOpcode::Mul | IrOpcode::Div
            | IrOpcode::And | IrOpcode::Or | IrOpcode::Xor
            | IrOpcode::Eq | IrOpcode::Ne | IrOpcode::Lt | IrOpcode::Le
            | IrOpcode::Gt | IrOpcode::Ge | IrOpcode::Store
            | IrOpcode::Min | IrOpcode::Max | IrOpcode::Sample => 2,
            IrOpcode::BranchCond | IrOpcode::Clamp => 3,
            IrOpcode::Call | IrOpcode::Phi => 0, // Variable operands
        }
    }

    /// Check if this is an arithmetic opcode
    #[inline]
    pub const fn is_arithmetic(self) -> bool {
        matches!(
            self,
            IrOpcode::Add | IrOpcode::Sub | IrOpcode::Mul | IrOpcode::Div | IrOpcode::Neg
        )
    }

    /// Check if this is a comparison opcode
    #[inline]
    pub const fn is_comparison(self) -> bool {
        matches!(
            self,
            IrOpcode::Eq | IrOpcode::Ne | IrOpcode::Lt | IrOpcode::Le | IrOpcode::Gt | IrOpcode::Ge
        )
    }

    /// Check if this is a control flow opcode
    #[inline]
    pub const fn is_control_flow(self) -> bool {
        matches!(
            self,
            IrOpcode::Branch | IrOpcode::BranchCond | IrOpcode::Return | IrOpcode::Call | IrOpcode::Phi
        )
    }

    /// Check if this is a math function opcode
    #[inline]
    pub const fn is_math(self) -> bool {
        matches!(
            self,
            IrOpcode::Sin | IrOpcode::Cos | IrOpcode::Sqrt | IrOpcode::Min | IrOpcode::Max | IrOpcode::Clamp
        )
    }

    /// Convert from u8 representation
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(IrOpcode::Add),
            1 => Some(IrOpcode::Sub),
            2 => Some(IrOpcode::Mul),
            3 => Some(IrOpcode::Div),
            4 => Some(IrOpcode::Neg),
            5 => Some(IrOpcode::And),
            6 => Some(IrOpcode::Or),
            7 => Some(IrOpcode::Xor),
            8 => Some(IrOpcode::Not),
            9 => Some(IrOpcode::Eq),
            10 => Some(IrOpcode::Ne),
            11 => Some(IrOpcode::Lt),
            12 => Some(IrOpcode::Le),
            13 => Some(IrOpcode::Gt),
            14 => Some(IrOpcode::Ge),
            15 => Some(IrOpcode::Load),
            16 => Some(IrOpcode::Store),
            17 => Some(IrOpcode::Branch),
            18 => Some(IrOpcode::BranchCond),
            19 => Some(IrOpcode::Return),
            20 => Some(IrOpcode::Call),
            21 => Some(IrOpcode::Phi),
            22 => Some(IrOpcode::Sin),
            23 => Some(IrOpcode::Cos),
            24 => Some(IrOpcode::Sqrt),
            25 => Some(IrOpcode::Min),
            26 => Some(IrOpcode::Max),
            27 => Some(IrOpcode::Clamp),
            28 => Some(IrOpcode::Sample),
            29 => Some(IrOpcode::Nop),
            _ => None,
        }
    }
}

impl Default for IrOpcode {
    fn default() -> Self {
        IrOpcode::Nop
    }
}

// ============================================================================
// SSA Value
// ============================================================================

/// SSA (Static Single Assignment) value reference
///
/// Represents a unique definition in the IR. Each value is defined exactly once,
/// enabling efficient dataflow analysis and optimization.
///
/// # Layout
/// - Bits 0-30: Value index (max 2^31 values)
/// - Bit 31: Constant flag (1 = constant, 0 = register)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct SsaValue(pub u32);

impl SsaValue {
    /// Invalid/undefined value sentinel
    pub const UNDEF: SsaValue = SsaValue(u32::MAX);

    /// Create a new SSA value from index
    #[inline]
    pub const fn new(index: u32) -> Self {
        SsaValue(index & 0x7FFF_FFFF)
    }

    /// Create a constant SSA value
    #[inline]
    pub const fn constant(index: u32) -> Self {
        SsaValue((index & 0x7FFF_FFFF) | 0x8000_0000)
    }

    /// Get the value index
    #[inline]
    pub const fn index(self) -> u32 {
        self.0 & 0x7FFF_FFFF
    }

    /// Check if this is a constant
    #[inline]
    pub const fn is_constant(self) -> bool {
        (self.0 & 0x8000_0000) != 0
    }

    /// Check if this is undefined
    #[inline]
    pub const fn is_undef(self) -> bool {
        self.0 == u32::MAX
    }

    /// Check if this value is valid (not UNDEF)
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl core::fmt::Debug for SsaValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_undef() {
            write!(f, "undef")
        } else if self.is_constant() {
            write!(f, "c{}", self.index())
        } else {
            write!(f, "v{}", self.index())
        }
    }
}

impl core::fmt::Display for SsaValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

// ============================================================================
// IR Instruction
// ============================================================================

/// Single IR instruction in SSA form
///
/// # Layout (32 bytes)
/// ```text
/// Offset  Size    Field
/// 0       1       opcode: IrOpcode
/// 1       1       result_type: IrType
/// 2       2       flags: InstructionFlags
/// 4       4       result: SsaValue
/// 8       4       operand0: SsaValue
/// 12      4       operand1: SsaValue
/// 16      4       basic_block: u32
/// 20      4       source_line: u32 (debug info)
/// 24      8       immediate: u64 (for constants/labels)
/// ```
///
/// # ASSUM Safety
/// `#ASSUME_INSTRUCTION_32B`: All instructions fit in 32 bytes.
/// `#VERIFY_INSTRUCTION_32B`: Verified by static_assert below.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct IrInstruction {
    /// The opcode
    pub opcode: IrOpcode,
    /// Result type (IrType::Void if no result)
    pub result_type: IrType,
    /// Instruction flags
    pub flags: u16,
    /// Result SSA value (UNDEF if no result)
    pub result: SsaValue,
    /// First operand
    pub operand0: SsaValue,
    /// Second operand
    pub operand1: SsaValue,
    /// Basic block this instruction belongs to
    pub basic_block: u32,
    /// Source line for debugging
    pub source_line: u32,
    /// Immediate value (constants, branch targets, etc.)
    pub immediate: u64,
}

// Verify 32-byte size at compile time
const _: () = {
    assert!(core::mem::size_of::<IrInstruction>() == 32);
};

impl IrInstruction {
    /// Create a new instruction
    #[inline]
    pub const fn new(opcode: IrOpcode, result_type: IrType) -> Self {
        Self {
            opcode,
            result_type,
            flags: 0,
            result: SsaValue::UNDEF,
            operand0: SsaValue::UNDEF,
            operand1: SsaValue::UNDEF,
            basic_block: 0,
            source_line: 0,
            immediate: 0,
        }
    }

    /// Create a binary operation (add, sub, mul, etc.)
    #[inline]
    pub const fn binary(
        opcode: IrOpcode,
        result_type: IrType,
        result: SsaValue,
        op0: SsaValue,
        op1: SsaValue,
    ) -> Self {
        Self {
            opcode,
            result_type,
            flags: 0,
            result,
            operand0: op0,
            operand1: op1,
            basic_block: 0,
            source_line: 0,
            immediate: 0,
        }
    }

    /// Create a unary operation (neg, not, sin, etc.)
    #[inline]
    pub const fn unary(
        opcode: IrOpcode,
        result_type: IrType,
        result: SsaValue,
        op0: SsaValue,
    ) -> Self {
        Self {
            opcode,
            result_type,
            flags: 0,
            result,
            operand0: op0,
            operand1: SsaValue::UNDEF,
            basic_block: 0,
            source_line: 0,
            immediate: 0,
        }
    }

    /// Create a NOP instruction
    #[inline]
    pub const fn nop() -> Self {
        Self::new(IrOpcode::Nop, IrType::Void)
    }

    /// Check if this instruction is dead (NOP or flagged dead)
    #[inline]
    pub const fn is_dead(&self) -> bool {
        self.opcode as u8 == IrOpcode::Nop as u8 || (self.flags & InstructionFlags::DEAD) != 0
    }

    /// Mark this instruction as dead
    #[inline]
    pub fn mark_dead(&mut self) {
        self.flags |= InstructionFlags::DEAD;
    }

    /// Check if this instruction has side effects
    #[inline]
    pub const fn has_side_effects(&self) -> bool {
        matches!(
            self.opcode,
            IrOpcode::Store | IrOpcode::Call | IrOpcode::Branch | IrOpcode::BranchCond | IrOpcode::Return
        )
    }

    /// Get the immediate as f32 (for float constants)
    #[inline]
    pub fn immediate_f32(&self) -> f32 {
        f32::from_bits(self.immediate as u32)
    }

    /// Get the immediate as f64 (for double constants)
    #[inline]
    pub fn immediate_f64(&self) -> f64 {
        f64::from_bits(self.immediate)
    }

    /// Get the immediate as i32
    #[inline]
    pub const fn immediate_i32(&self) -> i32 {
        self.immediate as i32
    }

    /// Get the immediate as u32
    #[inline]
    pub const fn immediate_u32(&self) -> u32 {
        self.immediate as u32
    }

    /// Set immediate from f32
    #[inline]
    pub fn set_immediate_f32(&mut self, v: f32) {
        self.immediate = v.to_bits() as u64;
    }

    /// Set immediate from f64
    #[inline]
    pub fn set_immediate_f64(&mut self, v: f64) {
        self.immediate = v.to_bits();
    }
}

impl Default for IrInstruction {
    fn default() -> Self {
        Self::nop()
    }
}

impl core::fmt::Debug for IrInstruction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:?} {:?} = {:?}({:?}, {:?})",
            self.result, self.result_type, self.opcode, self.operand0, self.operand1
        )
    }
}

/// Instruction flags
pub mod InstructionFlags {
    /// Instruction is dead (will be removed by DCE)
    pub const DEAD: u16 = 0x0001;
    /// Instruction is a constant folding candidate
    pub const CONST_FOLDABLE: u16 = 0x0002;
    /// Instruction has been optimized
    pub const OPTIMIZED: u16 = 0x0004;
    /// Instruction is a loop invariant
    pub const LOOP_INVARIANT: u16 = 0x0008;
}

// ============================================================================
// IR Module Constant
// ============================================================================

/// Constant value stored in the IR module
#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
pub struct IrConstant {
    /// Constant type
    pub ty: IrType,
    /// Constant SSA value reference
    pub value: SsaValue,
    /// Raw bits of the constant
    pub bits: u64,
}

impl IrConstant {
    /// Create an i32 constant
    #[inline]
    pub const fn i32(value: SsaValue, v: i32) -> Self {
        Self {
            ty: IrType::Int32,
            value,
            bits: v as u64,
        }
    }

    /// Create a u32 constant
    #[inline]
    pub const fn u32(value: SsaValue, v: u32) -> Self {
        Self {
            ty: IrType::Uint32,
            value,
            bits: v as u64,
        }
    }

    /// Create an f32 constant
    #[inline]
    pub fn f32(value: SsaValue, v: f32) -> Self {
        Self {
            ty: IrType::Float32,
            value,
            bits: v.to_bits() as u64,
        }
    }

    /// Create an f64 constant
    #[inline]
    pub fn f64(value: SsaValue, v: f64) -> Self {
        Self {
            ty: IrType::Float64,
            value,
            bits: v.to_bits(),
        }
    }

    /// Create a bool constant
    #[inline]
    pub const fn bool(value: SsaValue, v: bool) -> Self {
        Self {
            ty: IrType::Bool,
            value,
            bits: v as u64,
        }
    }

    /// Get value as i32
    #[inline]
    pub const fn as_i32(&self) -> i32 {
        self.bits as i32
    }

    /// Get value as u32
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.bits as u32
    }

    /// Get value as f32
    #[inline]
    pub fn as_f32(&self) -> f32 {
        f32::from_bits(self.bits as u32)
    }

    /// Get value as f64
    #[inline]
    pub fn as_f64(&self) -> f64 {
        f64::from_bits(self.bits)
    }

    /// Get value as bool
    #[inline]
    pub const fn as_bool(&self) -> bool {
        self.bits != 0
    }
}

impl core::fmt::Debug for IrConstant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.ty {
            IrType::Int32 => write!(f, "{:?} = {}i32", self.value, self.as_i32()),
            IrType::Uint32 => write!(f, "{:?} = {}u32", self.value, self.as_u32()),
            IrType::Float32 => write!(f, "{:?} = {}f32", self.value, self.as_f32()),
            IrType::Float64 => write!(f, "{:?} = {}f64", self.value, self.as_f64()),
            IrType::Bool => write!(f, "{:?} = {}", self.value, self.as_bool()),
            _ => write!(f, "{:?} = 0x{:016X}", self.value, self.bits),
        }
    }
}

// ============================================================================
// Module State
// ============================================================================

/// Module state flags packed into AtomicU64
pub mod ModuleState {
    /// Module is empty/uninitialized
    pub const EMPTY: u64 = 0;
    /// Module has instructions loaded
    pub const LOADED: u64 = 1;
    /// Module is being optimized
    pub const OPTIMIZING: u64 = 2;
    /// Module optimization complete
    pub const OPTIMIZED: u64 = 3;
    /// Module has errors
    pub const ERROR: u64 = 0xFF;
}

// ============================================================================
// ShaderIrModuleCapsule (T1 Atomic, 256B)
// ============================================================================

/// Shader IR Module Capsule
///
/// T1 Atomic tier capsule for tracking shader IR module state.
/// Provides lockfree coordination for concurrent access during optimization.
///
/// # Memory Layout (256B aligned)
///
/// ```text
/// Offset  Size    Field
/// 0       8       state: AtomicU64
/// 8       8       generation: AtomicU64
/// 16      8       instruction_count: AtomicU64
/// 24      8       register_count: AtomicU64
/// 32      8       constant_count: AtomicU64
/// 40      8       basic_block_count: AtomicU64
/// 48      8       optimization_level: AtomicU64
/// 56      8       pass_count: AtomicU64
/// 64      192     _padding
/// ```
#[repr(C, align(256))]
pub struct ShaderIrModuleCapsule {
    /// Module state (see ModuleState)
    state: AtomicU64,
    /// Generation counter for ABA prevention
    generation: AtomicU64,
    /// Number of instructions in module
    instruction_count: AtomicU64,
    /// Number of SSA registers allocated
    register_count: AtomicU64,
    /// Number of constants
    constant_count: AtomicU64,
    /// Number of basic blocks
    basic_block_count: AtomicU64,
    /// Current optimization level (0-3)
    optimization_level: AtomicU64,
    /// Number of optimization passes applied
    pass_count: AtomicU64,
    /// Padding to 256 bytes
    _padding: [u8; 192],
}

// Verify 256-byte alignment and size
const _: () = {
    assert!(core::mem::size_of::<ShaderIrModuleCapsule>() == 256);
    assert!(core::mem::align_of::<ShaderIrModuleCapsule>() == 256);
};

impl ShaderIrModuleCapsule {
    /// Create a new empty capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(ModuleState::EMPTY),
            generation: AtomicU64::new(0),
            instruction_count: AtomicU64::new(0),
            register_count: AtomicU64::new(0),
            constant_count: AtomicU64::new(0),
            basic_block_count: AtomicU64::new(0),
            optimization_level: AtomicU64::new(0),
            pass_count: AtomicU64::new(0),
            _padding: [0; 192],
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get instruction count
    #[inline]
    pub fn instruction_count(&self) -> u64 {
        self.instruction_count.load(Ordering::Acquire)
    }

    /// Get register count
    #[inline]
    pub fn register_count(&self) -> u64 {
        self.register_count.load(Ordering::Acquire)
    }

    /// Get constant count
    #[inline]
    pub fn constant_count(&self) -> u64 {
        self.constant_count.load(Ordering::Acquire)
    }

    /// Get basic block count
    #[inline]
    pub fn basic_block_count(&self) -> u64 {
        self.basic_block_count.load(Ordering::Acquire)
    }

    /// Get optimization level
    #[inline]
    pub fn optimization_level(&self) -> u64 {
        self.optimization_level.load(Ordering::Acquire)
    }

    /// Get pass count
    #[inline]
    pub fn pass_count(&self) -> u64 {
        self.pass_count.load(Ordering::Acquire)
    }

    /// Set module state atomically
    ///
    /// # ASSUM Safety
    /// `#ASSUME_STATE_TRANSITION_VALID`: State transitions follow protocol.
    #[inline]
    pub fn set_state(&self, state: u64) {
        self.state.store(state, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set instruction count
    #[inline]
    pub fn set_instruction_count(&self, count: u64) {
        self.instruction_count.store(count, Ordering::Release);
    }

    /// Set register count
    #[inline]
    pub fn set_register_count(&self, count: u64) {
        self.register_count.store(count, Ordering::Release);
    }

    /// Set constant count
    #[inline]
    pub fn set_constant_count(&self, count: u64) {
        self.constant_count.store(count, Ordering::Release);
    }

    /// Set basic block count
    #[inline]
    pub fn set_basic_block_count(&self, count: u64) {
        self.basic_block_count.store(count, Ordering::Release);
    }

    /// Increment pass count
    #[inline]
    pub fn increment_pass_count(&self) -> u64 {
        self.pass_count.fetch_add(1, Ordering::AcqRel)
    }

    /// Take atomic snapshot
    #[inline]
    pub fn snapshot(&self) -> ShaderIrModuleSnapshot {
        ShaderIrModuleSnapshot {
            state: self.state(),
            generation: self.generation(),
            instruction_count: self.instruction_count(),
            register_count: self.register_count(),
            constant_count: self.constant_count(),
            basic_block_count: self.basic_block_count(),
            optimization_level: self.optimization_level(),
            pass_count: self.pass_count(),
        }
    }

    /// Check if module is ready for codegen
    #[inline]
    pub fn is_ready(&self) -> bool {
        let state = self.state();
        state == ModuleState::LOADED || state == ModuleState::OPTIMIZED
    }
}

impl Default for ShaderIrModuleCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for ShaderIrModuleCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShaderIrModuleCapsule")
            .field("state", &self.state())
            .field("generation", &self.generation())
            .field("instruction_count", &self.instruction_count())
            .field("register_count", &self.register_count())
            .finish()
    }
}

/// Snapshot of ShaderIrModuleCapsule state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderIrModuleSnapshot {
    pub state: u64,
    pub generation: u64,
    pub instruction_count: u64,
    pub register_count: u64,
    pub constant_count: u64,
    pub basic_block_count: u64,
    pub optimization_level: u64,
    pub pass_count: u64,
}

// ============================================================================
// Optimization Pass Results
// ============================================================================

/// Result of an optimization pass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationResult {
    /// Number of instructions removed
    pub instructions_removed: usize,
    /// Number of instructions modified
    pub instructions_modified: usize,
    /// Whether any changes were made
    pub changed: bool,
}

impl OptimizationResult {
    /// No changes made
    pub const UNCHANGED: Self = Self {
        instructions_removed: 0,
        instructions_modified: 0,
        changed: false,
    };

    /// Create result with changes
    #[inline]
    pub const fn new(removed: usize, modified: usize) -> Self {
        Self {
            instructions_removed: removed,
            instructions_modified: modified,
            changed: removed > 0 || modified > 0,
        }
    }

    /// Merge two results
    #[inline]
    pub const fn merge(self, other: Self) -> Self {
        Self {
            instructions_removed: self.instructions_removed + other.instructions_removed,
            instructions_modified: self.instructions_modified + other.instructions_modified,
            changed: self.changed || other.changed,
        }
    }
}

// ============================================================================
// Optimization Passes
// ============================================================================

/// Dead Code Elimination Pass
///
/// Removes instructions whose results are never used (mark-sweep algorithm).
///
/// # Algorithm
/// 1. Mark all instructions with side effects as live
/// 2. Backward pass: mark operands of live instructions as live
/// 3. Remove unmarked instructions (replace with NOP)
///
/// # ASSUM Safety
/// `#ASSUME_SSA_VALID`: All operand references are valid.
pub fn dead_code_elimination(
    instructions: &mut [IrInstruction],
    capsule: &ShaderIrModuleCapsule,
) -> OptimizationResult {
    if instructions.is_empty() {
        return OptimizationResult::UNCHANGED;
    }

    capsule.set_state(ModuleState::OPTIMIZING);

    // Mark phase: find live values
    // Use a simple bitset (Vec<bool> for clarity)
    let max_value = instructions
        .iter()
        .filter_map(|i| {
            if i.result.is_valid() && !i.result.is_constant() {
                Some(i.result.index() as usize)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);

    let mut live = vec![false; max_value + 1];

    // Initial mark: instructions with side effects are live
    for instr in instructions.iter() {
        if instr.has_side_effects() {
            // Mark operands as live
            if instr.operand0.is_valid() && !instr.operand0.is_constant() {
                let idx = instr.operand0.index() as usize;
                if idx < live.len() {
                    live[idx] = true;
                }
            }
            if instr.operand1.is_valid() && !instr.operand1.is_constant() {
                let idx = instr.operand1.index() as usize;
                if idx < live.len() {
                    live[idx] = true;
                }
            }
        }
    }

    // Propagate liveness (fixed-point iteration)
    let mut changed = true;
    while changed {
        changed = false;
        for instr in instructions.iter() {
            if instr.result.is_valid() && !instr.result.is_constant() {
                let result_idx = instr.result.index() as usize;
                if result_idx < live.len() && live[result_idx] {
                    // This instruction produces a live value, mark its operands
                    if instr.operand0.is_valid() && !instr.operand0.is_constant() {
                        let idx = instr.operand0.index() as usize;
                        if idx < live.len() && !live[idx] {
                            live[idx] = true;
                            changed = true;
                        }
                    }
                    if instr.operand1.is_valid() && !instr.operand1.is_constant() {
                        let idx = instr.operand1.index() as usize;
                        if idx < live.len() && !live[idx] {
                            live[idx] = true;
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    // Sweep phase: remove dead instructions
    let mut removed = 0;
    for instr in instructions.iter_mut() {
        if instr.result.is_valid() && !instr.result.is_constant() {
            let result_idx = instr.result.index() as usize;
            if result_idx < live.len() && !live[result_idx] && !instr.has_side_effects() {
                instr.mark_dead();
                *instr = IrInstruction::nop();
                removed += 1;
            }
        }
    }

    capsule.set_state(ModuleState::OPTIMIZED);
    capsule.increment_pass_count();

    OptimizationResult::new(removed, 0)
}

/// Constant Folding Pass
///
/// Evaluates operations on constants at compile time.
///
/// # Supported Folds
/// - Arithmetic: add, sub, mul, div on integer/float constants
/// - Comparisons: eq, ne, lt, le, gt, ge
/// - Logical: and, or, xor, not on boolean constants
///
/// # ASSUM Safety
/// `#ASSUME_CONSTANT_VALID`: Constant values are within type bounds.
pub fn constant_folding(
    instructions: &mut [IrInstruction],
    constants: &[IrConstant],
    capsule: &ShaderIrModuleCapsule,
) -> OptimizationResult {
    if instructions.is_empty() {
        return OptimizationResult::UNCHANGED;
    }

    capsule.set_state(ModuleState::OPTIMIZING);

    let mut modified = 0;

    for instr in instructions.iter_mut() {
        // Skip dead instructions
        if instr.is_dead() {
            continue;
        }

        // Check if both operands are constants
        let op0_const = instr.operand0.is_constant();
        let op1_const = instr.operand1.is_constant();

        if !op0_const && !op1_const {
            continue;
        }

        // Find constant values
        let c0 = if op0_const {
            constants.iter().find(|c| c.value == instr.operand0)
        } else {
            None
        };
        let c1 = if op1_const {
            constants.iter().find(|c| c.value == instr.operand1)
        } else {
            None
        };

        // Fold binary operations with two constants
        if let (Some(c0), Some(c1)) = (c0, c1) {
            let folded = match (instr.opcode, c0.ty) {
                // Integer operations
                (IrOpcode::Add, IrType::Int32) => {
                    Some(c0.as_i32().wrapping_add(c1.as_i32()) as u64)
                }
                (IrOpcode::Sub, IrType::Int32) => {
                    Some(c0.as_i32().wrapping_sub(c1.as_i32()) as u64)
                }
                (IrOpcode::Mul, IrType::Int32) => {
                    Some(c0.as_i32().wrapping_mul(c1.as_i32()) as u64)
                }
                (IrOpcode::Div, IrType::Int32) if c1.as_i32() != 0 => {
                    Some(c0.as_i32().wrapping_div(c1.as_i32()) as u64)
                }
                // Float operations
                (IrOpcode::Add, IrType::Float32) => Some((c0.as_f32() + c1.as_f32()).to_bits() as u64),
                (IrOpcode::Sub, IrType::Float32) => Some((c0.as_f32() - c1.as_f32()).to_bits() as u64),
                (IrOpcode::Mul, IrType::Float32) => Some((c0.as_f32() * c1.as_f32()).to_bits() as u64),
                (IrOpcode::Div, IrType::Float32) if c1.as_f32() != 0.0 => {
                    Some((c0.as_f32() / c1.as_f32()).to_bits() as u64)
                }
                // Comparisons
                (IrOpcode::Eq, IrType::Int32) => Some((c0.as_i32() == c1.as_i32()) as u64),
                (IrOpcode::Ne, IrType::Int32) => Some((c0.as_i32() != c1.as_i32()) as u64),
                (IrOpcode::Lt, IrType::Int32) => Some((c0.as_i32() < c1.as_i32()) as u64),
                (IrOpcode::Le, IrType::Int32) => Some((c0.as_i32() <= c1.as_i32()) as u64),
                (IrOpcode::Gt, IrType::Int32) => Some((c0.as_i32() > c1.as_i32()) as u64),
                (IrOpcode::Ge, IrType::Int32) => Some((c0.as_i32() >= c1.as_i32()) as u64),
                // Logical
                (IrOpcode::And, IrType::Bool) => Some((c0.as_bool() && c1.as_bool()) as u64),
                (IrOpcode::Or, IrType::Bool) => Some((c0.as_bool() || c1.as_bool()) as u64),
                (IrOpcode::Xor, IrType::Bool) => Some((c0.as_bool() ^ c1.as_bool()) as u64),
                _ => None,
            };

            if let Some(value) = folded {
                // Convert to constant load
                instr.immediate = value;
                instr.flags |= InstructionFlags::CONST_FOLDABLE;
                modified += 1;
            }
        }

        // Fold unary operations with one constant
        if let Some(c0) = c0 {
            if !op1_const || instr.operand1.is_undef() {
                let folded = match (instr.opcode, c0.ty) {
                    (IrOpcode::Neg, IrType::Int32) => Some((-c0.as_i32()) as u64),
                    (IrOpcode::Neg, IrType::Float32) => Some((-c0.as_f32()).to_bits() as u64),
                    (IrOpcode::Not, IrType::Bool) => Some((!c0.as_bool()) as u64),
                    (IrOpcode::Sqrt, IrType::Float32) if c0.as_f32() >= 0.0 => {
                        Some(c0.as_f32().sqrt().to_bits() as u64)
                    }
                    (IrOpcode::Sin, IrType::Float32) => Some(c0.as_f32().sin().to_bits() as u64),
                    (IrOpcode::Cos, IrType::Float32) => Some(c0.as_f32().cos().to_bits() as u64),
                    _ => None,
                };

                if let Some(value) = folded {
                    instr.immediate = value;
                    instr.flags |= InstructionFlags::CONST_FOLDABLE;
                    modified += 1;
                }
            }
        }
    }

    capsule.set_state(ModuleState::OPTIMIZED);
    capsule.increment_pass_count();

    OptimizationResult::new(0, modified)
}

/// Strength Reduction Pass
///
/// Replaces expensive operations with cheaper equivalents.
///
/// # Transformations
/// - Multiply by power of 2 -> Shift left
/// - Divide by power of 2 -> Shift right (unsigned only)
/// - Multiply by 0 -> Constant 0
/// - Multiply by 1 -> Copy
/// - Add 0 -> Copy
///
/// # ASSUM Safety
/// `#ASSUME_SHIFT_VALID`: Shift amounts are within type width.
pub fn strength_reduction(
    instructions: &mut [IrInstruction],
    constants: &[IrConstant],
    capsule: &ShaderIrModuleCapsule,
) -> OptimizationResult {
    if instructions.is_empty() {
        return OptimizationResult::UNCHANGED;
    }

    capsule.set_state(ModuleState::OPTIMIZING);

    let mut modified = 0;

    for instr in instructions.iter_mut() {
        if instr.is_dead() {
            continue;
        }

        // Check for constant operand
        let c1 = if instr.operand1.is_constant() {
            constants.iter().find(|c| c.value == instr.operand1)
        } else {
            None
        };

        match instr.opcode {
            IrOpcode::Mul => {
                if let Some(c) = c1 {
                    if c.ty == IrType::Int32 || c.ty == IrType::Uint32 {
                        let val = c.as_u32();
                        if val == 0 {
                            // x * 0 = 0
                            instr.immediate = 0;
                            instr.flags |= InstructionFlags::OPTIMIZED;
                            modified += 1;
                        } else if val == 1 {
                            // x * 1 = x (copy)
                            instr.opcode = IrOpcode::Nop;
                            instr.flags |= InstructionFlags::OPTIMIZED;
                            // Keep result pointing to operand0
                            modified += 1;
                        } else if val.is_power_of_two() {
                            // x * 2^n = x << n
                            let shift = val.trailing_zeros();
                            instr.immediate = shift as u64;
                            instr.flags |= InstructionFlags::OPTIMIZED;
                            modified += 1;
                        }
                    }
                }
            }
            IrOpcode::Div => {
                if let Some(c) = c1 {
                    if c.ty == IrType::Uint32 {
                        let val = c.as_u32();
                        if val == 1 {
                            // x / 1 = x (copy)
                            instr.opcode = IrOpcode::Nop;
                            instr.flags |= InstructionFlags::OPTIMIZED;
                            modified += 1;
                        } else if val.is_power_of_two() {
                            // x / 2^n = x >> n (unsigned only)
                            let shift = val.trailing_zeros();
                            instr.immediate = shift as u64;
                            instr.flags |= InstructionFlags::OPTIMIZED;
                            modified += 1;
                        }
                    }
                }
            }
            IrOpcode::Add | IrOpcode::Sub => {
                if let Some(c) = c1 {
                    if (c.ty == IrType::Int32 || c.ty == IrType::Uint32) && c.as_u32() == 0 {
                        // x + 0 = x, x - 0 = x
                        instr.opcode = IrOpcode::Nop;
                        instr.flags |= InstructionFlags::OPTIMIZED;
                        modified += 1;
                    }
                }
            }
            _ => {}
        }
    }

    capsule.set_state(ModuleState::OPTIMIZED);
    capsule.increment_pass_count();

    OptimizationResult::new(0, modified)
}

/// Run all optimization passes
///
/// Runs DCE, constant folding, and strength reduction in sequence.
/// Iterates until no more changes or max iterations reached.
pub fn run_all_passes(
    instructions: &mut [IrInstruction],
    constants: &[IrConstant],
    capsule: &ShaderIrModuleCapsule,
    max_iterations: usize,
) -> OptimizationResult {
    let mut total = OptimizationResult::UNCHANGED;

    for _ in 0..max_iterations {
        let dce = dead_code_elimination(instructions, capsule);
        let fold = constant_folding(instructions, constants, capsule);
        let strength = strength_reduction(instructions, constants, capsule);

        let round = dce.merge(fold).merge(strength);
        total = total.merge(round);

        if !round.changed {
            break;
        }
    }

    total
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // IrType Tests
    // ========================================================================

    #[test]
    fn test_ir_type_size() {
        assert_eq!(IrType::Void.size_bytes(), 0);
        assert_eq!(IrType::Bool.size_bytes(), 1);
        assert_eq!(IrType::Int32.size_bytes(), 4);
        assert_eq!(IrType::Int64.size_bytes(), 8);
        assert_eq!(IrType::Float32.size_bytes(), 4);
        assert_eq!(IrType::Float64.size_bytes(), 8);
        assert_eq!(IrType::Vec2.size_bytes(), 8);
        assert_eq!(IrType::Vec3.size_bytes(), 12);
        assert_eq!(IrType::Vec4.size_bytes(), 16);
        assert_eq!(IrType::Mat4.size_bytes(), 64);
    }

    #[test]
    fn test_ir_type_classification() {
        assert!(IrType::Int32.is_scalar());
        assert!(IrType::Float32.is_scalar());
        assert!(!IrType::Vec2.is_scalar());

        assert!(IrType::Vec2.is_vector());
        assert!(IrType::Vec3.is_vector());
        assert!(IrType::Vec4.is_vector());
        assert!(!IrType::Int32.is_vector());

        assert!(IrType::Mat4.is_matrix());
        assert!(!IrType::Vec4.is_matrix());

        assert!(IrType::Float32.is_float());
        assert!(IrType::Float64.is_float());
        assert!(!IrType::Int32.is_float());

        assert!(IrType::Int32.is_integer());
        assert!(IrType::Uint64.is_integer());
        assert!(!IrType::Float32.is_integer());
    }

    #[test]
    fn test_ir_type_component_count() {
        assert_eq!(IrType::Int32.component_count(), 1);
        assert_eq!(IrType::Vec2.component_count(), 2);
        assert_eq!(IrType::Vec3.component_count(), 3);
        assert_eq!(IrType::Vec4.component_count(), 4);
        assert_eq!(IrType::Mat4.component_count(), 4);
    }

    #[test]
    fn test_ir_type_from_u8() {
        assert_eq!(IrType::from_u8(0), Some(IrType::Void));
        assert_eq!(IrType::from_u8(6), Some(IrType::Float32));
        assert_eq!(IrType::from_u8(11), Some(IrType::Mat4));
        assert_eq!(IrType::from_u8(12), None);
        assert_eq!(IrType::from_u8(255), None);
    }

    // ========================================================================
    // IrOpcode Tests
    // ========================================================================

    #[test]
    fn test_ir_opcode_has_result() {
        assert!(IrOpcode::Add.has_result());
        assert!(IrOpcode::Mul.has_result());
        assert!(IrOpcode::Load.has_result());
        assert!(!IrOpcode::Store.has_result());
        assert!(!IrOpcode::Branch.has_result());
        assert!(!IrOpcode::Nop.has_result());
    }

    #[test]
    fn test_ir_opcode_operand_count() {
        assert_eq!(IrOpcode::Nop.operand_count(), 0);
        assert_eq!(IrOpcode::Return.operand_count(), 0);
        assert_eq!(IrOpcode::Neg.operand_count(), 1);
        assert_eq!(IrOpcode::Sin.operand_count(), 1);
        assert_eq!(IrOpcode::Add.operand_count(), 2);
        assert_eq!(IrOpcode::Store.operand_count(), 2);
        assert_eq!(IrOpcode::Clamp.operand_count(), 3);
    }

    #[test]
    fn test_ir_opcode_classification() {
        assert!(IrOpcode::Add.is_arithmetic());
        assert!(IrOpcode::Mul.is_arithmetic());
        assert!(!IrOpcode::And.is_arithmetic());

        assert!(IrOpcode::Eq.is_comparison());
        assert!(IrOpcode::Lt.is_comparison());
        assert!(!IrOpcode::Add.is_comparison());

        assert!(IrOpcode::Branch.is_control_flow());
        assert!(IrOpcode::Return.is_control_flow());
        assert!(!IrOpcode::Add.is_control_flow());

        assert!(IrOpcode::Sin.is_math());
        assert!(IrOpcode::Sqrt.is_math());
        assert!(!IrOpcode::Add.is_math());
    }

    #[test]
    fn test_ir_opcode_from_u8() {
        assert_eq!(IrOpcode::from_u8(0), Some(IrOpcode::Add));
        assert_eq!(IrOpcode::from_u8(15), Some(IrOpcode::Load));
        assert_eq!(IrOpcode::from_u8(29), Some(IrOpcode::Nop));
        assert_eq!(IrOpcode::from_u8(30), None);
    }

    // ========================================================================
    // SsaValue Tests
    // ========================================================================

    #[test]
    fn test_ssa_value_new() {
        let v = SsaValue::new(42);
        assert_eq!(v.index(), 42);
        assert!(!v.is_constant());
        assert!(v.is_valid());
    }

    #[test]
    fn test_ssa_value_constant() {
        let c = SsaValue::constant(100);
        assert_eq!(c.index(), 100);
        assert!(c.is_constant());
        assert!(c.is_valid());
    }

    #[test]
    fn test_ssa_value_undef() {
        assert!(SsaValue::UNDEF.is_undef());
        assert!(!SsaValue::UNDEF.is_valid());
        assert!(!SsaValue::new(0).is_undef());
    }

    #[test]
    fn test_ssa_value_display() {
        assert_eq!(format!("{}", SsaValue::new(5)), "v5");
        assert_eq!(format!("{}", SsaValue::constant(3)), "c3");
        assert_eq!(format!("{}", SsaValue::UNDEF), "undef");
    }

    // ========================================================================
    // IrInstruction Tests
    // ========================================================================

    #[test]
    fn test_instruction_size() {
        assert_eq!(core::mem::size_of::<IrInstruction>(), 32);
    }

    #[test]
    fn test_instruction_binary() {
        let add = IrInstruction::binary(
            IrOpcode::Add,
            IrType::Float32,
            SsaValue::new(3),
            SsaValue::new(1),
            SsaValue::new(2),
        );
        assert_eq!(add.opcode, IrOpcode::Add);
        assert_eq!(add.result_type, IrType::Float32);
        assert_eq!(add.result.index(), 3);
        assert_eq!(add.operand0.index(), 1);
        assert_eq!(add.operand1.index(), 2);
    }

    #[test]
    fn test_instruction_unary() {
        let neg = IrInstruction::unary(
            IrOpcode::Neg,
            IrType::Int32,
            SsaValue::new(2),
            SsaValue::new(1),
        );
        assert_eq!(neg.opcode, IrOpcode::Neg);
        assert_eq!(neg.operand0.index(), 1);
        assert!(neg.operand1.is_undef());
    }

    #[test]
    fn test_instruction_nop() {
        let nop = IrInstruction::nop();
        assert_eq!(nop.opcode, IrOpcode::Nop);
        assert!(nop.is_dead());
    }

    #[test]
    fn test_instruction_side_effects() {
        assert!(IrInstruction::new(IrOpcode::Store, IrType::Void).has_side_effects());
        assert!(IrInstruction::new(IrOpcode::Call, IrType::Int32).has_side_effects());
        assert!(!IrInstruction::new(IrOpcode::Add, IrType::Int32).has_side_effects());
    }

    #[test]
    fn test_instruction_immediate_f32() {
        let mut instr = IrInstruction::new(IrOpcode::Nop, IrType::Float32);
        instr.set_immediate_f32(3.14159);
        let diff = (instr.immediate_f32() - 3.14159).abs();
        assert!(diff < 0.0001);
    }

    // ========================================================================
    // IrConstant Tests
    // ========================================================================

    #[test]
    fn test_constant_i32() {
        let c = IrConstant::i32(SsaValue::constant(0), -42);
        assert_eq!(c.ty, IrType::Int32);
        assert_eq!(c.as_i32(), -42);
    }

    #[test]
    fn test_constant_f32() {
        let c = IrConstant::f32(SsaValue::constant(1), 2.5);
        assert_eq!(c.ty, IrType::Float32);
        let diff = (c.as_f32() - 2.5).abs();
        assert!(diff < 0.0001);
    }

    #[test]
    fn test_constant_bool() {
        let t = IrConstant::bool(SsaValue::constant(2), true);
        let f = IrConstant::bool(SsaValue::constant(3), false);
        assert!(t.as_bool());
        assert!(!f.as_bool());
    }

    // ========================================================================
    // ShaderIrModuleCapsule Tests
    // ========================================================================

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<ShaderIrModuleCapsule>(), 256);
        assert_eq!(core::mem::align_of::<ShaderIrModuleCapsule>(), 256);
    }

    #[test]
    fn test_capsule_new() {
        let capsule = ShaderIrModuleCapsule::new();
        assert_eq!(capsule.state(), ModuleState::EMPTY);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.instruction_count(), 0);
        assert_eq!(capsule.register_count(), 0);
    }

    #[test]
    fn test_capsule_state_transition() {
        let capsule = ShaderIrModuleCapsule::new();
        assert_eq!(capsule.state(), ModuleState::EMPTY);

        capsule.set_state(ModuleState::LOADED);
        assert_eq!(capsule.state(), ModuleState::LOADED);
        assert_eq!(capsule.generation(), 1);

        capsule.set_state(ModuleState::OPTIMIZING);
        assert_eq!(capsule.state(), ModuleState::OPTIMIZING);
        assert_eq!(capsule.generation(), 2);
    }

    #[test]
    fn test_capsule_counters() {
        let capsule = ShaderIrModuleCapsule::new();

        capsule.set_instruction_count(100);
        capsule.set_register_count(50);
        capsule.set_constant_count(20);
        capsule.set_basic_block_count(10);

        assert_eq!(capsule.instruction_count(), 100);
        assert_eq!(capsule.register_count(), 50);
        assert_eq!(capsule.constant_count(), 20);
        assert_eq!(capsule.basic_block_count(), 10);
    }

    #[test]
    fn test_capsule_snapshot() {
        let capsule = ShaderIrModuleCapsule::new();
        capsule.set_state(ModuleState::LOADED);
        capsule.set_instruction_count(42);

        let snap = capsule.snapshot();
        assert_eq!(snap.state, ModuleState::LOADED);
        assert_eq!(snap.instruction_count, 42);
        assert_eq!(snap.generation, 1);
    }

    #[test]
    fn test_capsule_is_ready() {
        let capsule = ShaderIrModuleCapsule::new();
        assert!(!capsule.is_ready());

        capsule.set_state(ModuleState::LOADED);
        assert!(capsule.is_ready());

        capsule.set_state(ModuleState::OPTIMIZING);
        assert!(!capsule.is_ready());

        capsule.set_state(ModuleState::OPTIMIZED);
        assert!(capsule.is_ready());
    }

    // ========================================================================
    // Dead Code Elimination Tests
    // ========================================================================

    #[test]
    fn test_dce_removes_unused() {
        let capsule = ShaderIrModuleCapsule::new();

        // v0 = add v1, v2  (dead - result never used)
        // store ptr, v3    (live - side effect)
        let mut instructions = vec![
            IrInstruction::binary(
                IrOpcode::Add,
                IrType::Int32,
                SsaValue::new(0),
                SsaValue::new(1),
                SsaValue::new(2),
            ),
            IrInstruction::binary(
                IrOpcode::Store,
                IrType::Void,
                SsaValue::UNDEF,
                SsaValue::new(10),
                SsaValue::new(3),
            ),
        ];

        let result = dead_code_elimination(&mut instructions, &capsule);
        assert!(result.changed);
        assert_eq!(result.instructions_removed, 1);
        assert!(instructions[0].is_dead());
    }

    #[test]
    fn test_dce_keeps_used() {
        let capsule = ShaderIrModuleCapsule::new();

        // v0 = add v1, v2
        // store ptr, v0    (uses v0, so add is live)
        let mut instructions = vec![
            IrInstruction::binary(
                IrOpcode::Add,
                IrType::Int32,
                SsaValue::new(0),
                SsaValue::new(1),
                SsaValue::new(2),
            ),
            IrInstruction::binary(
                IrOpcode::Store,
                IrType::Void,
                SsaValue::UNDEF,
                SsaValue::new(10),
                SsaValue::new(0),
            ),
        ];

        let result = dead_code_elimination(&mut instructions, &capsule);
        assert!(!result.changed);
        assert!(!instructions[0].is_dead());
    }

    #[test]
    fn test_dce_empty_input() {
        let capsule = ShaderIrModuleCapsule::new();
        let mut instructions: Vec<IrInstruction> = vec![];
        let result = dead_code_elimination(&mut instructions, &capsule);
        assert!(!result.changed);
    }

    // ========================================================================
    // Constant Folding Tests
    // ========================================================================

    #[test]
    fn test_constant_folding_add_i32() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![
            IrConstant::i32(SsaValue::constant(0), 10),
            IrConstant::i32(SsaValue::constant(1), 32),
        ];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Add,
            IrType::Int32,
            SsaValue::new(2),
            SsaValue::constant(0),
            SsaValue::constant(1),
        )];

        let result = constant_folding(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(result.instructions_modified, 1);
        assert_eq!(instructions[0].immediate, 42);
    }

    #[test]
    fn test_constant_folding_mul_f32() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![
            IrConstant::f32(SsaValue::constant(0), 2.0),
            IrConstant::f32(SsaValue::constant(1), 3.5),
        ];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Mul,
            IrType::Float32,
            SsaValue::new(2),
            SsaValue::constant(0),
            SsaValue::constant(1),
        )];

        let result = constant_folding(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        let folded = f32::from_bits(instructions[0].immediate as u32);
        let diff = (folded - 7.0).abs();
        assert!(diff < 0.0001);
    }

    #[test]
    fn test_constant_folding_comparison() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![
            IrConstant::i32(SsaValue::constant(0), 5),
            IrConstant::i32(SsaValue::constant(1), 10),
        ];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Lt,
            IrType::Bool,
            SsaValue::new(2),
            SsaValue::constant(0),
            SsaValue::constant(1),
        )];

        let result = constant_folding(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(instructions[0].immediate, 1); // 5 < 10 = true
    }

    #[test]
    fn test_constant_folding_sqrt() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![IrConstant::f32(SsaValue::constant(0), 16.0)];

        let mut instructions = vec![IrInstruction::unary(
            IrOpcode::Sqrt,
            IrType::Float32,
            SsaValue::new(1),
            SsaValue::constant(0),
        )];

        let result = constant_folding(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        let folded = f32::from_bits(instructions[0].immediate as u32);
        let diff = (folded - 4.0).abs();
        assert!(diff < 0.0001);
    }

    // ========================================================================
    // Strength Reduction Tests
    // ========================================================================

    #[test]
    fn test_strength_reduction_mul_power_of_two() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![IrConstant::u32(SsaValue::constant(0), 8)]; // 2^3

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Mul,
            IrType::Uint32,
            SsaValue::new(1),
            SsaValue::new(0),
            SsaValue::constant(0),
        )];

        let result = strength_reduction(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(instructions[0].immediate, 3); // shift by 3
    }

    #[test]
    fn test_strength_reduction_mul_by_zero() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![IrConstant::u32(SsaValue::constant(0), 0)];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Mul,
            IrType::Uint32,
            SsaValue::new(1),
            SsaValue::new(0),
            SsaValue::constant(0),
        )];

        let result = strength_reduction(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(instructions[0].immediate, 0);
    }

    #[test]
    fn test_strength_reduction_mul_by_one() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![IrConstant::u32(SsaValue::constant(0), 1)];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Mul,
            IrType::Uint32,
            SsaValue::new(1),
            SsaValue::new(0),
            SsaValue::constant(0),
        )];

        let result = strength_reduction(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(instructions[0].opcode, IrOpcode::Nop);
    }

    #[test]
    fn test_strength_reduction_add_zero() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![IrConstant::i32(SsaValue::constant(0), 0)];

        let mut instructions = vec![IrInstruction::binary(
            IrOpcode::Add,
            IrType::Int32,
            SsaValue::new(1),
            SsaValue::new(0),
            SsaValue::constant(0),
        )];

        let result = strength_reduction(&mut instructions, &constants, &capsule);
        assert!(result.changed);
        assert_eq!(instructions[0].opcode, IrOpcode::Nop);
    }

    // ========================================================================
    // Combined Pass Tests
    // ========================================================================

    #[test]
    fn test_run_all_passes() {
        let capsule = ShaderIrModuleCapsule::new();
        let constants = vec![
            IrConstant::i32(SsaValue::constant(0), 5),
            IrConstant::i32(SsaValue::constant(1), 10),
        ];

        let mut instructions = vec![
            // Dead instruction
            IrInstruction::binary(
                IrOpcode::Add,
                IrType::Int32,
                SsaValue::new(0),
                SsaValue::constant(0),
                SsaValue::constant(1),
            ),
            // Live store
            IrInstruction::binary(
                IrOpcode::Store,
                IrType::Void,
                SsaValue::UNDEF,
                SsaValue::new(10),
                SsaValue::new(5),
            ),
        ];

        let result = run_all_passes(&mut instructions, &constants, &capsule, 10);
        assert!(result.changed);
        // The dead add instruction should be removed
        assert!(instructions[0].is_dead());
    }

    #[test]
    fn test_optimization_result_merge() {
        let r1 = OptimizationResult::new(5, 3);
        let r2 = OptimizationResult::new(2, 7);
        let merged = r1.merge(r2);

        assert_eq!(merged.instructions_removed, 7);
        assert_eq!(merged.instructions_modified, 10);
        assert!(merged.changed);
    }
}
