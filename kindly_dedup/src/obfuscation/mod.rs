//! Code Obfuscation Primitives
//!
//! T2 (SIMD) + T3 (Fixed-Point) computational capsules for code obfuscation.
//!
//! ## Overview
//!
//! Provides deterministic, auditable instruction mutation for code hardening and
//! anti-reverse-engineering. Uses SIMD batch processing and Q16.16 fixed-point PRNG
//! for reproducibility and verifiability.
//!
//! ## Components
//!
//! - **InstructionSubstitutionCapsule**: SIMD-based instruction mutation (T2+T3)
//!
//! ## Features
//!
//! - **Deterministic mutations**: Same seed always produces same obfuscation
//! - **SIMD batch processing**: 16 opcodes in ~15ns (~1ns per opcode)
//! - **100% lockfree**: Pure atomic operations, no mutex/RwLock
//! - **Q34 auditable**: Hash-chained mutations, audit trail support
//!
//! ## Example
//!
//! ```rust
//! use kindly_dedup::obfuscation::InstructionSubstitutionCapsule;
//!
//! let capsule = InstructionSubstitutionCapsule::new(0xDEADBEEF);
//! capsule.activate();
//!
//! // Mutate instructions deterministically
//! let opcodes = vec![0x01, 0x29, 0x69];  // ADD, SUB, IMUL
//! let obfuscated = capsule.mutate_instructions(&opcodes);
//!
//! // Batch SIMD mutations (16 at once)
//! let batch = [0x01; 16];
//! let batch_obfuscated = capsule.apply_simd_mutations(&batch);
//! ```

pub mod code_encryption;
pub mod control_flow;
pub mod instruction_substitution;
pub mod simd_masking;

pub use code_encryption::{CodeEncryptionCapsule, EncryptionError, EncryptionResult};
pub use control_flow::ControlFlowObfuscationCapsule;
pub use instruction_substitution::InstructionSubstitutionCapsule;
pub use simd_masking::SimdMaskingCapsule;
