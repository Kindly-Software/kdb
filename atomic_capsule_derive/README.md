# atomic_capsule_derive

**Procedural macro for automatic computational capsule verification**

## Overview

`atomic_capsule_derive` provides `#[derive(ComputationalCapsule)]` which automatically generates:
- **Compile-time alignment verification** - Catches misaligned capsules at build time
- **Compile-time size verification** - Ensures struct layout matches expectations
- **Tier-specific validation** - Validates UCE33 capsule tier requirements
- **Send + Sync trait implementations** - Marks capsules as thread-safe

## Features

- ✅ **Zero runtime cost** - All verification at compile-time only
- ✅ **Clear error messages** - Actionable compile errors with span information
- ✅ **Minimal dependencies** - Only syn + quote + proc-macro2
- ✅ **Stable Rust** - No nightly features required
- ✅ **UCE33 compliant** - Validates all 10 capsule tiers

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
atomic_capsule_derive = { path = "../atomic_capsule_derive" }
```

## Usage

### Basic Example

```rust
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
```

### Attribute Parameters

- **`alignment`** (required): Cache line alignment in bytes (32/64/128/256)
- **`size`** (optional): Expected struct size in bytes (for layout verification)
- **`tier`** (optional): UCE33 capsule tier ("Atomic", "SIMD", etc.)

### SIMD Capsule Example

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, tier = "SIMD")]
#[repr(C, align(128))]
struct SimdVenueScorer {
    scores: [f64; 8],
    _padding: [u8; 64],
}
```

## Generated Code

The macro generates:

```rust
const _: () = {
    assert!(core::mem::align_of::<MyCapsule>() == 64);
    assert!(core::mem::size_of::<MyCapsule>() == 64);
    // ... power-of-2 and range checks
};

unsafe impl Send for MyCapsule {}
unsafe impl Sync for MyCapsule {}
```

## Compile-Time Errors & Warnings

### Error Messages Reference

All error messages are designed to be actionable with clear fix suggestions following UCE33 Q11 (Rust Transform).

#### Missing #[capsule(...)] Attribute

```rust
#[derive(ComputationalCapsule)]  // ERROR: Missing #[capsule(...)]
#[repr(C, align(64))]
struct BadCapsule { data: [u8; 64] }
```

Error message:
```
error: Missing #[capsule(...)] attribute
 Help: Add #[capsule(alignment = 64)] or similar
```

#### Missing #[repr(C)]

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(align(64))]  // ERROR: Missing C!
struct BadCapsule { data: [u8; 64] }
```

Error message:
```
error: Capsules must use #[repr(C)] for deterministic field layout

Computational capsules require predictable memory layout for cache optimization.

Help: Add #[repr(C, align(N))] to your struct:

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]  // ← Add this!
struct MyCapsule { ... }

UCE33 Q11: Rust's #[repr(C)] ensures zero-cost predictable layout
```

#### Alignment Mismatch Between #[repr(...)] and #[capsule(...)]

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]  // Says 64
#[repr(C, align(128))]      // But repr says 128!
struct AlignmentMismatch { data: [u8; 128] }
```

Error message:
```
error: Alignment mismatch between #[repr(...)] and #[capsule(...)]

#[capsule(alignment = 64)] specifies 64 bytes
#[repr(C, align(128))] specifies 128 bytes

These MUST match. Choose one:

Option 1: Update repr to match capsule
#[repr(C, align(64))]  // Change 128 → 64

Option 2: Update capsule to match repr
#[capsule(alignment = 128)]  // Change 64 → 128

Help: Use alignment = 64 for standard capsules
```

#### Alignment Not Power of 2

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 100)]  // ERROR: Not power of 2!
#[repr(C, align(128))]
struct BadCapsule { data: [u8; 128] }
```

Error message:
```
error: Capsule alignment must be power of 2
 Got: 100 (binary: 1100100)
 Valid: 32, 64, 128, 256
 Help: Use alignment = 64 for standard capsules
```

#### Alignment Out of Range

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 512)]  // ERROR: Too large!
#[repr(C, align(512))]
struct TooLarge { data: [u8; 512] }
```

Error message:
```
error: Capsule alignment out of range
 Got: 512 bytes
 Valid range: 32-256 bytes
 - 32B: Sub-line structures (tight packing)
 - 64B: Single cache line (prevents false sharing)
 - 128B: Dual cache line (DualAtomicU64 pattern)
 - 256B: Multi-line (large capsules)
 Help: Use alignment = 64 for most capsules
```

### Compile-Time Warnings (Non-Blocking)

The derive macro also generates warnings for potentially problematic field types:

#### Mutex<T> Fields Warning

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct SuboptimalCapsule {
    state: Mutex<u64>,  // WARNING: Not lockfree!
}
```

Warning message:
```
warning: Field `state` uses Mutex which is incompatible with capsule architecture.

Capsules require lockfree atomic operations (UCE33 Q10: Tier 1 Atomic).

Replace Mutex with:
- AtomicU64 for packed state (3-10× faster)
- DualAtomicU64 for dual-channel coordination
- Atomic types with appropriate memory ordering

Example:
// Before: Mutex<u64> (slow, blocking)
// After:  AtomicU64 (fast, lockfree)

See: /home/samuel/Docs/The Atomic Capsule.md
```

#### RwLock<T> Fields Warning

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct SuboptimalCapsule {
    state: RwLock<State>,  // WARNING: Not lockfree!
}
```

Warning message:
```
warning: Field `state` uses RwLock which is incompatible with capsule architecture.

Capsules require lockfree atomic operations (UCE33 Q10: Tier 1 Atomic).

Replace RwLock with:
- AtomicU64 for state coordination
- Atomic loads with Acquire ordering for reads
- Atomic stores with Release ordering for writes

Example:
// Before: RwLock<State> (slow, writer-blocking)
// After:  AtomicU64 (fast, always lock-free)
```

#### Cell<T>/RefCell<T> Fields Warning

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct SuboptimalCapsule {
    state: Cell<u64>,  // WARNING: Not thread-safe!
}
```

Warning message:
```
warning: Field `state` uses Cell/RefCell which may not be thread-safe.

Capsules are Send + Sync and require atomic operations for thread safety.

Replace Cell/RefCell with:
- AtomicU64/AtomicI64/AtomicBool for primitive types
- Atomic operations with appropriate memory ordering

Example:
// Before: Cell<u64> (not Send/Sync)
// After:  AtomicU64 (Send + Sync, lockfree)
```

## UCE33 Tier Validation

Valid tiers (from [UCE33 Framework](../../docs/frameworks/UCE33_FRAMEWORK.md)):

**Foundation Tiers (1-6)**:
- `Atomic`: Lockfree coordination (3-10x speedup)
- `SIMD`: Vectorized computation (2-19x speedup)
- `FixedPoint`: Deterministic precision (2-10x speedup)
- `Batch`: Throughput processing (10-100x speedup)
- `Streaming`: Continuous computation
- `Mixed`: Hybrid (compound speedups)

**Extended Tiers (7-10)**:
- `GPU`: Accelerator computing (100-1000x speedup)
- `Network`: Zero-copy networking (10-50x speedup)
- `Persistent`: Crash-safe storage
- `Probabilistic`: Approximate algorithms (100-1000x memory reduction)

## Testing

Run the example:

```bash
cargo run --example simple_capsule
```

Run compile-pass tests:

```bash
cargo test --test trybuild_tests -- compile_pass
```

Run compile-fail tests:

```bash
cargo test --test trybuild_tests -- compile_fail
```

## Performance

**Compilation overhead**: <20ms per capsule (measured on Intel Ultra 7 155H)

The proc-macro adds minimal compilation time while providing compile-time safety guarantees that prevent runtime bugs.

## ASSUM Framework Compliance

- `#ASSUME_CAPSULE_VALID`: All derived capsules have correct alignment/size
- `#VERIFY_CAPSULE`: Enforced by generated const assertions (compile-time)
- `#ASSUME_ALIGNMENT_POW2`: All alignments are powers of 2
- `#VERIFY_ALIGNMENT_POW2`: Enforced by generated assertions

## Implementation

**Module Structure** (v0.4.1):
- `lib.rs` (160 lines): Main derive handler with diagnostics integration
- `parser.rs` (200 lines): Attribute parsing with syn (alignment, size, tier, auditable, hash algorithms)
- `validator.rs` (392 lines): Alignment/size/tier/auditable validation
- `repr_validator.rs` (280 lines): #[repr(C, align(N))] validation (NEW in v0.4.1)
- `field_diagnostics.rs` (270 lines): Field type analysis and warnings (NEW in v0.4.1)
- `codegen.rs` (600 lines): Verification code generation (optimized iterator-based)
- `error_handler.rs` (43 lines): Error utilities

**Total**: ~1,945 lines of production code (includes auditable capsule support)

**Performance**:
- Compilation overhead: <20ms per capsule (measured on Intel Ultra 7 155H)
- Generated code: Optimized with iterator-based field processing
- Binary size impact: <5% (zero-cost abstractions)

**v0.4.1 Enhancements** (Phase 1.6):
- ✅ **Repr validation**: Ensures #[repr(C, align(N))] matches #[capsule(alignment = N)]
- ✅ **Enhanced error messages**: Actionable fixes with UCE33 Q11 guidance
- ✅ **Field diagnostics**: Compile-time warnings for Mutex/RwLock/Cell fields
- ✅ **Optimized codegen**: Iterator-based field processing (no Vec allocation)
- ✅ **35 unit tests**: All passing (repr validation + field diagnostics)
- ✅ **Clear documentation**: Comprehensive error reference section

## License

MIT OR Apache-2.0

## See Also

- [atomic_capsule](../atomic_capsule/): Foundation crate with verification macros
- [UCE33 Framework](../../docs/frameworks/UCE33_FRAMEWORK.md): Systematic discovery
- [The Computational Capsule](../../Docs/The Computational Capsule.md): Architecture philosophy
- [KEY_INNOVATIONS.md](../Docs/KEY_INNOVATIONS.md): Proven capsule patterns
