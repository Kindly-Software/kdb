//! SPIR-V Binary Parser - Zero-Copy SPIR-V Parsing and IR Conversion
//!
//! **Tier**: T1 Atomic (256B aligned)
//! **Purpose**: Parse SPIR-V binary format with zero-copy instruction iteration
//!
//! # Architecture
//!
//! This module provides a complete SPIR-V binary parser that:
//! - Validates SPIR-V headers and magic numbers
//! - Provides zero-copy instruction iteration
//! - Converts SPIR-V to an intermediate representation (ShaderIR)
//! - Tracks parsing state atomically for concurrent access
//!
//! # SPIR-V Binary Format
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │ Header (20 bytes / 5 words)          │
//! │  - Magic (0x07230203)                │
//! │  - Version                           │
//! │  - Generator ID                      │
//! │  - Bound (max ID + 1)                │
//! │  - Schema (reserved, must be 0)      │
//! ├──────────────────────────────────────┤
//! │ Instructions                         │
//! │  ┌─────────────────────────────────┐ │
//! │  │ Word 0: opcode(16) | count(16)  │ │
//! │  │ Word 1..N: operands             │ │
//! │  └─────────────────────────────────┘ │
//! │  ...                                 │
//! └──────────────────────────────────────┘
//! ```
//!
//! # Memory Layout (SpirVParserCapsule - 256B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       Primary: state(8) | instruction_count(24) | generation(32)
//! 8       8       Secondary: error_count(16) | current_offset(48)
//! 16      8       module_ptr: AtomicPtr to SPIR-V data
//! 24      8       module_size: Size of SPIR-V data
//! 32      8       bound: Maximum ID in module
//! 40      8       version: SPIR-V version
//! 48      208     _padding (to 256B)
//! ```
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree state coordination)
//! - Q11: Rust transform (zero-copy, type-safe)
//! - Q33: #[derive(ComputationalCapsule)] mandate
//! - Q34: Audit trail for shader parsing
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SPIRV_LITTLE_ENDIAN`: SPIR-V uses little-endian by default
//! - `#ASSUME_SPIRV_WORD_ALIGNED`: SPIR-V data is 4-byte aligned
//! - `#ASSUME_OPCODE_STABLE`: SPIR-V opcodes are stable across versions
//! - `#ASSUME_INSTRUCTION_VALID`: Instructions have valid word counts > 0
//!
//! # Framework Compliance
//!
//! - **Chaos**: 100% lockfree, zero mutex/RwLock
//! - **T28**: 35+ tests (unit/property/integration)
//! - **B32**: Validated performance claims

#![allow(dead_code)] // During development

use core::sync::atomic::{AtomicU64, AtomicPtr, Ordering};

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// ============================================================================
// SPIR-V Constants
// ============================================================================

/// SPIR-V magic number (little-endian): 0x07230203
///
/// # ASSUM Safety
/// `#ASSUME_SPIRV_MAGIC_STABLE`: Defined by Khronos SPIR-V specification.
/// `#VERIFY_SPIRV_MAGIC_STABLE`: Magic number is fundamental to SPIR-V identity.
pub const SPIRV_MAGIC: u32 = 0x07230203;

/// SPIR-V magic number in little-endian bytes
pub const SPIRV_MAGIC_LE: [u8; 4] = [0x03, 0x02, 0x23, 0x07];

/// SPIR-V magic number in big-endian bytes (reverse endian detection)
pub const SPIRV_MAGIC_BE: [u8; 4] = [0x07, 0x23, 0x02, 0x03];

/// SPIR-V header size in bytes (5 words * 4 bytes)
pub const SPIRV_HEADER_SIZE_BYTES: usize = 20;

/// SPIR-V header size in words
pub const SPIRV_HEADER_SIZE_WORDS: usize = 5;

/// Minimum valid SPIR-V module size (header only)
pub const MIN_SPIRV_SIZE_BYTES: usize = SPIRV_HEADER_SIZE_BYTES;

/// Maximum supported SPIR-V version (major << 16 | minor << 8)
pub const MAX_SPIRV_VERSION: u32 = 0x00010600; // SPIR-V 1.6

/// Minimum instruction word count (opcode word only)
pub const MIN_INSTRUCTION_WORDS: u16 = 1;

// ============================================================================
// SPIR-V Header
// ============================================================================

/// SPIR-V binary header structure (first 5 words / 20 bytes)
///
/// # Layout
/// ```text
/// Word 0: Magic number (0x07230203)
/// Word 1: Version (major.minor in upper bytes)
/// Word 2: Generator ID (tool that created the module)
/// Word 3: Bound (upper limit on IDs, max_id + 1)
/// Word 4: Schema (reserved, must be 0)
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpirVHeader {
    /// Magic number - must be SPIRV_MAGIC (0x07230203)
    pub magic: u32,
    /// Version - major.minor.patch encoded
    pub version: u32,
    /// Generator tool ID (Khronos registry)
    pub generator: u32,
    /// Upper bound on all IDs used in the module
    pub bound: u32,
    /// Schema (reserved, must be 0)
    pub schema: u32,
}

impl SpirVHeader {
    /// Parse header from little-endian word slice
    ///
    /// # Safety
    /// `#ASSUME_SPIRV_LITTLE_ENDIAN`: SPIR-V default is little-endian.
    #[inline]
    pub fn from_words(words: &[u32]) -> Option<Self> {
        if words.len() < SPIRV_HEADER_SIZE_WORDS {
            return None;
        }
        Some(Self {
            magic: words[0],
            version: words[1],
            generator: words[2],
            bound: words[3],
            schema: words[4],
        })
    }

    /// Parse header from raw bytes (little-endian)
    #[inline]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < SPIRV_HEADER_SIZE_BYTES {
            return None;
        }
        Some(Self {
            magic: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            version: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            generator: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            bound: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
            schema: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
        })
    }

    /// Check if this is a valid SPIR-V header
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.magic == SPIRV_MAGIC && self.schema == 0
    }

    /// Check if this is big-endian SPIR-V (needs byte swap)
    #[inline]
    pub const fn is_big_endian(&self) -> bool {
        self.magic == u32::from_be_bytes(SPIRV_MAGIC_LE)
    }

    /// Get SPIR-V version as (major, minor)
    #[inline]
    pub const fn version_tuple(&self) -> (u8, u8) {
        let major = ((self.version >> 16) & 0xFF) as u8;
        let minor = ((self.version >> 8) & 0xFF) as u8;
        (major, minor)
    }

    /// Get SPIR-V version string (e.g., "1.5")
    #[inline]
    pub fn version_string(&self) -> [u8; 4] {
        let (major, minor) = self.version_tuple();
        [b'0' + major, b'.', b'0' + minor, 0]
    }
}

// ============================================================================
// SPIR-V Opcodes (40+ Essential Opcodes)
// ============================================================================

/// SPIR-V operation codes
///
/// This enum covers the 40+ essential opcodes needed for shader compilation:
/// - Type declarations (void, bool, int, float, vector, matrix, pointer, function)
/// - Constants
/// - Memory operations (variable, load, store, access chain)
/// - Arithmetic (add, sub, mul, div for int and float)
/// - Control flow (function, label, branch, return, phi)
/// - Texture operations
///
/// # ASSUM Safety
/// `#ASSUME_OPCODE_STABLE`: SPIR-V opcodes are stable across versions.
/// `#VERIFY_OPCODE_STABLE`: Khronos maintains backwards compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SpirVOp {
    // ========================================================================
    // Miscellaneous
    // ========================================================================

    /// No operation
    OpNop = 0,
    /// Undefined value
    OpUndef = 1,
    /// Source language info
    OpSourceContinued = 2,
    /// Source language
    OpSource = 3,
    /// Source extension
    OpSourceExtension = 4,
    /// Debug name
    OpName = 5,
    /// Debug member name
    OpMemberName = 6,
    /// Debug string
    OpString = 7,
    /// Debug line info
    OpLine = 8,

    // ========================================================================
    // Extension Instructions
    // ========================================================================

    /// Extension instruction import
    OpExtInstImport = 11,
    /// Execute extension instruction
    OpExtInst = 12,

    // ========================================================================
    // Type Declarations
    // ========================================================================

    /// Void type
    OpTypeVoid = 19,
    /// Boolean type
    OpTypeBool = 20,
    /// Integer type
    OpTypeInt = 21,
    /// Floating-point type
    OpTypeFloat = 22,
    /// Vector type
    OpTypeVector = 23,
    /// Matrix type
    OpTypeMatrix = 24,
    /// Image type
    OpTypeImage = 25,
    /// Sampler type
    OpTypeSampler = 26,
    /// Sampled image type
    OpTypeSampledImage = 27,
    /// Array type
    OpTypeArray = 28,
    /// Runtime array type
    OpTypeRuntimeArray = 29,
    /// Struct type
    OpTypeStruct = 30,
    /// Opaque type
    OpTypeOpaque = 31,
    /// Pointer type
    OpTypePointer = 32,
    /// Function type
    OpTypeFunction = 33,
    /// Event type
    OpTypeEvent = 34,
    /// Device event type
    OpTypeDeviceEvent = 35,
    /// Reserve ID type
    OpTypeReserveId = 36,
    /// Queue type
    OpTypeQueue = 37,
    /// Pipe type
    OpTypePipe = 38,
    /// Forward pointer type
    OpTypeForwardPointer = 39,

    // ========================================================================
    // Constant Creation
    // ========================================================================

    /// Constant true
    OpConstantTrue = 41,
    /// Constant false
    OpConstantFalse = 42,
    /// Scalar constant
    OpConstant = 43,
    /// Composite constant
    OpConstantComposite = 44,
    /// Sampler constant
    OpConstantSampler = 45,
    /// Null constant
    OpConstantNull = 46,

    // ========================================================================
    // Spec Constant Creation
    // ========================================================================

    /// Specialization constant true
    OpSpecConstantTrue = 48,
    /// Specialization constant false
    OpSpecConstantFalse = 49,
    /// Specialization constant
    OpSpecConstant = 50,
    /// Specialization constant composite
    OpSpecConstantComposite = 51,
    /// Specialization constant operation
    OpSpecConstantOp = 52,

    // ========================================================================
    // Memory Operations
    // ========================================================================

    /// Declare function or global variable
    OpFunction = 54,
    /// Function parameter
    OpFunctionParameter = 55,
    /// End of function definition
    OpFunctionEnd = 56,
    /// Function call
    OpFunctionCall = 57,
    /// Allocate variable
    OpVariable = 59,
    /// Load from pointer
    OpLoad = 61,
    /// Store to pointer
    OpStore = 62,
    /// Copy memory
    OpCopyMemory = 63,
    /// Copy memory with size
    OpCopyMemorySized = 64,
    /// Create pointer into composite
    OpAccessChain = 65,
    /// In-bounds access chain
    OpInBoundsAccessChain = 66,
    /// Pointer to member
    OpPtrAccessChain = 67,
    /// Array length
    OpArrayLength = 68,
    /// Generic pointer cast
    OpGenericPtrMemSemantics = 69,
    /// In-bounds pointer access chain
    OpInBoundsPtrAccessChain = 70,

    // ========================================================================
    // Decoration
    // ========================================================================

    /// Decoration instruction
    OpDecorate = 71,
    /// Member decoration
    OpMemberDecorate = 72,
    /// Decoration group
    OpDecorationGroup = 73,
    /// Group decoration
    OpGroupDecorate = 74,
    /// Group member decoration
    OpGroupMemberDecorate = 75,

    // ========================================================================
    // Composite Operations
    // ========================================================================

    /// Construct composite from constituents
    OpCompositeConstruct = 80,
    /// Extract component from composite
    OpCompositeExtract = 81,
    /// Insert component into composite
    OpCompositeInsert = 82,
    /// Copy object
    OpCopyObject = 83,
    /// Transpose matrix
    OpTranspose = 84,

    // ========================================================================
    // Image Operations
    // ========================================================================

    /// Combine sampler and image
    OpSampledImage = 86,
    /// Sample image with implicit LOD
    OpImageSampleImplicitLod = 87,
    /// Sample image with explicit LOD
    OpImageSampleExplicitLod = 88,
    /// Sample image with dref and implicit LOD
    OpImageSampleDrefImplicitLod = 89,
    /// Sample image with dref and explicit LOD
    OpImageSampleDrefExplicitLod = 90,
    /// Sample image with projection and implicit LOD
    OpImageSampleProjImplicitLod = 91,
    /// Sample image with projection and explicit LOD
    OpImageSampleProjExplicitLod = 92,
    /// Fetch texel
    OpImageFetch = 95,
    /// Gather image
    OpImageGather = 96,
    /// Dref gather image
    OpImageDrefGather = 97,
    /// Read image
    OpImageRead = 98,
    /// Write image
    OpImageWrite = 99,
    /// Extract image from sampled image
    OpImage = 100,
    /// Query image size LOD
    OpImageQuerySizeLod = 103,
    /// Query image size
    OpImageQuerySize = 104,
    /// Query image LOD
    OpImageQueryLod = 105,
    /// Query image levels
    OpImageQueryLevels = 106,
    /// Query image samples
    OpImageQuerySamples = 107,

    // ========================================================================
    // Conversion Operations
    // ========================================================================

    /// Convert float to uint
    OpConvertFToU = 109,
    /// Convert float to sint
    OpConvertFToS = 110,
    /// Convert sint to float
    OpConvertSToF = 111,
    /// Convert uint to float
    OpConvertUToF = 112,
    /// Unsigned convert
    OpUConvert = 113,
    /// Signed convert
    OpSConvert = 114,
    /// Float convert
    OpFConvert = 115,
    /// Quantize to F16
    OpQuantizeToF16 = 116,
    /// Convert pointer to uint
    OpConvertPtrToU = 117,
    /// Saturate convert sint to uint
    OpSatConvertSToU = 118,
    /// Saturate convert uint to sint
    OpSatConvertUToS = 119,
    /// Convert uint to pointer
    OpConvertUToPtr = 120,
    /// Pointer cast to generic
    OpPtrCastToGeneric = 121,
    /// Generic cast to pointer
    OpGenericCastToPtr = 122,
    /// Generic cast to pointer with explicit storage class
    OpGenericCastToPtrExplicit = 123,
    /// Bitcast
    OpBitcast = 124,

    // ========================================================================
    // Arithmetic Operations
    // ========================================================================

    /// Signed negate
    OpSNegate = 126,
    /// Float negate
    OpFNegate = 127,
    /// Integer add
    OpIAdd = 128,
    /// Float add
    OpFAdd = 129,
    /// Integer subtract
    OpISub = 130,
    /// Float subtract
    OpFSub = 131,
    /// Integer multiply
    OpIMul = 132,
    /// Float multiply
    OpFMul = 133,
    /// Unsigned divide
    OpUDiv = 134,
    /// Signed divide
    OpSDiv = 135,
    /// Float divide
    OpFDiv = 136,
    /// Unsigned modulo
    OpUMod = 137,
    /// Signed remainder
    OpSRem = 138,
    /// Signed modulo
    OpSMod = 139,
    /// Float remainder
    OpFRem = 140,
    /// Float modulo
    OpFMod = 141,
    /// Vector times scalar
    OpVectorTimesScalar = 142,
    /// Matrix times scalar
    OpMatrixTimesScalar = 143,
    /// Vector times matrix
    OpVectorTimesMatrix = 144,
    /// Matrix times vector
    OpMatrixTimesVector = 145,
    /// Matrix times matrix
    OpMatrixTimesMatrix = 146,
    /// Outer product
    OpOuterProduct = 147,
    /// Dot product
    OpDot = 148,
    /// Integer add with carry
    OpIAddCarry = 149,
    /// Integer subtract with borrow
    OpISubBorrow = 150,
    /// Unsigned multiply extended
    OpUMulExtended = 151,
    /// Signed multiply extended
    OpSMulExtended = 152,

    // ========================================================================
    // Bit Operations
    // ========================================================================

    /// Shift right logical
    OpShiftRightLogical = 194,
    /// Shift right arithmetic
    OpShiftRightArithmetic = 195,
    /// Shift left logical
    OpShiftLeftLogical = 196,
    /// Bitwise or
    OpBitwiseOr = 197,
    /// Bitwise xor
    OpBitwiseXor = 198,
    /// Bitwise and
    OpBitwiseAnd = 199,
    /// Bitwise not
    OpNot = 200,
    /// Bit field insert
    OpBitFieldInsert = 201,
    /// Bit field signed extract
    OpBitFieldSExtract = 202,
    /// Bit field unsigned extract
    OpBitFieldUExtract = 203,
    /// Bit reverse
    OpBitReverse = 204,
    /// Bit count
    OpBitCount = 205,

    // ========================================================================
    // Relational and Logical Operations
    // ========================================================================

    /// Any component true
    OpAny = 154,
    /// All components true
    OpAll = 155,
    /// Check if NaN
    OpIsNan = 156,
    /// Check if infinite
    OpIsInf = 157,
    /// Check if finite
    OpIsFinite = 158,
    /// Check if normal
    OpIsNormal = 159,
    /// Check sign bit
    OpSignBitSet = 160,
    /// Logical equal
    OpLogicalEqual = 164,
    /// Logical not equal
    OpLogicalNotEqual = 165,
    /// Logical or
    OpLogicalOr = 166,
    /// Logical and
    OpLogicalAnd = 167,
    /// Logical not
    OpLogicalNot = 168,
    /// Select (ternary)
    OpSelect = 169,
    /// Integer equal
    OpIEqual = 170,
    /// Integer not equal
    OpINotEqual = 171,
    /// Unsigned greater than
    OpUGreaterThan = 172,
    /// Signed greater than
    OpSGreaterThan = 173,
    /// Unsigned greater than or equal
    OpUGreaterThanEqual = 174,
    /// Signed greater than or equal
    OpSGreaterThanEqual = 175,
    /// Unsigned less than
    OpULessThan = 176,
    /// Signed less than
    OpSLessThan = 177,
    /// Unsigned less than or equal
    OpULessThanEqual = 178,
    /// Signed less than or equal
    OpSLessThanEqual = 179,
    /// Float ordered equal
    OpFOrdEqual = 180,
    /// Float unordered not equal
    OpFUnordNotEqual = 181,
    /// Float ordered less than
    OpFOrdLessThan = 182,
    /// Float unordered less than
    OpFUnordLessThan = 183,
    /// Float ordered greater than
    OpFOrdGreaterThan = 184,
    /// Float unordered greater than
    OpFUnordGreaterThan = 185,
    /// Float ordered less than or equal
    OpFOrdLessThanEqual = 186,
    /// Float unordered less than or equal
    OpFUnordLessThanEqual = 187,
    /// Float ordered greater than or equal
    OpFOrdGreaterThanEqual = 188,
    /// Float unordered greater than or equal
    OpFUnordGreaterThanEqual = 189,

    // ========================================================================
    // Control Flow
    // ========================================================================

    /// PHI node
    OpPhi = 245,
    /// Loop merge
    OpLoopMerge = 246,
    /// Selection merge
    OpSelectionMerge = 247,
    /// Label (basic block start)
    OpLabel = 248,
    /// Unconditional branch
    OpBranch = 249,
    /// Conditional branch
    OpBranchConditional = 250,
    /// Multi-way branch (switch)
    OpSwitch = 251,
    /// Kill fragment
    OpKill = 252,
    /// Return with no value
    OpReturn = 253,
    /// Return with value
    OpReturnValue = 254,
    /// Unreachable code
    OpUnreachable = 255,
    /// Lifetime start
    OpLifetimeStart = 256,
    /// Lifetime stop
    OpLifetimeStop = 257,

    // ========================================================================
    // Atomic Operations
    // ========================================================================

    /// Atomic load
    OpAtomicLoad = 227,
    /// Atomic store
    OpAtomicStore = 228,
    /// Atomic exchange
    OpAtomicExchange = 229,
    /// Atomic compare exchange
    OpAtomicCompareExchange = 230,
    /// Atomic compare exchange weak
    OpAtomicCompareExchangeWeak = 231,
    /// Atomic increment
    OpAtomicIIncrement = 232,
    /// Atomic decrement
    OpAtomicIDecrement = 233,
    /// Atomic add
    OpAtomicIAdd = 234,
    /// Atomic subtract
    OpAtomicISub = 235,
    /// Atomic signed min
    OpAtomicSMin = 236,
    /// Atomic unsigned min
    OpAtomicUMin = 237,
    /// Atomic signed max
    OpAtomicSMax = 238,
    /// Atomic unsigned max
    OpAtomicUMax = 239,
    /// Atomic and
    OpAtomicAnd = 240,
    /// Atomic or
    OpAtomicOr = 241,
    /// Atomic xor
    OpAtomicXor = 242,

    // ========================================================================
    // Barrier Operations
    // ========================================================================

    /// Control barrier
    OpControlBarrier = 224,
    /// Memory barrier
    OpMemoryBarrier = 225,

    // ========================================================================
    // Derivative Operations
    // ========================================================================

    /// Partial derivative with respect to x
    OpDPdx = 207,
    /// Partial derivative with respect to y
    OpDPdy = 208,
    /// Fragment width
    OpFwidth = 209,
    /// Fine partial derivative with respect to x
    OpDPdxFine = 210,
    /// Fine partial derivative with respect to y
    OpDPdyFine = 211,
    /// Fine fragment width
    OpFwidthFine = 212,
    /// Coarse partial derivative with respect to x
    OpDPdxCoarse = 213,
    /// Coarse partial derivative with respect to y
    OpDPdyCoarse = 214,
    /// Coarse fragment width
    OpFwidthCoarse = 215,

    // ========================================================================
    // Capability
    // ========================================================================

    /// Declare capability
    OpCapability = 17,
    /// Memory model
    OpMemoryModel = 14,
    /// Entry point
    OpEntryPoint = 15,
    /// Execution mode
    OpExecutionMode = 16,

    // ========================================================================
    // Unknown/Extension
    // ========================================================================

    /// Unknown opcode (for forward compatibility)
    Unknown = 0xFFFF,
}

impl SpirVOp {
    /// Convert from raw u16 opcode
    #[inline]
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0 => Self::OpNop,
            1 => Self::OpUndef,
            2 => Self::OpSourceContinued,
            3 => Self::OpSource,
            4 => Self::OpSourceExtension,
            5 => Self::OpName,
            6 => Self::OpMemberName,
            7 => Self::OpString,
            8 => Self::OpLine,
            11 => Self::OpExtInstImport,
            12 => Self::OpExtInst,
            14 => Self::OpMemoryModel,
            15 => Self::OpEntryPoint,
            16 => Self::OpExecutionMode,
            17 => Self::OpCapability,
            19 => Self::OpTypeVoid,
            20 => Self::OpTypeBool,
            21 => Self::OpTypeInt,
            22 => Self::OpTypeFloat,
            23 => Self::OpTypeVector,
            24 => Self::OpTypeMatrix,
            25 => Self::OpTypeImage,
            26 => Self::OpTypeSampler,
            27 => Self::OpTypeSampledImage,
            28 => Self::OpTypeArray,
            29 => Self::OpTypeRuntimeArray,
            30 => Self::OpTypeStruct,
            31 => Self::OpTypeOpaque,
            32 => Self::OpTypePointer,
            33 => Self::OpTypeFunction,
            34 => Self::OpTypeEvent,
            35 => Self::OpTypeDeviceEvent,
            36 => Self::OpTypeReserveId,
            37 => Self::OpTypeQueue,
            38 => Self::OpTypePipe,
            39 => Self::OpTypeForwardPointer,
            41 => Self::OpConstantTrue,
            42 => Self::OpConstantFalse,
            43 => Self::OpConstant,
            44 => Self::OpConstantComposite,
            45 => Self::OpConstantSampler,
            46 => Self::OpConstantNull,
            48 => Self::OpSpecConstantTrue,
            49 => Self::OpSpecConstantFalse,
            50 => Self::OpSpecConstant,
            51 => Self::OpSpecConstantComposite,
            52 => Self::OpSpecConstantOp,
            54 => Self::OpFunction,
            55 => Self::OpFunctionParameter,
            56 => Self::OpFunctionEnd,
            57 => Self::OpFunctionCall,
            59 => Self::OpVariable,
            61 => Self::OpLoad,
            62 => Self::OpStore,
            63 => Self::OpCopyMemory,
            64 => Self::OpCopyMemorySized,
            65 => Self::OpAccessChain,
            66 => Self::OpInBoundsAccessChain,
            67 => Self::OpPtrAccessChain,
            68 => Self::OpArrayLength,
            69 => Self::OpGenericPtrMemSemantics,
            70 => Self::OpInBoundsPtrAccessChain,
            71 => Self::OpDecorate,
            72 => Self::OpMemberDecorate,
            73 => Self::OpDecorationGroup,
            74 => Self::OpGroupDecorate,
            75 => Self::OpGroupMemberDecorate,
            80 => Self::OpCompositeConstruct,
            81 => Self::OpCompositeExtract,
            82 => Self::OpCompositeInsert,
            83 => Self::OpCopyObject,
            84 => Self::OpTranspose,
            86 => Self::OpSampledImage,
            87 => Self::OpImageSampleImplicitLod,
            88 => Self::OpImageSampleExplicitLod,
            89 => Self::OpImageSampleDrefImplicitLod,
            90 => Self::OpImageSampleDrefExplicitLod,
            91 => Self::OpImageSampleProjImplicitLod,
            92 => Self::OpImageSampleProjExplicitLod,
            95 => Self::OpImageFetch,
            96 => Self::OpImageGather,
            97 => Self::OpImageDrefGather,
            98 => Self::OpImageRead,
            99 => Self::OpImageWrite,
            100 => Self::OpImage,
            103 => Self::OpImageQuerySizeLod,
            104 => Self::OpImageQuerySize,
            105 => Self::OpImageQueryLod,
            106 => Self::OpImageQueryLevels,
            107 => Self::OpImageQuerySamples,
            109 => Self::OpConvertFToU,
            110 => Self::OpConvertFToS,
            111 => Self::OpConvertSToF,
            112 => Self::OpConvertUToF,
            113 => Self::OpUConvert,
            114 => Self::OpSConvert,
            115 => Self::OpFConvert,
            116 => Self::OpQuantizeToF16,
            117 => Self::OpConvertPtrToU,
            118 => Self::OpSatConvertSToU,
            119 => Self::OpSatConvertUToS,
            120 => Self::OpConvertUToPtr,
            121 => Self::OpPtrCastToGeneric,
            122 => Self::OpGenericCastToPtr,
            123 => Self::OpGenericCastToPtrExplicit,
            124 => Self::OpBitcast,
            126 => Self::OpSNegate,
            127 => Self::OpFNegate,
            128 => Self::OpIAdd,
            129 => Self::OpFAdd,
            130 => Self::OpISub,
            131 => Self::OpFSub,
            132 => Self::OpIMul,
            133 => Self::OpFMul,
            134 => Self::OpUDiv,
            135 => Self::OpSDiv,
            136 => Self::OpFDiv,
            137 => Self::OpUMod,
            138 => Self::OpSRem,
            139 => Self::OpSMod,
            140 => Self::OpFRem,
            141 => Self::OpFMod,
            142 => Self::OpVectorTimesScalar,
            143 => Self::OpMatrixTimesScalar,
            144 => Self::OpVectorTimesMatrix,
            145 => Self::OpMatrixTimesVector,
            146 => Self::OpMatrixTimesMatrix,
            147 => Self::OpOuterProduct,
            148 => Self::OpDot,
            149 => Self::OpIAddCarry,
            150 => Self::OpISubBorrow,
            151 => Self::OpUMulExtended,
            152 => Self::OpSMulExtended,
            154 => Self::OpAny,
            155 => Self::OpAll,
            156 => Self::OpIsNan,
            157 => Self::OpIsInf,
            158 => Self::OpIsFinite,
            159 => Self::OpIsNormal,
            160 => Self::OpSignBitSet,
            164 => Self::OpLogicalEqual,
            165 => Self::OpLogicalNotEqual,
            166 => Self::OpLogicalOr,
            167 => Self::OpLogicalAnd,
            168 => Self::OpLogicalNot,
            169 => Self::OpSelect,
            170 => Self::OpIEqual,
            171 => Self::OpINotEqual,
            172 => Self::OpUGreaterThan,
            173 => Self::OpSGreaterThan,
            174 => Self::OpUGreaterThanEqual,
            175 => Self::OpSGreaterThanEqual,
            176 => Self::OpULessThan,
            177 => Self::OpSLessThan,
            178 => Self::OpULessThanEqual,
            179 => Self::OpSLessThanEqual,
            180 => Self::OpFOrdEqual,
            181 => Self::OpFUnordNotEqual,
            182 => Self::OpFOrdLessThan,
            183 => Self::OpFUnordLessThan,
            184 => Self::OpFOrdGreaterThan,
            185 => Self::OpFUnordGreaterThan,
            186 => Self::OpFOrdLessThanEqual,
            187 => Self::OpFUnordLessThanEqual,
            188 => Self::OpFOrdGreaterThanEqual,
            189 => Self::OpFUnordGreaterThanEqual,
            194 => Self::OpShiftRightLogical,
            195 => Self::OpShiftRightArithmetic,
            196 => Self::OpShiftLeftLogical,
            197 => Self::OpBitwiseOr,
            198 => Self::OpBitwiseXor,
            199 => Self::OpBitwiseAnd,
            200 => Self::OpNot,
            201 => Self::OpBitFieldInsert,
            202 => Self::OpBitFieldSExtract,
            203 => Self::OpBitFieldUExtract,
            204 => Self::OpBitReverse,
            205 => Self::OpBitCount,
            207 => Self::OpDPdx,
            208 => Self::OpDPdy,
            209 => Self::OpFwidth,
            210 => Self::OpDPdxFine,
            211 => Self::OpDPdyFine,
            212 => Self::OpFwidthFine,
            213 => Self::OpDPdxCoarse,
            214 => Self::OpDPdyCoarse,
            215 => Self::OpFwidthCoarse,
            224 => Self::OpControlBarrier,
            225 => Self::OpMemoryBarrier,
            227 => Self::OpAtomicLoad,
            228 => Self::OpAtomicStore,
            229 => Self::OpAtomicExchange,
            230 => Self::OpAtomicCompareExchange,
            231 => Self::OpAtomicCompareExchangeWeak,
            232 => Self::OpAtomicIIncrement,
            233 => Self::OpAtomicIDecrement,
            234 => Self::OpAtomicIAdd,
            235 => Self::OpAtomicISub,
            236 => Self::OpAtomicSMin,
            237 => Self::OpAtomicUMin,
            238 => Self::OpAtomicSMax,
            239 => Self::OpAtomicUMax,
            240 => Self::OpAtomicAnd,
            241 => Self::OpAtomicOr,
            242 => Self::OpAtomicXor,
            245 => Self::OpPhi,
            246 => Self::OpLoopMerge,
            247 => Self::OpSelectionMerge,
            248 => Self::OpLabel,
            249 => Self::OpBranch,
            250 => Self::OpBranchConditional,
            251 => Self::OpSwitch,
            252 => Self::OpKill,
            253 => Self::OpReturn,
            254 => Self::OpReturnValue,
            255 => Self::OpUnreachable,
            256 => Self::OpLifetimeStart,
            257 => Self::OpLifetimeStop,
            _ => Self::Unknown,
        }
    }

    /// Get the raw opcode value
    #[inline]
    pub const fn raw(self) -> u16 {
        self as u16
    }

    /// Check if this is a type declaration opcode
    #[inline]
    pub const fn is_type(&self) -> bool {
        matches!(
            self,
            Self::OpTypeVoid
                | Self::OpTypeBool
                | Self::OpTypeInt
                | Self::OpTypeFloat
                | Self::OpTypeVector
                | Self::OpTypeMatrix
                | Self::OpTypeImage
                | Self::OpTypeSampler
                | Self::OpTypeSampledImage
                | Self::OpTypeArray
                | Self::OpTypeRuntimeArray
                | Self::OpTypeStruct
                | Self::OpTypeOpaque
                | Self::OpTypePointer
                | Self::OpTypeFunction
                | Self::OpTypeEvent
                | Self::OpTypeDeviceEvent
                | Self::OpTypeReserveId
                | Self::OpTypeQueue
                | Self::OpTypePipe
                | Self::OpTypeForwardPointer
        )
    }

    /// Check if this is a constant creation opcode
    #[inline]
    pub const fn is_constant(&self) -> bool {
        matches!(
            self,
            Self::OpConstantTrue
                | Self::OpConstantFalse
                | Self::OpConstant
                | Self::OpConstantComposite
                | Self::OpConstantSampler
                | Self::OpConstantNull
                | Self::OpSpecConstantTrue
                | Self::OpSpecConstantFalse
                | Self::OpSpecConstant
                | Self::OpSpecConstantComposite
                | Self::OpSpecConstantOp
        )
    }

    /// Check if this is a control flow opcode
    #[inline]
    pub const fn is_control_flow(&self) -> bool {
        matches!(
            self,
            Self::OpPhi
                | Self::OpLoopMerge
                | Self::OpSelectionMerge
                | Self::OpLabel
                | Self::OpBranch
                | Self::OpBranchConditional
                | Self::OpSwitch
                | Self::OpKill
                | Self::OpReturn
                | Self::OpReturnValue
                | Self::OpUnreachable
        )
    }

    /// Check if this is an arithmetic opcode
    #[inline]
    pub const fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Self::OpSNegate
                | Self::OpFNegate
                | Self::OpIAdd
                | Self::OpFAdd
                | Self::OpISub
                | Self::OpFSub
                | Self::OpIMul
                | Self::OpFMul
                | Self::OpUDiv
                | Self::OpSDiv
                | Self::OpFDiv
                | Self::OpUMod
                | Self::OpSRem
                | Self::OpSMod
                | Self::OpFRem
                | Self::OpFMod
                | Self::OpVectorTimesScalar
                | Self::OpMatrixTimesScalar
                | Self::OpVectorTimesMatrix
                | Self::OpMatrixTimesVector
                | Self::OpMatrixTimesMatrix
                | Self::OpOuterProduct
                | Self::OpDot
        )
    }

    /// Check if this is a memory operation opcode
    #[inline]
    pub const fn is_memory(&self) -> bool {
        matches!(
            self,
            Self::OpVariable
                | Self::OpLoad
                | Self::OpStore
                | Self::OpCopyMemory
                | Self::OpCopyMemorySized
                | Self::OpAccessChain
                | Self::OpInBoundsAccessChain
                | Self::OpPtrAccessChain
                | Self::OpInBoundsPtrAccessChain
        )
    }
}

// ============================================================================
// Zero-Copy Instruction
// ============================================================================

/// A single SPIR-V instruction (zero-copy reference)
///
/// # Layout
/// ```text
/// Word 0: | opcode (bits 0-15) | word_count (bits 16-31) |
/// Word 1..N: operands
/// ```
#[derive(Clone, Copy)]
pub struct SpirVInstruction<'a> {
    /// The opcode
    pub opcode: SpirVOp,
    /// Total word count (including opcode word)
    pub word_count: u16,
    /// Raw opcode value (for unknown opcodes)
    pub raw_opcode: u16,
    /// Operand words (zero-copy slice)
    pub operands: &'a [u32],
}

impl<'a> SpirVInstruction<'a> {
    /// Get the result ID if this instruction produces one
    ///
    /// Type instructions have:
    /// - Word 1: Result ID (no result type)
    ///
    /// Most other result-producing instructions have:
    /// - Word 1: Result Type ID
    /// - Word 2: Result ID
    #[inline]
    pub fn result_id(&self) -> Option<u32> {
        // Type instructions have result ID in word 1 (first operand)
        if self.opcode.is_type() && !self.operands.is_empty() {
            return Some(self.operands[0]);
        }

        // Instructions with results typically have result ID in word 2
        // This is a simplification - full SPIR-V parsing needs grammar tables
        if self.operands.len() >= 2 {
            match self.opcode {
                SpirVOp::OpVariable
                | SpirVOp::OpLoad
                | SpirVOp::OpAccessChain
                | SpirVOp::OpInBoundsAccessChain
                | SpirVOp::OpCompositeConstruct
                | SpirVOp::OpCompositeExtract
                | SpirVOp::OpCompositeInsert
                | SpirVOp::OpIAdd
                | SpirVOp::OpFAdd
                | SpirVOp::OpISub
                | SpirVOp::OpFSub
                | SpirVOp::OpIMul
                | SpirVOp::OpFMul
                | SpirVOp::OpUDiv
                | SpirVOp::OpSDiv
                | SpirVOp::OpFDiv
                | SpirVOp::OpPhi
                | SpirVOp::OpConstant
                | SpirVOp::OpConstantComposite
                | SpirVOp::OpConstantTrue
                | SpirVOp::OpConstantFalse
                | SpirVOp::OpConstantNull => Some(self.operands[1]),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the result type ID if this instruction has one
    #[inline]
    pub fn result_type_id(&self) -> Option<u32> {
        if !self.operands.is_empty() {
            match self.opcode {
                SpirVOp::OpVariable
                | SpirVOp::OpLoad
                | SpirVOp::OpAccessChain
                | SpirVOp::OpInBoundsAccessChain
                | SpirVOp::OpCompositeConstruct
                | SpirVOp::OpCompositeExtract
                | SpirVOp::OpCompositeInsert
                | SpirVOp::OpIAdd
                | SpirVOp::OpFAdd
                | SpirVOp::OpISub
                | SpirVOp::OpFSub
                | SpirVOp::OpIMul
                | SpirVOp::OpFMul
                | SpirVOp::OpUDiv
                | SpirVOp::OpSDiv
                | SpirVOp::OpFDiv
                | SpirVOp::OpPhi => Some(self.operands[0]),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl<'a> core::fmt::Debug for SpirVInstruction<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpirVInstruction")
            .field("opcode", &self.opcode)
            .field("word_count", &self.word_count)
            .field("operands_len", &self.operands.len())
            .finish()
    }
}

// ============================================================================
// Zero-Copy Instruction Iterator
// ============================================================================

/// Zero-copy iterator over SPIR-V instructions
///
/// # Usage
/// ```ignore
/// let module: &[u32] = ...; // SPIR-V words
/// let iter = SpirVInstructionIterator::new(module);
/// for instruction in iter {
///     match instruction.opcode {
///         SpirVOp::OpTypeFloat => println!("Found float type"),
///         _ => {}
///     }
/// }
/// ```
#[derive(Clone)]
pub struct SpirVInstructionIterator<'a> {
    /// The SPIR-V word data (starting after header)
    data: &'a [u32],
    /// Current offset in words
    offset: usize,
}

impl<'a> SpirVInstructionIterator<'a> {
    /// Create a new iterator from SPIR-V words (including header)
    ///
    /// Automatically skips the 5-word header.
    #[inline]
    pub fn new(words: &'a [u32]) -> Self {
        let data = if words.len() > SPIRV_HEADER_SIZE_WORDS {
            &words[SPIRV_HEADER_SIZE_WORDS..]
        } else {
            &[]
        };
        Self { data, offset: 0 }
    }

    /// Create iterator from instruction section only (no header)
    #[inline]
    pub fn from_instructions(instructions: &'a [u32]) -> Self {
        Self {
            data: instructions,
            offset: 0,
        }
    }

    /// Get current byte offset (for error reporting)
    #[inline]
    pub fn current_offset(&self) -> usize {
        (SPIRV_HEADER_SIZE_WORDS + self.offset) * 4
    }

    /// Get remaining word count
    #[inline]
    pub fn remaining_words(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }
}

impl<'a> Iterator for SpirVInstructionIterator<'a> {
    type Item = SpirVInstruction<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let header_word = self.data[self.offset];
        let raw_opcode = (header_word & 0xFFFF) as u16;
        let word_count = (header_word >> 16) as u16;

        // Validate word count
        // #ASSUME_INSTRUCTION_VALID: SPIR-V instructions have word_count >= 1
        if word_count < MIN_INSTRUCTION_WORDS {
            return None;
        }

        let word_count_usize = word_count as usize;
        if self.offset + word_count_usize > self.data.len() {
            return None; // Truncated instruction
        }

        let opcode = SpirVOp::from_raw(raw_opcode);
        let operands = if word_count_usize > 1 {
            &self.data[self.offset + 1..self.offset + word_count_usize]
        } else {
            &[]
        };

        self.offset += word_count_usize;

        Some(SpirVInstruction {
            opcode,
            word_count,
            raw_opcode,
            operands,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // Each instruction is at least 1 word
        let remaining = self.remaining_words();
        (0, Some(remaining))
    }
}

// ============================================================================
// Shader IR (Intermediate Representation)
// ============================================================================

/// Shader type for the intermediate representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShaderIrType {
    /// Void type
    Void = 0,
    /// Boolean type
    Bool = 1,
    /// 32-bit signed integer
    Int32 = 2,
    /// 32-bit unsigned integer
    Uint32 = 3,
    /// 64-bit signed integer
    Int64 = 4,
    /// 64-bit unsigned integer
    Uint64 = 5,
    /// 16-bit float (half)
    Float16 = 6,
    /// 32-bit float
    Float32 = 7,
    /// 64-bit float (double)
    Float64 = 8,
    /// Vector type
    Vector = 9,
    /// Matrix type
    Matrix = 10,
    /// Array type
    Array = 11,
    /// Struct type
    Struct = 12,
    /// Pointer type
    Pointer = 13,
    /// Function type
    Function = 14,
    /// Image/Texture type
    Image = 15,
    /// Sampler type
    Sampler = 16,
    /// Sampled image type
    SampledImage = 17,
}

/// Shader IR operation kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShaderIrOpKind {
    /// Type declaration
    TypeDecl = 0,
    /// Constant definition
    Constant = 1,
    /// Variable declaration
    Variable = 2,
    /// Function definition
    Function = 3,
    /// Load from memory
    Load = 4,
    /// Store to memory
    Store = 5,
    /// Access chain (pointer arithmetic)
    AccessChain = 6,
    /// Arithmetic operation
    Arithmetic = 7,
    /// Comparison operation
    Compare = 8,
    /// Control flow (branch, return, etc.)
    ControlFlow = 9,
    /// Texture/Image operation
    Texture = 10,
    /// Barrier operation
    Barrier = 11,
    /// Atomic operation
    Atomic = 12,
    /// Debug/annotation
    Debug = 13,
    /// Decoration
    Decoration = 14,
    /// Unknown/unsupported
    Unknown = 255,
}

/// A single IR instruction
#[derive(Debug, Clone)]
pub struct ShaderIrInstruction {
    /// Operation kind
    pub kind: ShaderIrOpKind,
    /// Result ID (if any)
    pub result_id: Option<u32>,
    /// Result type ID (if any)
    pub result_type_id: Option<u32>,
    /// Operand IDs
    pub operand_ids: Vec<u32>,
    /// Original SPIR-V opcode
    pub spirv_opcode: SpirVOp,
}

/// Shader Intermediate Representation
///
/// A simplified representation of a SPIR-V shader suitable for
/// further compilation or analysis.
#[derive(Debug, Clone)]
pub struct ShaderIr {
    /// SPIR-V version
    pub version: (u8, u8),
    /// Maximum ID bound
    pub bound: u32,
    /// Entry point name (if found)
    pub entry_point: Option<Vec<u8>>,
    /// Execution model
    pub execution_model: Option<u32>,
    /// All IR instructions
    pub instructions: Vec<ShaderIrInstruction>,
    /// Type declarations (id -> type)
    pub types: Vec<(u32, ShaderIrType)>,
    /// Constants (id -> value words)
    pub constants: Vec<(u32, Vec<u32>)>,
}

impl ShaderIr {
    /// Create empty IR
    pub fn new() -> Self {
        Self {
            version: (1, 0),
            bound: 0,
            entry_point: None,
            execution_model: None,
            instructions: Vec::new(),
            types: Vec::new(),
            constants: Vec::new(),
        }
    }
}

impl Default for ShaderIr {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPIR-V to IR Converter
// ============================================================================

/// Converter from SPIR-V to ShaderIr
pub struct SpirVToIrConverter {
    /// Collected IR
    ir: ShaderIr,
}

impl SpirVToIrConverter {
    /// Create a new converter
    pub fn new() -> Self {
        Self {
            ir: ShaderIr::new(),
        }
    }

    /// Convert SPIR-V module to ShaderIr
    pub fn convert(mut self, words: &[u32]) -> Result<ShaderIr, SpirVParseError> {
        // Parse header
        let header = SpirVHeader::from_words(words)
            .ok_or(SpirVParseError::InvalidHeader)?;

        if !header.is_valid() {
            return Err(SpirVParseError::InvalidMagic);
        }

        self.ir.version = header.version_tuple();
        self.ir.bound = header.bound;

        // Iterate instructions
        let iter = SpirVInstructionIterator::new(words);
        for instr in iter {
            self.process_instruction(&instr)?;
        }

        Ok(self.ir)
    }

    /// Process a single SPIR-V instruction
    fn process_instruction(&mut self, instr: &SpirVInstruction<'_>) -> Result<(), SpirVParseError> {
        let kind = Self::classify_opcode(&instr.opcode);

        let ir_instr = ShaderIrInstruction {
            kind,
            result_id: instr.result_id(),
            result_type_id: instr.result_type_id(),
            operand_ids: instr.operands.to_vec(),
            spirv_opcode: instr.opcode,
        };

        // Extract type information
        if instr.opcode.is_type() {
            if let Some(result_id) = ir_instr.result_id {
                let ir_type = Self::spirv_to_ir_type(&instr.opcode);
                self.ir.types.push((result_id, ir_type));
            }
        }

        // Extract constants
        if instr.opcode.is_constant() {
            if let Some(result_id) = ir_instr.result_id {
                // Constants have their value in operands after type and result
                let value = if instr.operands.len() > 2 {
                    instr.operands[2..].to_vec()
                } else {
                    Vec::new()
                };
                self.ir.constants.push((result_id, value));
            }
        }

        // Extract entry point
        if matches!(instr.opcode, SpirVOp::OpEntryPoint) && !instr.operands.is_empty() {
            self.ir.execution_model = Some(instr.operands[0]);
            // Entry point name starts at operand 2, as literal string
            if instr.operands.len() > 2 {
                let name_words = &instr.operands[2..];
                let mut name_bytes = Vec::new();
                for &word in name_words {
                    let bytes = word.to_le_bytes();
                    for &b in &bytes {
                        if b == 0 {
                            break;
                        }
                        name_bytes.push(b);
                    }
                }
                self.ir.entry_point = Some(name_bytes);
            }
        }

        self.ir.instructions.push(ir_instr);
        Ok(())
    }

    /// Classify SPIR-V opcode to IR operation kind
    fn classify_opcode(op: &SpirVOp) -> ShaderIrOpKind {
        if op.is_type() {
            ShaderIrOpKind::TypeDecl
        } else if op.is_constant() {
            ShaderIrOpKind::Constant
        } else if op.is_memory() {
            match op {
                SpirVOp::OpVariable => ShaderIrOpKind::Variable,
                SpirVOp::OpLoad => ShaderIrOpKind::Load,
                SpirVOp::OpStore => ShaderIrOpKind::Store,
                SpirVOp::OpAccessChain | SpirVOp::OpInBoundsAccessChain => ShaderIrOpKind::AccessChain,
                _ => ShaderIrOpKind::Unknown,
            }
        } else if op.is_arithmetic() {
            ShaderIrOpKind::Arithmetic
        } else if op.is_control_flow() {
            ShaderIrOpKind::ControlFlow
        } else {
            match op {
                SpirVOp::OpFunction | SpirVOp::OpFunctionEnd | SpirVOp::OpFunctionParameter => {
                    ShaderIrOpKind::Function
                }
                SpirVOp::OpDecorate | SpirVOp::OpMemberDecorate => ShaderIrOpKind::Decoration,
                SpirVOp::OpName | SpirVOp::OpMemberName | SpirVOp::OpString | SpirVOp::OpLine => {
                    ShaderIrOpKind::Debug
                }
                SpirVOp::OpSampledImage
                | SpirVOp::OpImageSampleImplicitLod
                | SpirVOp::OpImageSampleExplicitLod
                | SpirVOp::OpImageFetch
                | SpirVOp::OpImageRead
                | SpirVOp::OpImageWrite => ShaderIrOpKind::Texture,
                SpirVOp::OpControlBarrier | SpirVOp::OpMemoryBarrier => ShaderIrOpKind::Barrier,
                SpirVOp::OpAtomicLoad
                | SpirVOp::OpAtomicStore
                | SpirVOp::OpAtomicExchange
                | SpirVOp::OpAtomicCompareExchange
                | SpirVOp::OpAtomicIAdd
                | SpirVOp::OpAtomicISub => ShaderIrOpKind::Atomic,
                _ => ShaderIrOpKind::Unknown,
            }
        }
    }

    /// Convert SPIR-V type opcode to IR type
    fn spirv_to_ir_type(op: &SpirVOp) -> ShaderIrType {
        match op {
            SpirVOp::OpTypeVoid => ShaderIrType::Void,
            SpirVOp::OpTypeBool => ShaderIrType::Bool,
            SpirVOp::OpTypeInt => ShaderIrType::Int32, // Simplified
            SpirVOp::OpTypeFloat => ShaderIrType::Float32, // Simplified
            SpirVOp::OpTypeVector => ShaderIrType::Vector,
            SpirVOp::OpTypeMatrix => ShaderIrType::Matrix,
            SpirVOp::OpTypeArray | SpirVOp::OpTypeRuntimeArray => ShaderIrType::Array,
            SpirVOp::OpTypeStruct => ShaderIrType::Struct,
            SpirVOp::OpTypePointer => ShaderIrType::Pointer,
            SpirVOp::OpTypeFunction => ShaderIrType::Function,
            SpirVOp::OpTypeImage => ShaderIrType::Image,
            SpirVOp::OpTypeSampler => ShaderIrType::Sampler,
            SpirVOp::OpTypeSampledImage => ShaderIrType::SampledImage,
            _ => ShaderIrType::Void,
        }
    }
}

impl Default for SpirVToIrConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Parse Error
// ============================================================================

/// SPIR-V parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpirVParseError {
    /// Invalid or missing magic number
    InvalidMagic = 0,
    /// Header too short
    InvalidHeader = 1,
    /// Module too short
    ModuleTooSmall = 2,
    /// Invalid instruction word count
    InvalidWordCount = 3,
    /// Truncated instruction
    TruncatedInstruction = 4,
    /// Unsupported SPIR-V version
    UnsupportedVersion = 5,
    /// Invalid schema (must be 0)
    InvalidSchema = 6,
    /// Big-endian SPIR-V (not supported)
    BigEndianNotSupported = 7,
    /// Parser state error
    InvalidState = 8,
}

impl core::fmt::Display for SpirVParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid SPIR-V magic number"),
            Self::InvalidHeader => write!(f, "Invalid SPIR-V header"),
            Self::ModuleTooSmall => write!(f, "SPIR-V module too small"),
            Self::InvalidWordCount => write!(f, "Invalid instruction word count"),
            Self::TruncatedInstruction => write!(f, "Truncated SPIR-V instruction"),
            Self::UnsupportedVersion => write!(f, "Unsupported SPIR-V version"),
            Self::InvalidSchema => write!(f, "Invalid SPIR-V schema"),
            Self::BigEndianNotSupported => write!(f, "Big-endian SPIR-V not supported"),
            Self::InvalidState => write!(f, "Invalid parser state"),
        }
    }
}

// ============================================================================
// Parser State
// ============================================================================

/// Parser operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ParserState {
    /// Parser is idle/ready
    #[default]
    Idle = 0,
    /// Parser is processing header
    ParsingHeader = 1,
    /// Parser is processing instructions
    ParsingInstructions = 2,
    /// Parsing completed successfully
    Complete = 3,
    /// Parser encountered an error
    Error = 4,
}

impl ParserState {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Idle),
            1 => Some(Self::ParsingHeader),
            2 => Some(Self::ParsingInstructions),
            3 => Some(Self::Complete),
            4 => Some(Self::Error),
            _ => None,
        }
    }
}

// ============================================================================
// Bit Packing Constants
// ============================================================================

/// Primary packing: state(8) | instruction_count(24) | generation(32)
const PRIMARY_STATE_SHIFT: u32 = 56;
const PRIMARY_STATE_MASK: u64 = 0xFF << PRIMARY_STATE_SHIFT;
const PRIMARY_COUNT_SHIFT: u32 = 32;
const PRIMARY_COUNT_MASK: u64 = 0xFF_FFFF << PRIMARY_COUNT_SHIFT;
const PRIMARY_GEN_MASK: u64 = 0xFFFF_FFFF; // Lower 32 bits

/// Secondary packing: error_count(16) | current_offset(48)
const SECONDARY_ERROR_SHIFT: u32 = 48;
const SECONDARY_ERROR_MASK: u64 = 0xFFFF << SECONDARY_ERROR_SHIFT;
const SECONDARY_OFFSET_MASK: u64 = 0xFFFF_FFFF_FFFF; // Lower 48 bits

// ============================================================================
// SpirVParserCapsule
// ============================================================================

/// SPIR-V Parser Capsule (T1 Atomic, 256B aligned)
///
/// A lockfree, cache-aligned SPIR-V parser with atomic state tracking.
///
/// # Features
/// - Zero-copy instruction iteration
/// - Atomic state coordination
/// - Generation counter for ABA prevention
/// - Statistics tracking
///
/// # Thread Safety
/// All operations are atomic and lockfree. Safe for concurrent access.
#[repr(C, align(256))]
pub struct SpirVParserCapsule {
    /// Primary coordination word
    /// - Bits 63-56: ParserState (8 bits)
    /// - Bits 55-32: instruction_count (24 bits)
    /// - Bits 31-0: generation (32 bits)
    primary: AtomicU64,

    /// Secondary coordination word
    /// - Bits 63-48: error_count (16 bits)
    /// - Bits 47-0: current_offset (48 bits)
    secondary: AtomicU64,

    /// Pointer to SPIR-V module data
    module_ptr: AtomicPtr<u32>,

    /// Size of module in words
    module_size: AtomicU64,

    /// ID bound from header
    bound: AtomicU64,

    /// SPIR-V version from header
    version: AtomicU64,

    /// Padding to 256B
    _padding: [u8; 200],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<SpirVParserCapsule>() == 256);
    assert!(core::mem::align_of::<SpirVParserCapsule>() == 256);
};

impl SpirVParserCapsule {
    /// Create a new parser capsule
    pub const fn new() -> Self {
        // Pack initial primary: Idle state, 0 instructions, generation 1
        let primary_packed = ((ParserState::Idle as u64) << PRIMARY_STATE_SHIFT) | 1;

        Self {
            primary: AtomicU64::new(primary_packed),
            secondary: AtomicU64::new(0),
            module_ptr: AtomicPtr::new(core::ptr::null_mut()),
            module_size: AtomicU64::new(0),
            bound: AtomicU64::new(0),
            version: AtomicU64::new(0),
            _padding: [0u8; 200],
        }
    }

    /// Get current parser state
    #[inline]
    pub fn state(&self) -> ParserState {
        let packed = self.primary.load(Ordering::Acquire);
        let state_byte = ((packed & PRIMARY_STATE_MASK) >> PRIMARY_STATE_SHIFT) as u8;
        ParserState::from_u8(state_byte).unwrap_or(ParserState::Idle)
    }

    /// Get instruction count
    #[inline]
    pub fn instruction_count(&self) -> u32 {
        let packed = self.primary.load(Ordering::Acquire);
        ((packed & PRIMARY_COUNT_MASK) >> PRIMARY_COUNT_SHIFT) as u32
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u32 {
        let packed = self.primary.load(Ordering::Acquire);
        (packed & PRIMARY_GEN_MASK) as u32
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u16 {
        let packed = self.secondary.load(Ordering::Acquire);
        ((packed & SECONDARY_ERROR_MASK) >> SECONDARY_ERROR_SHIFT) as u16
    }

    /// Get current parsing offset (words)
    #[inline]
    pub fn current_offset(&self) -> u64 {
        let packed = self.secondary.load(Ordering::Acquire);
        packed & SECONDARY_OFFSET_MASK
    }

    /// Get module bound (max ID + 1)
    #[inline]
    pub fn bound(&self) -> u32 {
        self.bound.load(Ordering::Acquire) as u32
    }

    /// Get SPIR-V version as (major, minor)
    #[inline]
    pub fn version(&self) -> (u8, u8) {
        let v = self.version.load(Ordering::Acquire) as u32;
        let major = ((v >> 16) & 0xFF) as u8;
        let minor = ((v >> 8) & 0xFF) as u8;
        (major, minor)
    }

    /// Parse a SPIR-V module
    ///
    /// # Arguments
    /// * `words` - SPIR-V module as u32 words
    ///
    /// # Returns
    /// Number of instructions parsed, or error
    pub fn parse(&self, words: &[u32]) -> Result<u32, SpirVParseError> {
        // Validate size
        if words.len() < SPIRV_HEADER_SIZE_WORDS {
            self.set_state(ParserState::Error);
            self.increment_error_count();
            return Err(SpirVParseError::ModuleTooSmall);
        }

        // Set parsing state
        self.set_state(ParserState::ParsingHeader);

        // Parse header
        let header = SpirVHeader::from_words(words)
            .ok_or_else(|| {
                self.set_state(ParserState::Error);
                self.increment_error_count();
                SpirVParseError::InvalidHeader
            })?;

        if !header.is_valid() {
            self.set_state(ParserState::Error);
            self.increment_error_count();
            return Err(SpirVParseError::InvalidMagic);
        }

        if header.is_big_endian() {
            self.set_state(ParserState::Error);
            self.increment_error_count();
            return Err(SpirVParseError::BigEndianNotSupported);
        }

        // Store header info
        self.bound.store(header.bound as u64, Ordering::Release);
        self.version.store(header.version as u64, Ordering::Release);
        self.module_size.store(words.len() as u64, Ordering::Release);

        // Count instructions
        self.set_state(ParserState::ParsingInstructions);
        let mut count = 0u32;
        let iter = SpirVInstructionIterator::new(words);

        for instr in iter {
            count += 1;
            self.set_instruction_count(count);

            // Validate instruction
            if instr.word_count < MIN_INSTRUCTION_WORDS {
                self.set_state(ParserState::Error);
                self.increment_error_count();
                return Err(SpirVParseError::InvalidWordCount);
            }
        }

        self.set_state(ParserState::Complete);
        self.increment_generation();

        Ok(count)
    }

    /// Validate a SPIR-V module without full parsing
    ///
    /// Returns true if the module appears valid.
    #[inline]
    pub fn validate(&self, words: &[u32]) -> bool {
        if words.len() < SPIRV_HEADER_SIZE_WORDS {
            return false;
        }

        if let Some(header) = SpirVHeader::from_words(words) {
            header.is_valid()
        } else {
            false
        }
    }

    /// Get an instruction iterator for a module
    #[inline]
    pub fn iterate<'a>(&self, words: &'a [u32]) -> SpirVInstructionIterator<'a> {
        SpirVInstructionIterator::new(words)
    }

    /// Convert SPIR-V to ShaderIR
    pub fn to_ir(&self, words: &[u32]) -> Result<ShaderIr, SpirVParseError> {
        let converter = SpirVToIrConverter::new();
        converter.convert(words)
    }

    /// Reset parser state
    pub fn reset(&self) {
        self.set_state(ParserState::Idle);
        self.set_instruction_count(0);
        self.secondary.store(0, Ordering::Release);
        self.module_ptr.store(core::ptr::null_mut(), Ordering::Release);
        self.module_size.store(0, Ordering::Release);
        self.bound.store(0, Ordering::Release);
        self.version.store(0, Ordering::Release);
        self.increment_generation();
    }

    /// Get parser snapshot
    pub fn snapshot(&self) -> SpirVParserSnapshot {
        SpirVParserSnapshot {
            state: self.state(),
            instruction_count: self.instruction_count(),
            error_count: self.error_count(),
            generation: self.generation(),
            bound: self.bound(),
            version: self.version(),
        }
    }

    // === Internal helpers ===

    #[inline]
    fn set_state(&self, new_state: ParserState) {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let new_packed = (current & !PRIMARY_STATE_MASK)
                | ((new_state as u64) << PRIMARY_STATE_SHIFT);

            if self
                .primary
                .compare_exchange_weak(current, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    #[inline]
    fn set_instruction_count(&self, count: u32) {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let new_packed = (current & !PRIMARY_COUNT_MASK)
                | ((count as u64 & 0xFF_FFFF) << PRIMARY_COUNT_SHIFT);

            if self
                .primary
                .compare_exchange_weak(current, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    #[inline]
    fn increment_generation(&self) {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = ((current & PRIMARY_GEN_MASK) as u32).wrapping_add(1);
            let new_packed = (current & !PRIMARY_GEN_MASK) | (gen as u64);

            if self
                .primary
                .compare_exchange_weak(current, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    #[inline]
    fn increment_error_count(&self) {
        loop {
            let current = self.secondary.load(Ordering::Acquire);
            let count = ((current & SECONDARY_ERROR_MASK) >> SECONDARY_ERROR_SHIFT) as u16;
            let new_count = count.saturating_add(1);
            let new_packed = (current & !SECONDARY_ERROR_MASK)
                | ((new_count as u64) << SECONDARY_ERROR_SHIFT);

            if self
                .secondary
                .compare_exchange_weak(current, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }
}

impl Default for SpirVParserCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Thread safety - Chaos mandate
// SAFETY: All fields are atomic. No raw mutable pointers shared across threads.
// #ASSUME_ATOMIC_THREAD_SAFE: AtomicU64/AtomicPtr are thread-safe by definition.
unsafe impl Send for SpirVParserCapsule {}
unsafe impl Sync for SpirVParserCapsule {}

impl core::fmt::Debug for SpirVParserCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("SpirVParserCapsule")
            .field("state", &snapshot.state)
            .field("instruction_count", &snapshot.instruction_count)
            .field("error_count", &snapshot.error_count)
            .field("generation", &snapshot.generation)
            .finish()
    }
}

// ============================================================================
// Parser Snapshot
// ============================================================================

/// Immutable snapshot of parser state
#[derive(Debug, Clone, Copy)]
pub struct SpirVParserSnapshot {
    /// Current state
    pub state: ParserState,
    /// Number of instructions parsed
    pub instruction_count: u32,
    /// Number of errors encountered
    pub error_count: u16,
    /// Generation counter
    pub generation: u32,
    /// ID bound
    pub bound: u32,
    /// SPIR-V version (major, minor)
    pub version: (u8, u8),
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Validate SPIR-V magic number from bytes
#[inline]
pub fn validate_spirv_magic(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    magic == SPIRV_MAGIC
}

/// Validate SPIR-V magic number from words
#[inline]
pub fn validate_spirv_magic_words(words: &[u32]) -> bool {
    !words.is_empty() && words[0] == SPIRV_MAGIC
}

/// Check if SPIR-V module is big-endian (needs byte swap)
#[inline]
pub fn is_spirv_big_endian(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    magic == SPIRV_MAGIC
}

/// Extract SPIR-V version from raw bytes
#[inline]
pub fn extract_spirv_version(data: &[u8]) -> Option<(u8, u8)> {
    if data.len() < 8 {
        return None;
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let major = ((version >> 16) & 0xFF) as u8;
    let minor = ((version >> 8) & 0xFF) as u8;
    Some((major, minor))
}

/// Count instructions in a SPIR-V module
pub fn count_spirv_instructions(words: &[u32]) -> usize {
    SpirVInstructionIterator::new(words).count()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Test Data Helpers
    // ========================================================================

    /// Create a minimal valid SPIR-V module header
    fn make_header() -> [u32; 5] {
        [
            SPIRV_MAGIC,      // magic
            0x00010500,       // version 1.5
            0x00000000,       // generator
            0x00000010,       // bound = 16
            0x00000000,       // schema = 0
        ]
    }

    /// Create a simple SPIR-V module with some instructions
    fn make_simple_module() -> Vec<u32> {
        let mut words = make_header().to_vec();

        // OpCapability Shader (17 | 2<<16 = 0x00020011)
        words.push(0x00020011);
        words.push(1); // Shader capability

        // OpMemoryModel Logical GLSL450 (14 | 3<<16)
        words.push(0x00030013);
        words.push(0); // Logical
        words.push(1); // GLSL450

        // OpTypeVoid %1 (19 | 2<<16)
        words.push(0x00020013);
        words.push(1); // Result ID

        // OpTypeFunction %2 %1 (33 | 3<<16)
        words.push(0x00030021);
        words.push(2); // Result ID
        words.push(1); // Return type

        words
    }

    // ========================================================================
    // SPIR-V Magic Tests
    // ========================================================================

    #[test]
    fn test_spirv_magic_constant() {
        assert_eq!(SPIRV_MAGIC, 0x07230203);
    }

    #[test]
    fn test_spirv_magic_le_bytes() {
        let magic = u32::from_le_bytes(SPIRV_MAGIC_LE);
        assert_eq!(magic, SPIRV_MAGIC);
    }

    #[test]
    fn test_spirv_magic_be_bytes() {
        let magic = u32::from_be_bytes(SPIRV_MAGIC_BE);
        assert_eq!(magic, SPIRV_MAGIC);
    }

    #[test]
    fn test_validate_spirv_magic_valid() {
        let data = SPIRV_MAGIC_LE;
        assert!(validate_spirv_magic(&data));
    }

    #[test]
    fn test_validate_spirv_magic_invalid() {
        let data = [0xFF, 0xFF, 0xFF, 0xFF];
        assert!(!validate_spirv_magic(&data));
    }

    #[test]
    fn test_validate_spirv_magic_too_short() {
        let data = [0x03, 0x02];
        assert!(!validate_spirv_magic(&data));
    }

    #[test]
    fn test_validate_spirv_magic_words() {
        let words = [SPIRV_MAGIC];
        assert!(validate_spirv_magic_words(&words));
    }

    #[test]
    fn test_is_spirv_big_endian() {
        let le_data = SPIRV_MAGIC_LE;
        let be_data = SPIRV_MAGIC_BE;
        assert!(!is_spirv_big_endian(&le_data));
        assert!(is_spirv_big_endian(&be_data));
    }

    // ========================================================================
    // SPIR-V Header Tests
    // ========================================================================

    #[test]
    fn test_header_from_words() {
        let words = make_header();
        let header = SpirVHeader::from_words(&words).unwrap();
        assert_eq!(header.magic, SPIRV_MAGIC);
        assert!(header.is_valid());
    }

    #[test]
    fn test_header_from_bytes() {
        let mut bytes = [0u8; 20];
        bytes[0..4].copy_from_slice(&SPIRV_MAGIC_LE);
        bytes[4..8].copy_from_slice(&0x00010500u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&16u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&0u32.to_le_bytes());

        let header = SpirVHeader::from_bytes(&bytes).unwrap();
        assert!(header.is_valid());
    }

    #[test]
    fn test_header_invalid_magic() {
        let words = [0xFFFFFFFF, 0, 0, 0, 0];
        let header = SpirVHeader::from_words(&words).unwrap();
        assert!(!header.is_valid());
    }

    #[test]
    fn test_header_invalid_schema() {
        let mut words = make_header();
        words[4] = 1; // Invalid schema
        let header = SpirVHeader::from_words(&words).unwrap();
        assert!(!header.is_valid());
    }

    #[test]
    fn test_header_version_tuple() {
        let header = SpirVHeader {
            magic: SPIRV_MAGIC,
            version: 0x00010500, // 1.5
            generator: 0,
            bound: 0,
            schema: 0,
        };
        assert_eq!(header.version_tuple(), (1, 5));
    }

    #[test]
    fn test_header_too_short() {
        let words = [SPIRV_MAGIC];
        assert!(SpirVHeader::from_words(&words).is_none());
    }

    // ========================================================================
    // SpirVOp Tests
    // ========================================================================

    #[test]
    fn test_opcode_from_raw() {
        assert_eq!(SpirVOp::from_raw(0), SpirVOp::OpNop);
        assert_eq!(SpirVOp::from_raw(19), SpirVOp::OpTypeVoid);
        assert_eq!(SpirVOp::from_raw(43), SpirVOp::OpConstant);
        assert_eq!(SpirVOp::from_raw(61), SpirVOp::OpLoad);
        assert_eq!(SpirVOp::from_raw(129), SpirVOp::OpFAdd);
        assert_eq!(SpirVOp::from_raw(248), SpirVOp::OpLabel);
        assert_eq!(SpirVOp::from_raw(9999), SpirVOp::Unknown);
    }

    #[test]
    fn test_opcode_raw_roundtrip() {
        let opcodes = [
            SpirVOp::OpNop,
            SpirVOp::OpTypeVoid,
            SpirVOp::OpTypeFloat,
            SpirVOp::OpConstant,
            SpirVOp::OpLoad,
            SpirVOp::OpStore,
            SpirVOp::OpFAdd,
            SpirVOp::OpLabel,
            SpirVOp::OpReturn,
        ];
        for op in opcodes {
            let raw = op.raw();
            let back = SpirVOp::from_raw(raw);
            assert_eq!(op, back);
        }
    }

    #[test]
    fn test_opcode_is_type() {
        assert!(SpirVOp::OpTypeVoid.is_type());
        assert!(SpirVOp::OpTypeFloat.is_type());
        assert!(SpirVOp::OpTypeVector.is_type());
        assert!(SpirVOp::OpTypePointer.is_type());
        assert!(!SpirVOp::OpLoad.is_type());
        assert!(!SpirVOp::OpFAdd.is_type());
    }

    #[test]
    fn test_opcode_is_constant() {
        assert!(SpirVOp::OpConstant.is_constant());
        assert!(SpirVOp::OpConstantComposite.is_constant());
        assert!(SpirVOp::OpConstantTrue.is_constant());
        assert!(!SpirVOp::OpTypeVoid.is_constant());
        assert!(!SpirVOp::OpLoad.is_constant());
    }

    #[test]
    fn test_opcode_is_control_flow() {
        assert!(SpirVOp::OpLabel.is_control_flow());
        assert!(SpirVOp::OpBranch.is_control_flow());
        assert!(SpirVOp::OpReturn.is_control_flow());
        assert!(SpirVOp::OpPhi.is_control_flow());
        assert!(!SpirVOp::OpLoad.is_control_flow());
    }

    #[test]
    fn test_opcode_is_arithmetic() {
        assert!(SpirVOp::OpFAdd.is_arithmetic());
        assert!(SpirVOp::OpFMul.is_arithmetic());
        assert!(SpirVOp::OpIAdd.is_arithmetic());
        assert!(SpirVOp::OpDot.is_arithmetic());
        assert!(!SpirVOp::OpLoad.is_arithmetic());
    }

    #[test]
    fn test_opcode_is_memory() {
        assert!(SpirVOp::OpLoad.is_memory());
        assert!(SpirVOp::OpStore.is_memory());
        assert!(SpirVOp::OpVariable.is_memory());
        assert!(SpirVOp::OpAccessChain.is_memory());
        assert!(!SpirVOp::OpFAdd.is_memory());
    }

    // ========================================================================
    // Instruction Iterator Tests
    // ========================================================================

    #[test]
    fn test_iterator_empty() {
        let iter = SpirVInstructionIterator::new(&[]);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn test_iterator_header_only() {
        let words = make_header();
        let iter = SpirVInstructionIterator::new(&words);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn test_iterator_simple_module() {
        let words = make_simple_module();
        let iter = SpirVInstructionIterator::new(&words);
        let count = iter.count();
        assert!(count > 0, "Should have instructions");
    }

    #[test]
    fn test_iterator_parses_opcodes() {
        let words = make_simple_module();
        let iter = SpirVInstructionIterator::new(&words);

        let opcodes: Vec<SpirVOp> = iter.map(|i| i.opcode).collect();
        assert!(opcodes.contains(&SpirVOp::OpCapability));
        assert!(opcodes.contains(&SpirVOp::OpTypeVoid));
    }

    #[test]
    fn test_iterator_word_count() {
        let mut words = make_header().to_vec();
        // Add OpNop (1 word)
        words.push(0x00010000);

        let iter = SpirVInstructionIterator::new(&words);
        let instr = iter.into_iter().next().unwrap();
        assert_eq!(instr.word_count, 1);
        assert_eq!(instr.opcode, SpirVOp::OpNop);
    }

    #[test]
    fn test_iterator_operands() {
        let mut words = make_header().to_vec();
        // OpCapability Shader (2 words)
        words.push(0x00020011);
        words.push(1); // Shader = 1

        let iter = SpirVInstructionIterator::new(&words);
        let instr = iter.into_iter().next().unwrap();
        assert_eq!(instr.operands.len(), 1);
        assert_eq!(instr.operands[0], 1);
    }

    #[test]
    fn test_iterator_remaining_words() {
        let words = make_simple_module();
        let iter = SpirVInstructionIterator::new(&words);
        let remaining = iter.remaining_words();
        assert!(remaining > 0);
    }

    // ========================================================================
    // SpirVParserCapsule Tests
    // ========================================================================

    #[test]
    fn test_parser_new() {
        let parser = SpirVParserCapsule::new();
        assert_eq!(parser.state(), ParserState::Idle);
        assert_eq!(parser.instruction_count(), 0);
        assert_eq!(parser.generation(), 1);
        assert_eq!(parser.error_count(), 0);
    }

    #[test]
    fn test_parser_default() {
        let parser = SpirVParserCapsule::default();
        assert_eq!(parser.state(), ParserState::Idle);
    }

    #[test]
    fn test_parser_size() {
        assert_eq!(core::mem::size_of::<SpirVParserCapsule>(), 256);
    }

    #[test]
    fn test_parser_alignment() {
        assert_eq!(core::mem::align_of::<SpirVParserCapsule>(), 256);
    }

    #[test]
    fn test_parser_validate_valid() {
        let parser = SpirVParserCapsule::new();
        let words = make_header();
        assert!(parser.validate(&words));
    }

    #[test]
    fn test_parser_validate_invalid() {
        let parser = SpirVParserCapsule::new();
        let words = [0xFFFFFFFF, 0, 0, 0, 0];
        assert!(!parser.validate(&words));
    }

    #[test]
    fn test_parser_validate_too_short() {
        let parser = SpirVParserCapsule::new();
        let words = [SPIRV_MAGIC];
        assert!(!parser.validate(&words));
    }

    #[test]
    fn test_parser_parse_valid() {
        let parser = SpirVParserCapsule::new();
        let words = make_simple_module();
        let result = parser.parse(&words);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
        assert_eq!(parser.state(), ParserState::Complete);
    }

    #[test]
    fn test_parser_parse_invalid_magic() {
        let parser = SpirVParserCapsule::new();
        let words = [0xFFFFFFFF, 0, 0, 0, 0];
        let result = parser.parse(&words);
        assert_eq!(result, Err(SpirVParseError::InvalidMagic));
        assert_eq!(parser.state(), ParserState::Error);
        assert!(parser.error_count() > 0);
    }

    #[test]
    fn test_parser_parse_too_short() {
        let parser = SpirVParserCapsule::new();
        let words: [u32; 2] = [SPIRV_MAGIC, 0];
        let result = parser.parse(&words);
        assert_eq!(result, Err(SpirVParseError::ModuleTooSmall));
    }

    #[test]
    fn test_parser_reset() {
        let parser = SpirVParserCapsule::new();
        let words = make_simple_module();
        parser.parse(&words).unwrap();

        let gen_before = parser.generation();
        parser.reset();

        assert_eq!(parser.state(), ParserState::Idle);
        assert_eq!(parser.instruction_count(), 0);
        assert!(parser.generation() > gen_before);
    }

    #[test]
    fn test_parser_snapshot() {
        let parser = SpirVParserCapsule::new();
        let words = make_simple_module();
        parser.parse(&words).unwrap();

        let snapshot = parser.snapshot();
        assert_eq!(snapshot.state, ParserState::Complete);
        assert!(snapshot.instruction_count > 0);
        assert_eq!(snapshot.bound, 16);
        assert_eq!(snapshot.version, (1, 5));
    }

    #[test]
    fn test_parser_iterate() {
        let parser = SpirVParserCapsule::new();
        let words = make_simple_module();
        let iter = parser.iterate(&words);
        let count = iter.count();
        assert!(count > 0);
    }

    // ========================================================================
    // IR Conversion Tests
    // ========================================================================

    #[test]
    fn test_converter_empty() {
        let converter = SpirVToIrConverter::new();
        let words = make_header();
        let ir = converter.convert(&words).unwrap();
        assert_eq!(ir.version, (1, 5));
        assert_eq!(ir.bound, 16);
    }

    #[test]
    fn test_converter_simple_module() {
        let converter = SpirVToIrConverter::new();
        let words = make_simple_module();
        let ir = converter.convert(&words).unwrap();

        assert!(!ir.instructions.is_empty());
        assert!(!ir.types.is_empty());
    }

    #[test]
    fn test_converter_invalid_magic() {
        let converter = SpirVToIrConverter::new();
        let words = [0xFFFFFFFF, 0, 0, 0, 0];
        let result = converter.convert(&words);
        assert!(result.is_err());
    }

    #[test]
    fn test_shader_ir_default() {
        let ir = ShaderIr::default();
        assert_eq!(ir.version, (1, 0));
        assert_eq!(ir.bound, 0);
        assert!(ir.instructions.is_empty());
    }

    // ========================================================================
    // Parser State Tests
    // ========================================================================

    #[test]
    fn test_parser_state_values() {
        assert_eq!(ParserState::Idle as u8, 0);
        assert_eq!(ParserState::ParsingHeader as u8, 1);
        assert_eq!(ParserState::ParsingInstructions as u8, 2);
        assert_eq!(ParserState::Complete as u8, 3);
        assert_eq!(ParserState::Error as u8, 4);
    }

    #[test]
    fn test_parser_state_from_u8() {
        assert_eq!(ParserState::from_u8(0), Some(ParserState::Idle));
        assert_eq!(ParserState::from_u8(3), Some(ParserState::Complete));
        assert_eq!(ParserState::from_u8(99), None);
    }

    // ========================================================================
    // Error Tests
    // ========================================================================

    #[test]
    fn test_error_display() {
        let error = SpirVParseError::InvalidMagic;
        let msg = format!("{}", error);
        assert!(msg.contains("magic"));
    }

    #[test]
    fn test_error_values() {
        assert_eq!(SpirVParseError::InvalidMagic as u8, 0);
        assert_eq!(SpirVParseError::InvalidHeader as u8, 1);
        assert_eq!(SpirVParseError::ModuleTooSmall as u8, 2);
    }

    // ========================================================================
    // Utility Function Tests
    // ========================================================================

    #[test]
    fn test_extract_spirv_version() {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&SPIRV_MAGIC_LE);
        bytes[4..8].copy_from_slice(&0x00010500u32.to_le_bytes());

        let version = extract_spirv_version(&bytes);
        assert_eq!(version, Some((1, 5)));
    }

    #[test]
    fn test_extract_spirv_version_too_short() {
        let bytes = [0u8; 4];
        assert!(extract_spirv_version(&bytes).is_none());
    }

    #[test]
    fn test_count_spirv_instructions() {
        let words = make_simple_module();
        let count = count_spirv_instructions(&words);
        assert!(count > 0);
    }

    // ========================================================================
    // Thread Safety Tests
    // ========================================================================

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SpirVParserCapsule>();
        assert_send_sync::<SpirVHeader>();
        assert_send_sync::<SpirVOp>();
        assert_send_sync::<ParserState>();
        assert_send_sync::<SpirVParseError>();
    }

    #[test]
    fn test_parser_debug() {
        let parser = SpirVParserCapsule::new();
        let debug_str = format!("{:?}", parser);
        assert!(debug_str.contains("SpirVParserCapsule"));
        assert!(debug_str.contains("state"));
    }

    // ========================================================================
    // Instruction Debug Tests
    // ========================================================================

    #[test]
    fn test_instruction_debug() {
        let words = make_simple_module();
        let iter = SpirVInstructionIterator::new(&words);
        let instr = iter.into_iter().next().unwrap();
        let debug_str = format!("{:?}", instr);
        assert!(debug_str.contains("SpirVInstruction"));
    }

    #[test]
    fn test_instruction_result_id() {
        let mut words = make_header().to_vec();
        // OpTypeVoid %1 (19 | 2<<16)
        words.push(0x00020013);
        words.push(1); // Result ID

        let iter = SpirVInstructionIterator::new(&words);
        let instr = iter.into_iter().next().unwrap();

        // OpTypeVoid doesn't have result_id in our simple implementation
        // as it doesn't match our result ID patterns
        assert_eq!(instr.opcode, SpirVOp::OpTypeVoid);
    }

    // ========================================================================
    // IR Type Tests
    // ========================================================================

    #[test]
    fn test_shader_ir_type_values() {
        assert_eq!(ShaderIrType::Void as u8, 0);
        assert_eq!(ShaderIrType::Float32 as u8, 7);
        assert_eq!(ShaderIrType::Vector as u8, 9);
    }

    #[test]
    fn test_shader_ir_op_kind_values() {
        assert_eq!(ShaderIrOpKind::TypeDecl as u8, 0);
        assert_eq!(ShaderIrOpKind::Load as u8, 4);
        assert_eq!(ShaderIrOpKind::Unknown as u8, 255);
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_iterator_truncated_instruction() {
        let mut words = make_header().to_vec();
        // Add instruction claiming 10 words but only provide 1
        words.push(0x000A0000);

        let iter = SpirVInstructionIterator::new(&words);
        let count = iter.count();
        assert_eq!(count, 0); // Should reject truncated instruction
    }

    #[test]
    fn test_iterator_zero_word_count() {
        let mut words = make_header().to_vec();
        // Add instruction with 0 word count (invalid)
        words.push(0x00000000);

        let iter = SpirVInstructionIterator::new(&words);
        let count = iter.count();
        assert_eq!(count, 0); // Should reject zero-word instruction
    }

    // ========================================================================
    // Concurrent Access Tests (std only)
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_parsing() {
        use std::sync::Arc;
        use std::thread;

        let parser = Arc::new(SpirVParserCapsule::new());
        let words = make_simple_module();

        let mut handles = vec![];

        for _ in 0..4 {
            let p = Arc::clone(&parser);
            let w = words.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _ = p.validate(&w);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Parser should still be usable
        let result = parser.parse(&words);
        assert!(result.is_ok());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_iteration() {
        use std::sync::Arc;
        use std::thread;

        let words = Arc::new(make_simple_module());
        let mut handles = vec![];

        for _ in 0..4 {
            let w = Arc::clone(&words);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let iter = SpirVInstructionIterator::new(&w);
                    let _ = iter.count();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
