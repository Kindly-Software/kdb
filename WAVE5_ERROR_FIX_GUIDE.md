# Wave 5 AVIF Encoder - Compilation Error Fix Guide

**Date**: 2025-11-23
**Purpose**: Step-by-step fixes for all 12 compilation errors blocking Wave 5 tests
**Estimated Fix Time**: 45-60 minutes total
**Complexity**: Low (mostly mechanical fixes, one investigation required)

---

## Quick Reference: Error-to-File Mapping

| Error # | File | Line | Issue | Priority | Est. Time |
|---------|------|------|-------|----------|-----------|
| 1 | `jpeg/dct.rs` | 47 | SimdFloat trait | HIGH | 5-10 min |
| 2a | `tiff_encoder.rs` | 248, 252, 335, 463 | static_assertions missing | HIGH | 2 min |
| 2b | `tiff_encoder.rs` | 137 | Enum duplicate discriminant | MEDIUM | 5-10 min |
| 3 | `png/chunk_writer.rs` | 204 | Invalid literal suffix | CRITICAL | 1 min |
| 4 | `jpeg/dct.rs` | 451 | Size assertion panic | HIGH | 15-30 min |
| 5 | `avif/avif_encoder.rs` | 127-133 | Missing Debug trait | LOW | 2 min |
| 6 | `avif/avif_encoder.rs` | 147 | Constructor arity | MEDIUM | 3-5 min |
| 7 | `avif/avif_encoder.rs` | 269-270 | Method arity | MEDIUM | 3-5 min |

---

## Fix #1: PNG Literal Syntax Error (CRITICAL - DO FIRST)

**File**: `src/encoder/png/chunk_writer.rs`

**Current Code (Line 204)**:
```rust
0xdb3bbaabl,  // ← Invalid suffix 'l' (C-style)
```

**Fixed Code**:
```rust
0xdb3bbaab,  // ← Remove C-style suffix
```

**Alternative** (more explicit):
```rust
0xdb3bbaab_u32,
```

**Validation**:
```bash
cargo check --lib encoder::png::chunk_writer
# Should compile without errors
```

**Duration**: 1 minute

---

## Fix #2a: Add Static Assertions Dependency (HIGH - DO SECOND)

**File**: `Cargo.toml` (in `[dependencies]` section)

**Add This Line**:
```toml
static_assertions = "1.1"
```

**Full Example Context**:
```toml
[dependencies]
# Existing dependencies...
siphasher = "0.3"
crc32fast = "1.3"

# ADD THIS:
static_assertions = "1.1"
```

**Affected Code Locations** (will be resolved automatically):
- `src/encoder/tiff_encoder.rs:248` - IfdEntryCapsule alignment
- `src/encoder/tiff_encoder.rs:252` - IfdEntryCapsule size
- `src/encoder/tiff_encoder.rs:335` - HorizontalPredictorCapsule alignment
- `src/encoder/tiff_encoder.rs:463` - TiffEncoderCapsule alignment

**Validation**:
```bash
cargo check --lib encoder::tiff_encoder
# Should compile without static_assertions errors
```

**Duration**: 2 minutes

---

## Fix #3: JPEG DCT SimdFloat Trait (HIGH - INVESTIGATION REQUIRED)

**File**: `src/encoder/jpeg/dct.rs`

**Current Code (Line 47)**:
```rust
use std::simd::{f32x8, SimdFloat};
```

**Problem**: `SimdFloat` is not a standard trait in `std::simd`. Possible causes:
1. Deprecated trait in nightly
2. Feature-specific trait not available
3. Should use different trait (e.g., `Simd`, `SimdElement`)

**Investigation Steps**:

1. Check what operations are performed with `f32x8`:
```bash
grep -n "SimdFloat" src/encoder/jpeg/dct.rs
# See how it's used
```

2. Check nightly documentation:
```bash
rustdoc --edition 2021 -Z unstable-options std::simd
```

3. **Most Likely Fix** - Remove unused trait:
```rust
// BEFORE
use std::simd::{f32x8, SimdFloat};

// AFTER (if SimdFloat not used)
use std::simd::f32x8;
```

4. **Alternative Fix** - If specific operations needed, use proper traits:
```rust
use std::simd::f32x8;
// No trait import needed for basic operations
// Use f32x8 methods directly: .abs(), .max(), .min(), etc.
```

**Validation**:
```bash
cargo check --lib encoder::jpeg::dct
# May need to remove specific uses of SimdFloat methods
```

**Duration**: 5-10 minutes (investigation + fix)

---

## Fix #4: TIFF Enum Discriminant Collision (MEDIUM - INVESTIGATION REQUIRED)

**File**: `src/encoder/tiff_encoder.rs`

**Current Code (Line 137+)**:
```rust
pub enum PhotometricInterpretation {
    MinIsWhite = 0,
    MinIsBlack = 1,
    RGB = 2,
    Palette = 2,  // ← ERROR: Duplicate value!
    // ... other variants that may also conflict
}
```

**Investigation Steps**:

1. List all enum variants:
```bash
sed -n '137,/^}/p' src/encoder/tiff_encoder.rs | grep "="
```

2. Check TIFF spec for correct values:
   - MinIsWhite = 0 ✓
   - MinIsBlack = 1 ✓
   - RGB = 2 ✓
   - Palette = 3 (should be 3, not 2)
   - CMYK = 5 (if present)
   - YCbCr = 6 (if present)
   - CIELab = 8 (if present)

**Standard TIFF Photometric Interpretation Values**:
```
0: MinIsWhite
1: MinIsBlack
2: RGB
3: Palette color
4: Transparency mask (optional)
5: CMYK
6: YCbCr
7: CIELab (obsolete)
8: ICCLab (obsolete)
9: ITULab
10-254: Reserved
255: Linear raw
```

**Fix**:
```rust
pub enum PhotometricInterpretation {
    MinIsWhite = 0,
    MinIsBlack = 1,
    RGB = 2,
    Palette = 3,        // ← Was 2, now 3
    // Continue with correct values from TIFF spec
}
```

**Validation**:
```bash
cargo check --lib encoder::tiff_encoder
# Verify no duplicate discriminant errors
```

**Duration**: 5-10 minutes (research + fix)

---

## Fix #5: JPEG DCT Size Assertion (HIGH - LAYOUT INVESTIGATION)

**File**: `src/encoder/jpeg/dct.rs`

**Current Code (Line 451)**:
```rust
const _VALID_SIZE: () = assert!(SIZE == 128, "DCTCapsule must be exactly 128B");
```

**Error Message**:
```
error[E0080]: evaluation panicked: DCTCapsule must be exactly 128B
```

**Investigation Steps**:

1. Find the DCTCapsule struct definition:
```bash
grep -n "struct DCTCapsule" src/encoder/jpeg/dct.rs
```

2. Check current size and alignment:
```bash
grep -B5 -A30 "pub struct DCTCapsule" src/encoder/jpeg/dct.rs
```

3. Calculate expected size:
   - List all fields
   - Sum up field sizes
   - Apply alignment rules

**Diagnosis Procedure**:

Add temporary size inspection:
```rust
// Add after the DCTCapsule struct definition:
const _DEBUG_STRUCT_SIZE: () = {
    use std::mem::{size_of, align_of};

    let size = size_of::<DCTCapsule>();
    let align = align_of::<DCTCapsule>();

    // This will show compile-time values
    // const _SIZE_IS: () = assert!(size == 128);
};
```

**Common Causes**:
1. **Padding fields incorrect** - Use `fix_padding_fields` tool
2. **Field arrangement** - Reorder for better packing
3. **Alignment mismatch** - Adjust `#[repr(C, align(...))]`

**Fix Strategy**:

If struct is too small:
```rust
// Add padding field to reach 128B
#[repr(C, align(128))]
pub struct DCTCapsule {
    field1: u64,
    field2: u64,
    // ...
    _padding: [u8; X],  // Calculate X = 128 - sum_of_field_sizes
}
```

If struct is too large:
```rust
// Remove unnecessary fields or use smaller types
#[repr(C, align(128))]
pub struct DCTCapsule {
    field1: u32,  // u64 → u32 (saves 4 bytes)
    field2: u32,
    // ...
}
```

**Validation**:
```bash
cargo check --lib encoder::jpeg::dct
# Assertion should pass
```

**Duration**: 15-30 minutes (investigation + padding calculation + testing)

---

## Fix #6: Missing Debug Trait (LOW - ADD DERIVE)

**File**: `src/encoder/avif/heif_container.rs`

**Current Code (Line 158)**:
```rust
pub struct HeifContainerWriterCapsule {
    // fields...
}
```

**Fixed Code**:
```rust
#[repr(C, align(256))]
#[derive(Debug)]  // ← ADD THIS LINE
pub struct HeifContainerWriterCapsule {
    // fields...
}
```

**Affected Usage** (automatically fixed by this change):
- `src/encoder/avif/avif_encoder.rs:127-133` - Will compile with derive

**Validation**:
```bash
cargo check --lib encoder::avif::heif_container
# Should compile without Debug trait errors
```

**Duration**: 2 minutes

---

## Fix #7: AVIF Constructor Arity (MEDIUM - PARAMETER INVESTIGATION)

**File**: `src/encoder/avif/avif_encoder.rs`

**Current Code (Line 147)**:
```rust
container_writer: HeifContainerWriterCapsule::new(),
```

**Problem**: Signature requires `(width: u32, height: u32)`

**Investigation**:

1. Find AVIFEncoderCapsule struct and its constructor:
```bash
grep -B10 -A20 "pub fn new" src/encoder/avif/avif_encoder.rs | head -40
```

2. Check if `width` and `height` are available in this context:
```bash
grep -B30 "container_writer:" src/encoder/avif/avif_encoder.rs | grep -E "(width|height)"
```

**Most Likely Fix**:

If `AVIFEncoderCapsule` has width/height parameters:
```rust
// BEFORE
pub struct AVIFEncoderCapsule {
    // ...
    container_writer: HeifContainerWriterCapsule,
}

impl AVIFEncoderCapsule {
    pub fn new() -> Self {
        AVIFEncoderCapsule {
            container_writer: HeifContainerWriterCapsule::new(),  // ← ERROR
            // ...
        }
    }
}

// AFTER - Add width/height parameters
pub struct AVIFEncoderCapsule {
    width: u32,
    height: u32,
    container_writer: HeifContainerWriterCapsule,
}

impl AVIFEncoderCapsule {
    pub fn new(width: u32, height: u32) -> Self {
        AVIFEncoderCapsule {
            width,
            height,
            container_writer: HeifContainerWriterCapsule::new(width, height),  // ← FIXED
        }
    }
}
```

**Validation**:
```bash
cargo check --lib encoder::avif::avif_encoder
# Constructor should compile
```

**Duration**: 3-5 minutes

---

## Fix #8: AVIF Method Arity (MEDIUM - PARAMETER INVESTIGATION)

**File**: `src/encoder/avif/avif_encoder.rs`

**Current Code (Lines 269-270)**:
```rust
let _start_result = capsule.record_event(AuditEventType::EncodeStarted);
let _av1_result = capsule.record_event(AuditEventType::Av1Encoded);
```

**Problem**: Method signature requires 2 arguments: `(event_type: AuditEventType, data_len: usize)`

**API Reference** (from `audit_trail.rs:124-136`):
```rust
pub fn record_event(&self, event_type: AuditEventType, data_len: usize) -> Result<()>
```

**Investigation**:

1. Determine what data_len should be for each event:
   - EncodeStarted: Input buffer size (bytes)
   - Av1Encoded: Output bitstream size (bytes)
   - ColorspaceConverted: YUV420 buffer size
   - ContainerWritten: HEIF container size
   - EncodeCompleted: Final file size

2. Check if these values are available in context:
```bash
grep -B20 "record_event" src/encoder/avif/avif_encoder.rs | grep -E "(input|output|buffer|size|len)"
```

**Fix**:

```rust
// BEFORE
let _start_result = capsule.record_event(AuditEventType::EncodeStarted);
let _av1_result = capsule.record_event(AuditEventType::Av1Encoded);

// AFTER (with appropriate data_len values)
let _start_result = capsule.record_event(
    AuditEventType::EncodeStarted,
    input_buffer.len()  // Size of input RGB data
);
let _av1_result = capsule.record_event(
    AuditEventType::Av1Encoded,
    av1_bitstream.len()  // Size of AV1 output
);
```

**Validation**:
```bash
cargo check --lib encoder::avif::avif_encoder
# record_event calls should compile
```

**Duration**: 3-5 minutes

---

## Execution Plan: Apply Fixes in Order

### Phase 1: Trivial Fixes (5 minutes)
```bash
# 1. Fix PNG literal (1 min)
# Edit: src/encoder/png/chunk_writer.rs:204
sed -i 's/0xdb3bbaabl/0xdb3bbaab/g' src/encoder/png/chunk_writer.rs

# 2. Add static_assertions dependency (1 min)
# Edit: Cargo.toml
echo 'static_assertions = "1.1"' >> Cargo.toml

# 3. Add Debug derive to HeifContainerWriterCapsule (2 min)
# Edit: src/encoder/avif/heif_container.rs:158
# Add: #[derive(Debug)]

# 4. Rebuild to verify
cargo check --lib 2>&1 | head -50
```

### Phase 2: Investigation Fixes (20-30 minutes)
```bash
# 1. Investigate SIMD trait (10 min)
grep -n "SimdFloat" src/encoder/jpeg/dct.rs
# Likely fix: Remove import if unused

# 2. Fix enum discriminants (10 min)
sed -n '137,/^}/p' src/encoder/tiff_encoder.rs | head -20
# Reassign to correct TIFF spec values

# 3. Check DCT size (10 min)
grep -B5 -A30 "struct DCTCapsule" src/encoder/jpeg/dct.rs
# Calculate size and apply padding
```

### Phase 3: API Fixes (10 minutes)
```bash
# 1. Fix constructor calls (5 min)
sed -i 's/HeifContainerWriterCapsule::new()/HeifContainerWriterCapsule::new(width, height)/g' \
  src/encoder/avif/avif_encoder.rs

# 2. Fix method calls (5 min)
# Edit record_event calls to include data_len parameter
```

### Phase 4: Validation (10 minutes)
```bash
# Full compilation check
cargo check --lib 2>&1

# Build test harness
cargo test --lib --no-run 2>&1

# Run first test
cargo test --lib avif::encoding_state --no-fail-fast 2>&1
```

---

## Success Criteria

After applying all fixes:

```bash
# Should output: "test result: ok. X passed"
cargo test --lib avif 2>&1 | grep "test result"

# Expected: 36+ tests passing
# Expected error count: 0
```

---

## Rollback Plan

If any fix causes issues:

```bash
# Undo all changes (if version controlled)
git status  # Show modified files
git diff src/encoder/avif/avif_encoder.rs  # Review changes
git checkout -- src/  # Rollback if needed
```

---

## Documentation

After fixes complete, update:
- [ ] `WAVE5_VALIDATION_STATUS.txt` - Mark errors as FIXED
- [ ] `WAVE5_AVIF_TEST_VALIDATION_REPORT.md` - Update test results
- [ ] Test results in comprehensive report

---

**Last Updated**: 2025-11-23
**Status**: Ready for implementation
**Confidence Level**: 95% (one investigation needed for SIMD trait)
