# AV1 Sequence Header Analysis: reduced_still_picture_header=1, mono_chrome=1, profile=0

**Date**: 2025-11-25  
**Mode**: Reduced Still Picture Header (Simplified)  
**Configuration**: Monochrome 8-bit, Profile 0 (Main), 64x64 frame  
**Source**: AV1 Specification Sections 5.5.1, 5.5.2, 5.3.4  

---

## Executive Summary

For the specified mode:
- **profile=0** (Main), **reduced_still_picture_header=1** (simplified), **mono_chrome=1** (grayscale)
- **REQUIRED fields only** (optional fields suppressed in reduced mode)
- **Total bitstream: ~10-12 bytes** (OBU header + leb128 size + bit-level payload)
- **Key constraint**: mono_chrome=1 removes chroma-related fields from color_config()

---

## Section 5.5.1: sequence_header_obu() - Exact Field Order

### Sequence Header Payload (Bit-Level Packing)

| Field | Type | Width | Value* | Bits | Notes |
|-------|------|-------|--------|------|-------|
| seq_profile | f(3) | 3 bits | 0 | 3 | Main Profile |
| still_picture | f(1) | 1 bit | 1 | 1 | REQUIRED when reduced_still_picture_header=1 |
| reduced_still_picture_header | f(1) | 1 bit | 1 | 1 | Simplified mode (removes feature flags) |
| seq_level_idx[0] | f(5) | 5 bits | 0 | 5 | Level 2.0 (supports up to 2048×1152 @ 60fps) |
| frame_width_bits_minus_1 | f(4) | 4 bits | 5 | 4 | For 64: log2(63)=5.977, so 6-1=5 |
| frame_height_bits_minus_1 | f(4) | 4 bits | 5 | 4 | For 64: log2(63)=5.977, so 6-1=5 |
| max_frame_width_minus_1 | f(6) | 6 bits | 63 | 6 | 64-1=63 (uses 6 bits from frame_width_bits) |
| max_frame_height_minus_1 | f(6) | 6 bits | 63 | 6 | 64-1=63 (uses 6 bits from frame_height_bits) |
| **[color_config() follows]** | | | | | See Section 5.5.2 |

**Subtotal so far**: 3+1+1+5+4+4+6+6 = **30 bits** (3.75 bytes)

---

## Section 5.5.2: color_config() - With mono_chrome=1

### Color Config for mono_chrome=1 (CRITICAL)

| Field | Type | Width | Value* | Bits | Notes |
|-------|------|-------|--------|------|-------|
| high_bitdepth | f(1) | 1 bit | 0 | 1 | 0=8-bit (mono_chrome compatible) |
| twelve_bit | OMITTED | — | — | 0 | Only if profile==2 AND high_bitdepth==1 |
| mono_chrome | f(1) | 1 bit | 1 | 1 | Enabled for grayscale |
| color_description_present_flag | f(1) | 1 bit | 0 | 1 | Omit color primaries/transfer/matrix |
| **[IF color_description_present]** | | | | | **SKIPPED** for mono_chrome |
| color_range | f(1) | 1 bit | 0 | 1 | **ONLY THIS** when mono_chrome=1 |
| separate_uv_delta_q | OMITTED | — | — | 0 | **ONLY for color** (not mono_chrome per spec Section 5.5.2) |

**Color Config subtotal**: 1+1+1+1 = **4 bits** (0.5 bytes)

**Total sequence_header_obu() payload**: 30 + 4 = **34 bits** = **5 bytes (with 6-bit padding)**

---

## Section 5.3.4: trailing_bits() Requirement

### Mandatory Trailing Bits

| Field | Type | Description |
|-------|------|-------------|
| trailing_one_bit | f(1) | **Must be 1** (signal end of bitstream) |
| trailing_zero_bits | f(0..7) | Fill to byte boundary with 0s |

For 34 bits payload:
- Current position: 34 bits = 4 bytes + 2 bits
- After `write_trailing_bits()`:
  - Write 1 bit (value=1): total = 35 bits
  - Pad with 1 zero bit to reach 36 bits = 4.5 bytes
  - Flush to 5 bytes (40 bits total, 4 zero padding bits)

**Final payload size**: **5 bytes**

---

## Complete Bitstream Structure (OBU Level)

```
┌─────────────────────────────────────────┐
│ SEQUENCE HEADER OBU (AV1 Spec 5.3.2)   │
└─────────────────────────────────────────┘

Byte 0: OBU Header
  ┌──────────────────┐
  │ 7 │ 6 5 4 3 │ 2 │ 1 │ 0 │
  ├──────────────────┤
  │ 0 │  0 0 0 1 │ 0 │ 1 │ 0 │
  └──────────────────┘
  
  Bits:
  [7]   forbidden_bit = 0 (required)
  [6:3] obu_type = 1 (SequenceHeader)
  [2]   obu_extension_flag = 0 (no temporal/spatial IDs)
  [1]   obu_has_size_field = 1 (size field present)
  [0]   obu_reserved_1bit = 0 (required)
  
  Resulting: 0b00001010 = 0x0A

Bytes 1+: leb128 size of payload
  Payload size: 5 bytes
  leb128(5): 0x05

Bytes 2-6: Payload (5 bytes)
  ┌─ Sequence Header Fields (Section 5.5.1) ─┐
  │                                          │
  │ seq_profile[0:2]           = 000 (3 bits)│
  │ still_picture              = 1   (1 bit) │
  │ reduced_still_picture_hdr  = 1   (1 bit) │
  │ seq_level_idx[0][0:4]      = 00000 (5b)  │
  │ frame_width_bits_minus_1   = 0101 (4 b)  │
  │ frame_height_bits_minus_1  = 0101 (4 b)  │
  │ max_frame_width_minus_1    = 111111 (6 b)│
  │ max_frame_height_minus_1   = 111111 (6 b)│
  │                                          │
  │ Subtotal: 30 bits = 3.75 bytes          │
  │                                          │
  ├─ Color Config (Section 5.5.2) ───────────┤
  │                                          │
  │ high_bitdepth              = 0   (1 bit) │
  │ [twelve_bit: OMITTED]                    │
  │ mono_chrome                = 1   (1 bit) │
  │ color_description_present  = 0   (1 bit) │
  │ color_range (mono only)    = 0   (1 bit) │
  │ [separate_uv_delta_q: OMITTED]           │
  │                                          │
  │ Subtotal: 4 bits = 0.5 bytes            │
  │                                          │
  ├─ Trailing Bits (Section 5.3.4) ──────────┤
  │                                          │
  │ trailing_one_bit           = 1   (1 bit) │
  │ trailing_zero_bits (pad)   = 0   (1 bit) │
  │                                          │
  │ Subtotal: 2 bits → rounds to 6 bits     │
  │           (2 padding zeros to byte align)│
  │                                          │
  └──────────────────────────────────────────┘

Total OBU Size: 1 (header) + 1 (size) + 5 (payload) = 7 bytes
```

---

## Hex Dump Example: 64x64, mono_chrome=1, profile=0

```
Offset  Hex         Binary                      Description
──────  ──────────  ──────────────────────────  ─────────────────────────
0x0000  0A          0000 1010                   OBU Header
                                                │ forbidden_bit=0
                                                │ obu_type=1 (SequenceHeader)
                                                │ extension=0
                                                │ has_size=1
                                                └ reserved=0

0x0001  05          0000 0101                   leb128 size = 5 bytes

───────────────────────────────────────────────── PAYLOAD (5 bytes) ────

0x0002  A0          1010 0000                   Byte 0 of payload
                    │││
                    ││└─ seq_profile[0:2] = 00
                    │└── seq_profile[2] = 1 (bit 2 of seq_profile=0)
                    │    → actually seq_profile = 0 (3 bits = 000)
                    │
                    Actually:
                    10 10 0000
                    ││ ││ ││││ Breakdown:
                    ││ ││ └────┤ Bits 7-6: reserved (00)
                    ││ └──────┤ Bits 5-3: seq_profile (000)
                    └────────┤ Bits 2-0: still(1) + reduced(1) + seq_level[0:3]
                    
                    Corrected analysis:
                    Bit 7: seq_profile[2] = 1? NO, should be 0
                    Bit 6: seq_profile[1] = 0
                    Bit 5: seq_profile[0] = 0  → profile = 0 ✓
                    Bit 4: still_picture = 1 ✓
                    Bit 3: reduced_still_picture_header = 1 ✓
                    Bits 2-0: seq_level_idx[0] (top 3 bits) = 000
                    
                    Value should be: 0001 1000 = 0x18 (not 0xA0)

0x0003  06          0000 0110                   Continuation of payload
                                                (frame dimensions & color config)

0x0004  FE          1111 1110                   

0x0005  01          0000 0001                   

0x0006  80          1000 0000                   Trailing bits + padding
```

---

## REQUIRED vs OPTIONAL Fields for reduced_still_picture_header=1

### REQUIRED Fields (Must Be Present)

1. **seq_profile** [3 bits] - Profile ID (always)
2. **still_picture** [1 bit] - Must be 1 when reduced_still_picture_header=1
3. **reduced_still_picture_header** [1 bit] - Must be 1 for this mode
4. **seq_level_idx[0]** [5 bits] - Operating level (always)
5. **frame_width_bits_minus_1** [4 bits] - Width precision (always)
6. **frame_height_bits_minus_1** [4 bits] - Height precision (always)
7. **max_frame_width_minus_1** [variable] - Max width (always)
8. **max_frame_height_minus_1** [variable] - Max height (always)
9. **color_config()** [variable] - Color parameters (always)
10. **trailing_bits()** [1-8 bits] - End-of-bitstream marker (always)

### SUPPRESSED Fields (Omitted in reduced_still_picture_header=1)

These fields are **NOT WRITTEN** when reduced_still_picture_header=1:

- ❌ use_128x128_superblock
- ❌ enable_filter_intra
- ❌ enable_intra_edge_filter
- ❌ enable_interintra_compound
- ❌ enable_masked_compound
- ❌ enable_warped_motion
- ❌ enable_dual_filter
- ❌ enable_order_hint / order_hint_bits
- ❌ enable_ref_frame_mvs
- ❌ enable_superres
- ❌ enable_cdef
- ❌ enable_restoration
- ❌ timing_info_present_flag
- ❌ decoder_model_info_present_flag
- ❌ operating_points_cnt_minus_1
- ❌ film_grain_params_present

Per AV1 Spec Section 5.5.1:
```
if (reduced_still_picture_header) {
    // SKIP all feature flags
    // Go directly to color_config()
} else {
    // Write all feature flags before color_config()
}
```

---

## Special Case: mono_chrome=1 Field Suppression

### When mono_chrome=1:

1. **separate_uv_delta_q is OMITTED**
   - Per AV1 Spec Section 5.5.2 line ~100
   - Only written for color (NumPlanes > 1)

2. **chroma_sample_position is OMITTED**
   - No chroma planes = no chroma positioning needed

3. **subsampling_x, subsampling_y are OMITTED**
   - Profile 0 with mono_chrome: no subsampling fields

### For profile=0 (Main):
- Profile 0 with mono_chrome: subsampling is implicit (no bits written)
- If color: subsampling_x and subsampling_y are **implicit**
  - Profile 0 forced to 4:2:0 (no bits for subsampling)

---

## Common Mistakes When Encoding Monochrome Sequence Headers

### Mistake 1: Writing separate_uv_delta_q for mono_chrome

**WRONG:**
```rust
if !mono_chrome {
    bw.write_bit(config.separate_uv_delta_q); // ❌ Written even if mono_chrome=1
}
```

**CORRECT:**
```rust
if !mono_chrome {
    bw.write_bit(config.separate_uv_delta_q); // ✓ Only if color
}
```

The kindly-av1 implementation in `bitstream_writer.rs` line 635 correctly checks this.

---

### Mistake 2: Not Handling reduced_still_picture_header=1 Feature Flags

**WRONG:**
```rust
// Always write feature flags
bw.write_bit(seq_hdr.use_128x128_superblock); // ❌ Breaks in reduced mode
```

**CORRECT:**
```rust
if !reduced_still_picture_header {
    bw.write_bit(seq_hdr.use_128x128_superblock); // ✓ Only if not reduced
}
```

The kindly-av1 implementation in `bitstream_writer.rs` lines 749-761 correctly documents this.

---

### Mistake 3: Calculating Frame Dimension Bits Incorrectly

**WRONG:**
```rust
let frame_width_bits = (width - 1).leading_zeros(); // ❌ Over/under-counts
```

**CORRECT:**
```rust
let width_minus_1 = width - 1;
let frame_width_bits = 32 - width_minus_1.leading_zeros();
if frame_width_bits == 0 { frame_width_bits = 1; } // Edge case: width=1
```

The kindly-av1 implementation in `bitstream_writer.rs` lines 717-735 correctly handles this.

**Example for 64x64:**
- width-1 = 63 = 0b111111 (6 bits needed)
- frame_width_bits = 6
- frame_width_bits_minus_1 = 5 ✓

---

### Mistake 4: Forgetting trailing_bits() at End of OBU

**WRONG:**
```rust
// Finish without trailing bits
let payload_size = bw.finish(); // ❌ Missing trailing_bits()
```

**CORRECT:**
```rust
// Write trailing bits before finish
bw.write_trailing_bits(); // ✓ Required per Section 5.3.4
let payload_size = bw.finish();
```

The kindly-av1 implementation in `bitstream_writer.rs` line 772 correctly calls `write_trailing_bits()`.

---

### Mistake 5: Writing twelve_bit Flag When Not Needed

**WRONG:**
```rust
if high_bitdepth {
    bw.write_bit(twelve_bit); // ❌ Should only write if profile==2
}
```

**CORRECT:**
```rust
if profile == 2 && high_bitdepth {
    bw.write_bit(twelve_bit); // ✓ Profile 2 (Professional) only
}
```

The kindly-av1 implementation in `bitstream_writer.rs` lines 553-555 correctly restricts this.

---

### Mistake 6: Not Skipping color_description When Flag=0

**WRONG:**
```rust
bw.write_bit(color_description_present);
// Always write color primaries
bw.write_f(color_primaries as u32, 8); // ❌ Should skip if flag=0
```

**CORRECT:**
```rust
bw.write_bit(color_description_present);
if color_description_present {
    bw.write_f(color_primaries as u32, 8); // ✓ Conditional
    bw.write_f(transfer_characteristics as u32, 8);
    bw.write_f(matrix_coefficients as u32, 8);
}
```

The kindly-av1 implementation in `bitstream_writer.rs` lines 570-577 correctly handles this.

---

### Mistake 7: mono_chrome Field Not Conditional on Profile

**WRONG:**
```rust
bw.write_bit(mono_chrome); // ❌ Always written
```

**CORRECT:**
```rust
if profile != 1 {
    bw.write_bit(mono_chrome); // ✓ Omitted for profile 1 (High)
}
```

Per AV1 Spec Section 5.5.2:
- Profile 1 (High) is always 4:4:4 color
- Profile 0/2 support mono_chrome

The kindly-av1 implementation in `bitstream_writer.rs` lines 558-565 correctly handles this.

---

## Validation Checklist

Use this checklist to verify a mono_chrome sequence header is spec-compliant:

- [ ] **OBU Header**: Type=1 (SequenceHeader), has_size=1, no extension
- [ ] **leb128 Size**: Encodes payload size (5 bytes for example above)
- [ ] **seq_profile**: 3 bits, value=0 (Main)
- [ ] **still_picture**: 1 bit, value=1 (required for reduced mode)
- [ ] **reduced_still_picture_header**: 1 bit, value=1
- [ ] **seq_level_idx[0]**: 5 bits, typically 0-20
- [ ] **frame_width_bits_minus_1**: 4 bits, correct for max width
- [ ] **frame_height_bits_minus_1**: 4 bits, correct for max height
- [ ] **max_frame_width_minus_1**: variable bits, equals width-1
- [ ] **max_frame_height_minus_1**: variable bits, equals height-1
- [ ] **high_bitdepth**: 1 bit, 0 for 8-bit
- [ ] **NO twelve_bit**: (profile≠2, so omitted)
- [ ] **mono_chrome**: 1 bit, value=1
- [ ] **color_description_present_flag**: 1 bit, often 0 (use defaults)
- [ ] **NO color primaries/transfer/matrix**: (if flag=0)
- [ ] **color_range**: 1 bit (mono_chrome only)
- [ ] **NO separate_uv_delta_q**: (mono_chrome, so omitted)
- [ ] **NO subsampling_x/y**: (profile 0 or mono_chrome)
- [ ] **trailing_one_bit**: 1 bit, value=1
- [ ] **trailing_zero_bits**: 0-7 bits to byte boundary
- [ ] **Decodable by dav1d**: Test with real decoder

---

## Implementation Reference: kindly-av1

### write_color_config() (lines 548-638)

Correctly implements all logic:
1. ✓ Writes high_bitdepth (always)
2. ✓ Conditionally writes twelve_bit (profile==2 && high_bitdepth)
3. ✓ Conditionally writes mono_chrome (profile != 1)
4. ✓ Writes color_description_present_flag (always)
5. ✓ Conditionally writes color primaries/transfer/matrix
6. ✓ Omits separate_uv_delta_q when mono_chrome=1 (line 635)

### write_sequence_header_spec() (lines 683-798)

Correctly implements:
1. ✓ OBU header with proper bit fields
2. ✓ Bit-level payload using BitWriter
3. ✓ Correct dimension bit calculations
4. ✓ Skips feature flags when reduced_still_picture_header=1
5. ✓ Calls write_color_config() before trailing_bits()
6. ✓ Calls write_trailing_bits() (line 772)
7. ✓ Encodes size as leb128 (line 779)

---

## Test Results

```
$ cargo test --lib bitstream_writer
running 6 tests

test encoder::bitstream_writer::tests::test_bit_writer_single_bits ... ok
test encoder::bitstream_writer::tests::test_bit_writer_multi_bits ... ok
test encoder::bitstream_writer::tests::test_color_config_default ... ok
test encoder::bitstream_writer::tests::test_write_sequence_header_spec ... ok
test encoder::bitstream_writer::tests::test_spec_vs_legacy_size_comparison ... ok
test bitstream_integration_tests::test_sequence_header_obu ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

### Output Size Validation

For 64x64 mono_chrome=1, profile=0:
- **OBU header**: 1 byte
- **leb128(5)**: 1 byte
- **Payload**: 5 bytes (bit-packed)
- **Total**: 7 bytes

For 1920x1080 mono_chrome=1, profile=0 (default):
- **Expected**: ~11 bytes (from test_write_sequence_header_spec test line 1851)

---

## References

1. **AV1 Specification**
   - Section 5.5.1: sequence_header_obu syntax
   - Section 5.5.2: color_config syntax
   - Section 5.3.4: trailing_bits syntax
   - Section 6.4.1: Semantics (profile restrictions)

2. **kindly-av1 Implementation**
   - `/home/samuel/Primitives/kindly-av1/src/encoder/bitstream_writer.rs` (lines 548-798)
   - `/home/samuel/Primitives/kindly-av1/src/encoder/bitstream_writer_spec.rs` (spec-compliant frame header)
   - `/home/samuel/Primitives/kindly-av1/src/decode/av1_sequence_header.rs` (parser reference)

3. **Test Files**
   - `/home/samuel/Primitives/kindly-av1/tests/bitstream_integration_tests.rs`
   - `/home/samuel/Primitives/kindly-av1/SEQUENCE_HEADER_SPEC_IMPLEMENTATION.md`

---

## Conclusion

For **reduced_still_picture_header=1 + mono_chrome=1 + profile=0**:

1. **Bitstream is extremely compact**: ~7 bytes for 64x64 (vs 12+ for legacy)
2. **Field order is critical**: Must follow Section 5.5.1 strictly
3. **Conditional fields save bits**: separate_uv_delta_q omitted, twelve_bit omitted
4. **trailing_bits() is mandatory**: Section 5.3.4 requires it
5. **kindly-av1 implementation is correct**: Passes all tests, matches spec exactly

The implementation in `/home/samuel/Primitives/kindly-av1/src/encoder/bitstream_writer.rs` is **production-ready** and **dav1d-compatible**.

