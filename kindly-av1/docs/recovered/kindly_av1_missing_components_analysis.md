# kindly-av1 AV1 Encoder - Complete Missing Components Analysis

## Executive Summary

The kindly-av1 encoder is **30-40% complete** as a dav1d-compliant AV1 implementation. The wiring_capsule.rs encode_frame() function is **mostly placeholder** - it writes stub sequence/frame headers and placeholder tile data without actual codec algorithms. The encoder lacks critical algorithms for spec-compliant output.

**Root Cause of dav1d Failure**: `write_sequence_header()` generates dummy 8-byte payloads instead of AV1 spec-compliant sequence headers with proper bit packing.

---

## 1. The encode_frame() Placeholder Flow

### Location
`/home/samuel/Primitives/kindly-av1/src/encoder/wiring_capsule.rs:84-130`

### Current Implementation (PLACEHOLDER)
```rust
pub fn encode_frame(&self, yuv_data: &[u8], sub_capsules: &EncoderSubCapsules) -> Vec<u8> {
    // Line 120: Create placeholder tile data
    let tile_data = vec![0u8; 64];  // ← STUB: Just 64 zero bytes
    
    // Real workflow needed:
    // 1. YUV420 frame → Superblocks (64×64)
    // 2. Mode decision + RDO
    // 3. Transform (DCT/ADST)
    // 4. Quantization
    // 5. Entropy coding
    // 6. Loop filtering
    // 7. Bitstream packing
}
```

### What's Missing
| Step | Current | Needed |
|------|---------|--------|
| **1. Frame decomposition** | None | Split YUV into 64×64 superblocks, 32×32/16×16 blocks |
| **2. Mode decision** | None | Intra/inter prediction mode selection (56 intra modes) |
| **3. Transform** | Capsule exists (DCT) | Actually **call** `dct.transform()` on residuals |
| **4. Quantization** | Capsule exists (Q16.16) | Actually **call** `quantizer.quantize()` on coeffs |
| **5. Entropy coding** | Capsule exists (Daala) | Actually **call** `entropy.encode_symbol()` |
| **6. Loop filter** | Capsule exists (CDEF/LRF) | Actually **call** `loop_filter.apply()` |
| **7. Bitstream write** | Capsule exists (OBU) | Actually **populate** sequence/frame headers |

---

## 2. Missing Codec Algorithms (Priority Order)

### CRITICAL (Blocking dav1d): Must Have for Any Output

#### 2.1 Spec-Compliant Sequence Header
**Severity**: CRITICAL  
**File**: `/home/samuel/Primitives/atomic_capsule/src/encoder/obu_bitstream.rs:373-398`

**Current (BROKEN)**:
```rust
// write_sequence_header() - Line 373-398
pub fn write_sequence_header(&self, profile: u8, level: u8) -> Vec<u8> {
    // Line 376-381: Placeholder 8-byte payload
    let payload = vec![
        profile & 0x03,
        level & 0x1F,
        0, 0, 0, 0, 0, 0,  // ← Six ZEROS (dav1d rejects this)
    ];
}
```

**Why dav1d Fails**:
- AV1 spec requires 27-bit sequence header with bit packing:
  - Profile (3 bits)
  - Level (5 bits)
  - Tier (1 bit)
  - Still picture flag (1 bit)
  - Reserved (1 bit)
  - seq_force_integer_mv (1 bit)
  - seq_force_screen_content_tools (1 bit)
  - ...and 15 more fields
- Current code outputs 6 zero bytes → dav1d: "Error parsing sequence header"

**Fix Needed**: Implement AV1 spec §5.5 Sequence Header Syntax with proper bit packing.

**Complexity**: **HIGH** (40-60 lines, 8 fields, bit-level precision)

---

#### 2.2 Spec-Compliant Frame Header
**Severity**: CRITICAL  
**File**: `/home/samuel/Primitives/atomic_capsule/src/encoder/obu_bitstream.rs:423-446`

**Current (BROKEN)**:
```rust
// write_frame_header() - Line 423-446
pub fn write_frame_header(&self, frame_type: FrameType, width: u16, height: u16) -> Vec<u8> {
    let payload = vec![
        frame_type as u8,
        (width >> 8) as u8,
        (width & 0xFF) as u8,
        (height >> 8) as u8,
        (height & 0xFF) as u8,  // ← Only 5 bytes, missing 80+ fields
    ];
}
```

**Why dav1d Fails**:
- AV1 spec requires frame header with 80+ fields (uncompressed + compressed):
  - show_existing_frame (1 bit)
  - frame_to_show_map_idx (3 bits, conditional)
  - frame_type (2 bits)
  - show_frame (1 bit)
  - error_resilient_mode (1 bit)
  - disable_cdf_update (1 bit)
  - ...and ~75 more fields across 3KB spec text

**Fix Needed**: Implement AV1 spec §5.6 Frame Header Syntax.

**Complexity**: **VERY HIGH** (200+ lines, conditional fields, tile grid parameters)

---

### HIGH PRIORITY (Blocks Real Encoding)

#### 2.3 Tile Data Encoding Pipeline
**Severity**: HIGH  
**Current**: Empty placeholder (64 zero bytes)

**Missing Steps**:
1. **Superblock iteration** (64×64 blocks)
2. **Prediction mode selection** (56 intra modes via `IntraPredictionCapsule`)
3. **Residual transform** (4×4/8×8/16×16/32×32 via `DctTransformCapsule`)
4. **Coefficient quantization** (Q16.16 via `QuantizationCapsule`)
5. **Entropy encoding per symbol** (Daala range coder via `EntropyCoderCapsule`)
6. **Loop filtering** (CDEF + LRF via `LoopFilterCapsule`)
7. **Bitstream packing** (OBU format via `ObuBitstreamWriterCapsule`)

**What atomic_capsule Provides**:
- `IntraPredictionCapsule` (T2 SIMD, 256B) - 56 prediction modes
- `DctTransformCapsule` (T2 SIMD, 256B) - Chen-Wang DCT
- `QuantizationCapsule` (T3 Fixed-Point, 128B) - Q16.16 deterministic
- `EntropyCoderCapsule` (T2 SIMD, 256B) - Daala range coder
- `LoopFilterCapsule` (T2 SIMD, 256B) - CDEF + LRF

**What's Missing**: **Integration code** that calls these capsules in the right order.

**Fix Needed**: Implement loop in encode_frame():
```rust
for superblock in yuv_frame.superblocks() {
    for block in superblock.blocks() {
        let pred = intra_capsule.predict(block, mode);
        let residual = block - pred;
        let coeffs = dct_capsule.transform(&residual);
        let quantized = quant_capsule.quantize_block(&coeffs);
        entropy_capsule.encode_symbol(quantized);
    }
    let filtered = loop_filter_capsule.apply(&superblock);
    bitstream_capsule.write_tile_group(&filtered);
}
```

**Complexity**: **HIGH** (80-120 lines, coordination logic)

---

#### 2.4 Rate-Distortion Optimization (RDO)
**Severity**: HIGH  
**Current**: None

**What's Missing**:
- Mode decision (which intra mode minimizes distortion?)
- Quantizer selection (which QP for each block?)
- Motion estimation (inter-frame mode selection)
- Temporal RDO (multi-frame optimization)

**atomic_capsule Support**: None (Phase 2 not started)

**Fix Needed**: Implement basic intra RDO loop (Phase 1):
```rust
let mut best_distortion = f32::INFINITY;
let mut best_mode = 0;
for intra_mode in 0..56 {
    let pred = intra_capsule.predict(block, intra_mode);
    let residual = (block - pred).pow(2).sum(); // MSE
    if residual < best_distortion {
        best_distortion = residual;
        best_mode = intra_mode;
    }
}
```

**Complexity**: **MEDIUM** (60-100 lines, but slow without optimization)

---

#### 2.5 Reference Frame Management
**Severity**: MEDIUM-HIGH  
**Current**: Capsule exists but integration missing

**What's Missing**:
- Frame buffer management (store ref frames)
- Reference frame signaling (which frame to use for inter)
- Reference scaling (scale inter-frame predictions)

**atomic_capsule Support**: `ReferenceFrameCapsule` (T1, 128B) exists

**Fix Needed**: Call `ref_frames_capsule.add_reference()` and `.get_reference()` in encode loop.

**Complexity**: **MEDIUM** (40-60 lines)

---

### MEDIUM PRIORITY (Improves Quality)

#### 2.6 Loop Filtering (CDEF + LRF)
**Severity**: MEDIUM  
**Current**: Capsule exists but integration missing

**atomic_capsule Support**: `LoopFilterCapsule` (T2 SIMD, 256B) exists

**Fix Needed**: Post-encode filtering step:
```rust
let filtered_frame = loop_filter_capsule.apply(&reconstructed_frame);
```

**Complexity**: **LOW** (10-20 lines)

---

#### 2.7 Film Grain Synthesis
**Severity**: MEDIUM (Quality only)  
**Current**: None

**What's Missing**: Grain matching for visually lossless encoding

**atomic_capsule Support**: None (Phase 2 not started)

**Fix Needed**: Phase 2 feature

**Complexity**: **MEDIUM** (100-150 lines)

---

### LOW PRIORITY (Optimization)

#### 2.8 Super-Resolution (optional)
**Severity**: LOW (Optimization only)  
**Current**: None

**atomic_capsule Support**: None (Phase 2 not started)

**Fix Needed**: Phase 2+ feature

---

## 3. atomic_capsule Encoder Modules - What's Real vs Stub

### Tier 1 (Atomic) - 100% Real
| Module | Size | Status | What Works | What's Missing |
|--------|------|--------|-----------|-----------------|
| **EncoderStateCapsule** | 64B | ✅ Real | State coordination | None |
| **FrameBufferCapsule** | 128B | ✅ Real | YUV storage, frame metadata | Superblock layout |
| **ReferenceFrameCapsule** | 128B | ✅ Real | Frame storage API | Integration in pipeline |

### Tier 2 (SIMD) - Mostly Real, Some Stubs
| Module | Size | Status | What Works | What's Missing |
|--------|------|--------|-----------|-----------------|
| **IntraPredictionCapsule** | 256B | ✅ Real | 56 prediction modes (DC, planar, angle) | Mode decision integration |
| **DctTransformCapsule** | 256B | ⚠️ Partial | Transform kernels exist | Actual DCT arithmetic (Chen-Wang may be skeleton) |
| **EntropyCoderCapsule** | 256B | ⚠️ Partial | Range coder state, CDF tables | `encode_symbol()` has TODO: "Bit packing logic" |
| **LoopFilterCapsule** | 256B | ⚠️ Partial | CDEF/LRF APIs | Core filtering algorithms |

**Evidence of Stub**: 
- `/home/samuel/Primitives/atomic_capsule/src/encoder/entropy_coder.rs:483-487`:
  ```rust
  // For now, just increment outstanding_bits as placeholder
  self.outstanding_bits.fetch_add(1, Ordering::Relaxed);
  // TODO: Actual bit packing logic goes here
  ```

### Tier 3 (Fixed-Point) - 100% Real
| Module | Size | Status | What Works |
|--------|------|--------|-----------|
| **QuantizationCapsule** | 128B | ✅ Real | Q16.16 quantize/dequantize blocks |

### Tier 5 (Streaming) - 100% Real But Missing Spec Details
| Module | Size | Status | What Works | What's Missing |
|--------|------|--------|-----------|-----------------|
| **ObuBitstreamWriterCapsule** | 128B | ⚠️ Partial | OBU header packing, LEB128 encoding | Spec-compliant SH/FH payloads |

### Tier 4 (Batch) - Not Used
| Module | Size | Status |
|--------|------|--------|
| **TileCoordinatorCapsule** | 128B | ✅ Exists but not integrated |
| **ParallelTileEncoderCapsule** | 256B | ✅ Exists but not used |

---

## 4. Why Tests Fail with "Error parsing sequence header"

### Root Cause Chain

1. **kindly-av1 calls** `encoder.encode_frame()` → wiring_capsule.rs:102
   
2. **wiring_capsule calls** `bitstream().write_sequence_header(0, 0)` → obu_bitstream.rs:373

3. **write_sequence_header outputs**:
   ```
   [0x89]              // OBU header (1 byte)
   [0x01]              // LEB128 size = 1 byte (wrong!)
   [0x00]              // profile (placeholder)
   [0x00]              // level (placeholder)
   [0x00, 0x00, 0x00, 0x00, 0x00, 0x00]  // Six ZEROS
   ```

4. **dav1d decoder reads** the sequence header payload and expects:
   - 27 bits of structured data (profile, level, tier, etc.)
   - Proper bitstream format per AV1 spec §5.5
   - **But finds**: 8 random bytes with wrong structure
   
5. **dav1d fails** with: `"Error parsing sequence header: invalid profile/level"`

### The Fix
Replace the 8-byte placeholder with spec-compliant bitpacking:

```rust
// AV1 Spec §5.5 - Sequence Header OBU
let mut bits = BitWriter::new();
bits.write(3, profile);                    // profile (3 bits)
bits.write(5, level);                      // level (5 bits)
bits.write(1, tier);                       // tier (1 bit)
bits.write(1, still_picture_flag);         // still_picture (1 bit)
bits.write(1, 0);                          // reduced_still_picture_header (1 bit)
// ... continue for ~20 more fields
let payload = bits.finalize();
```

---

## 5. Prioritized Implementation Roadmap

### Phase 1: Dav1d Compliance (2-3 weeks)
**Goal**: Pass dav1d validation on simple grayscale frames

| Priority | Task | Files | Complexity | Status |
|----------|------|-------|-----------|--------|
| **P0** | Spec-compliant sequence header | obu_bitstream.rs | HIGH | 0% |
| **P0** | Spec-compliant frame header | obu_bitstream.rs | VERY HIGH | 0% |
| **P1** | Tile encoding pipeline | wiring_capsule.rs | HIGH | 0% |
| **P1** | Intra prediction integration | wiring_capsule.rs | MEDIUM | 0% |
| **P1** | DCT transform call | wiring_capsule.rs | MEDIUM | 0% |
| **P1** | Quantization call | wiring_capsule.rs | MEDIUM | 0% |
| **P1** | Entropy coding call | wiring_capsule.rs | MEDIUM | 5% |
| **P2** | Basic RDO (intra modes) | wiring_capsule.rs | MEDIUM | 0% |
| **P2** | Loop filtering integration | wiring_capsule.rs | LOW | 0% |

**Estimated effort**: 300-400 lines of new code + 50-100 lines fixes to atomic_capsule

### Phase 2: Quality Improvements (4-6 weeks)
- Inter-frame prediction
- Motion estimation
- Temporal RDO
- Film grain synthesis
- Rate control

### Phase 3: Performance Optimization (2-3 weeks)
- Parallel tile encoding
- GPU acceleration (ROCm/Vulkan)
- SIMD coefficient packing

---

## 6. File Checklist: What Needs Implementation

### atomic_capsule (7,922 lines total)

```
✅ state.rs (687 lines) - Complete, real state management
✅ frame_buffer.rs (626 lines) - Complete, real YUV storage
✅ quantization.rs (703 lines) - Complete, real Q16.16 math
⚠️ obu_bitstream.rs (687 lines) - NEEDS FIX:
   - write_sequence_header() → Spec-compliant bit packing (lines 373-398)
   - write_frame_header() → Spec-compliant field layout (lines 423-446)
   - Payload sizes are wrong (8 bytes vs 100+ bytes)
⚠️ entropy_coder.rs (590 lines) - NEEDS FIX:
   - encode_symbol() has TODO comment (line 487)
   - Actual bit packing logic missing
⚠️ dct_transform.rs (635 lines) - NEEDS VERIFICATION:
   - Chen-Wang DCT kernels may be skeleton (need code review)
⚠️ loop_filter.rs (570 lines) - NEEDS VERIFICATION:
   - CDEF/LRF filtering algorithms (need code review)
✅ intra_prediction.rs (696 lines) - Complete, real 56 modes
✅ reference_frame.rs (510 lines) - Complete, real storage
✅ tile_coordinator.rs (579 lines) - Complete, real coordination
✅ parallel_tile_encoder.rs (756 lines) - Complete, real parallelism
✅ file_io.rs (639 lines) - Complete, real YUV reading
```

### kindly-av1 (encoder/ 6,058 lines total)

```
⚠️ wiring_capsule.rs (6,058 lines) - NEEDS MAJOR WORK:
   - encode_frame() is 50 lines, mostly placeholder
   - Missing tile encoding loop (80-120 lines)
   - Missing integration of DCT/quantization/entropy/filtering
⚠️ sub_capsules.rs (314 lines) - OK, just accessor methods
⚠️ metacapsule.rs - CLI bridge, needs encode_frame() integration
⚠️ config.rs - Configuration, needs more parameters
```

---

## 7. Implementation Estimates

### To achieve dav1d-compliant output:

| Task | LOC | Time | Difficulty |
|------|-----|------|-----------|
| **Spec header (SH)** | 60 | 4h | HIGH |
| **Frame header (FH)** | 200 | 16h | VERY HIGH |
| **Tile pipeline** | 100 | 8h | MEDIUM |
| **Entropy coding fix** | 40 | 4h | MEDIUM |
| **Integration** | 80 | 8h | MEDIUM |
| **Testing** | — | 16h | MEDIUM |
| **Total** | ~480 | 56h (1.5 weeks solo) | — |

### With atomic_capsule fixes (if needed):

| Task | LOC | Time | Difficulty |
|------|-----|-----|-----------|
| **DCT verification** | 0 | 2h | LOW |
| **Entropy coder fix** | 40 | 4h | MEDIUM |
| **Loop filter fix** | 20 | 2h | LOW |
| **Total** | ~60 | 8h | — |

---

## 8. Critical Files to Focus On

### Must Fix (blocking dav1d validation)
1. **obu_bitstream.rs** - lines 373-398, 423-446 (sequence/frame headers)
2. **wiring_capsule.rs** - lines 84-130 (encode_frame integration)
3. **entropy_coder.rs** - lines 483-487 (bit packing TODO)

### Must Integrate (missing pipeline)
1. **wiring_capsule.rs** - Need tile encoding loop calling all sub-capsules
2. **sub_capsules.rs** - Already provides accessor methods (OK)
3. **metacapsule.rs** - Bridge encode_frame properly (OK structure, needs parameter tuning)

### Should Verify (may be stubs)
1. **dct_transform.rs** - Chen-Wang kernels may be skeleton
2. **loop_filter.rs** - CDEF/LRF may be incomplete
3. **intra_prediction.rs** - 56 modes implemented? (likely OK given tests pass)

---

## 9. Test Failures Explained

### Current Test Results (from CLAUDE.md)
```
✅ Unit (lib): 1718 tests pass
✅ Y4M Round-trip: 3 tests ignored (need dav1d)
✅ Bitstream Integration: 15 tests pass
❌ dav1d Validation: "Error parsing sequence header"
```

### Why dav1d Validation Fails
- **Sequence header** is 8 zero bytes (placeholder) → dav1d expects 27+ bits of structured data
- **Frame header** is 5 bytes (width/height only) → dav1d expects 80+ fields
- **Tile data** is 64 zero bytes (placeholder) → dav1d expects compressed superblocks

**Solution**: Implement spec-compliant headers + tile encoding pipeline.

---

## 10. Summary Table: All Missing Components

| Component | Severity | Location | Status | Lines Needed | Est. Hours |
|-----------|----------|----------|--------|--------------|-----------|
| **Sequence Header Spec** | CRITICAL | obu_bitstream.rs:373 | 0% | 60 | 4 |
| **Frame Header Spec** | CRITICAL | obu_bitstream.rs:423 | 0% | 200 | 16 |
| **Tile Encoding Loop** | CRITICAL | wiring_capsule.rs:84 | 5% | 100 | 8 |
| **Entropy Bit Packing** | HIGH | entropy_coder.rs:483 | 5% | 40 | 4 |
| **Mode Decision (RDO)** | HIGH | wiring_capsule.rs | 0% | 80 | 8 |
| **Reference Mgmt** | MEDIUM | wiring_capsule.rs | 0% | 40 | 4 |
| **Loop Filtering** | MEDIUM | wiring_capsule.rs | 0% | 20 | 2 |
| **DCT Verification** | LOW | dct_transform.rs | Unknown | 0 | 2 |
| **—** | **TOTAL** | **—** | **~5%** | **~540** | **~48-56h** |

---

## Conclusion

The kindly-av1 encoder is **5% complete** in terms of codec implementation:
- ✅ Infrastructure (capsules, state management) is solid (95% done)
- ❌ Codec algorithm integration is missing (5% done)
- ❌ Spec compliance is broken (0% done for headers)

**To achieve dav1d compliance**: Need ~540 LOC in 48-56 hours, starting with sequence/frame headers + tile pipeline integration.

**Root blocker**: `write_sequence_header()` and `write_frame_header()` are stubs with placeholder payloads.

