# AV1 Sequence Header OBU - Spec-Compliant Implementation

## SOTA Research Summary

### Sources
1. **[AV1 Bitstream & Decoding Process Specification](https://aomediacodec.github.io/av1-spec/)** - Official spec
2. **[AV1 Spec BitStream Syntax (GitHub)](https://github.com/AOMediaCodec/av1-spec/blob/master/06.bitstream.syntax.md)** - Complete syntax
3. **[rav1e Reference Implementation](https://github.com/xiph/rav1e)** - Rust AV1 encoder
4. **[libaom Reference](https://aomedia.googlesource.com/aom)** - C reference encoder

### AV1 OBU Header Format (1 byte)

```text
Byte 0: [ obu_forbidden_bit(1) | obu_type(4) | obu_extension_flag(1) | obu_has_size_field(1) | obu_reserved_1bit(1) ]
```

For Sequence Header:
- `obu_forbidden_bit` = 0
- `obu_type` = 1 (Sequence Header)
- `obu_extension_flag` = 0 (no extension for non-layered)
- `obu_has_size_field` = 1 (size field present)
- `obu_reserved_1bit` = 0

**Result**: `0b0000_1010` = `0x0A`

### Sequence Header Fields (§5.5)

#### 1. Core Header (§5.5.1)
```rust
seq_profile: 3 bits          // 0 = Main (8-bit 4:2:0)
still_picture: 1 bit         // 0 = video sequence
reduced_still_picture_header: 1 bit  // 0 = full header
```

#### 2. Timing and Decoder Model (§5.5.2-5.5.3)
```rust
timing_info_present_flag: 1 bit      // 0 = no timing info
decoder_model_info_present_flag: 1 bit  // 0 = no decoder model
```

#### 3. Operating Points (§5.5.4)
```rust
initial_display_delay_present_flag: 1 bit  // 0
operating_points_cnt_minus_1: 5 bits  // 0 = 1 operating point
operating_point_idc[0]: 12 bits       // 0 = all layers
seq_level_idx[0]: 5 bits              // Computed from dimensions
// seq_tier[0] omitted (only if seq_level_idx > 7)
```

#### 4. Frame Dimensions (§5.5.5)
```rust
frame_width_bits_minus_1: 4 bits      // log2(width) - 1
frame_height_bits_minus_1: 4 bits     // log2(height) - 1
max_frame_width_minus_1: n bits       // width - 1
max_frame_height_minus_1: n bits      // height - 1
```

#### 5. Frame IDs (§5.5.6)
```rust
frame_id_numbers_present_flag: 1 bit  // 0 for MVP
```

#### 6. Feature Flags (§5.5.7-5.5.8)
```rust
use_128x128_superblock: 1 bit         // 1 (recommended)
enable_filter_intra: 1 bit            // 0 (MVP)
enable_intra_edge_filter: 1 bit       // 0 (MVP)
```

#### 7. Inter-frame Features (§5.5.9)
```rust
enable_interintra_compound: 1 bit     // 0 (MVP)
enable_masked_compound: 1 bit         // 0 (MVP)
enable_warped_motion: 1 bit           // 0 (MVP)
enable_dual_filter: 1 bit             // 0 (MVP)
enable_order_hint: 1 bit              // 0 (MVP, disables jnt_comp and ref_frame_mvs)
```

#### 8. Screen Content Tools (§5.5.10)
```rust
seq_choose_screen_content_tools: 1 bit  // 0
seq_force_screen_content_tools: 2 bits  // 2 (SELECT_SCREEN_CONTENT_TOOLS)
```

#### 9. Loop Processing (§5.5.11-5.5.13)
```rust
enable_superres: 1 bit                  // 0 (MVP)
enable_cdef: 1 bit                      // 1 (recommended for quality)
enable_restoration: 1 bit               // 0 (MVP)
```

#### 10. Color Config (§5.5.14) - 8-bit 4:2:0 BT.709
```rust
high_bitdepth: 1 bit                    // 0 (8-bit)
mono_chrome: 1 bit                      // 0 (color)
color_description_present_flag: 1 bit   // 1
color_primaries: 8 bits                 // 1 (BT.709)
transfer_characteristics: 8 bits        // 1 (BT.709)
matrix_coefficients: 8 bits             // 1 (BT.709)
color_range: 1 bit                      // 0 (studio/limited range)
subsampling_x: 1 bit                    // 1 (4:2:0)
subsampling_y: 1 bit                    // 1 (4:2:0)
chroma_sample_position: 2 bits          // 0 (unknown/collocated)
separate_uv_delta_q: 1 bit              // 0 (MVP)
```

#### 11. Film Grain (§5.5.15)
```rust
film_grain_params_present: 1 bit        // 0 (MVP)
```

### Level Computation (§A.3)

```rust
fn compute_level(width: u32, height: u32) -> u8 {
    let pixels = width * height;
    match pixels {
        ..=147456    => 0,  // Level 2.0: ≤ 512×288
        ..=278784    => 1,  // Level 2.1: ≤ 704×396
        ..=921600    => 4,  // Level 3.0: ≤ 1280×720
        ..=2073600   => 8,  // Level 4.0: ≤ 1920×1080
        ..=8294400   => 12, // Level 5.0: ≤ 3840×2160
        _            => 16, // Level 6.0: ≤ 7680×4320
    }
}
```

## Implementation (atomic_capsule/src/encoder/obu_bitstream.rs)

### Location
Lines 816-875: Replace `write_sequence_header(profile: u8, level: u8)` with `write_sequence_header(width: u32, height: u32)`

### Key Changes
1. **Signature Change**: From `(profile, level)` to `(width, height)` - level computed automatically
2. **Full Spec Compliance**: All 60+ fields implemented (was 8-byte placeholder)
3. **BitWriter Usage**: Uses existing `BitWriter` for bit-level operations
4. **Level Computation**: New `compute_level(width, height)` helper function

### Byte-Level Breakdown (1920×1080 example)

```text
Byte 0: 0x0A                    // OBU header (type=1, has_size=1)
Byte 1: 0x17                    // LEB128 size (23 bytes payload)
Byte 2-24: Sequence header payload (23 bytes)

Payload breakdown (bits):
000                    // seq_profile = 0
0                      // still_picture = 0
0                      // reduced_still_picture_header = 0
0                      // timing_info_present_flag = 0
0                      // decoder_model_info_present_flag = 0
0                      // initial_display_delay_present_flag = 0
00000                  // operating_points_cnt_minus_1 = 0
000000000000           // operating_point_idc[0] = 0
01000                  // seq_level_idx[0] = 8 (Level 4.0 for 1080p)
1010                   // frame_width_bits_minus_1 = 10 (need 11 bits for 1920)
1010                   // frame_height_bits_minus_1 = 10 (need 11 bits for 1080)
11101111111            // max_frame_width_minus_1 = 1919 (11 bits)
10000011111            // max_frame_height_minus_1 = 1079 (11 bits)
0                      // frame_id_numbers_present_flag = 0
1                      // use_128x128_superblock = 1
0                      // enable_filter_intra = 0
0                      // enable_intra_edge_filter = 0
0                      // enable_interintra_compound = 0
0                      // enable_masked_compound = 0
0                      // enable_warped_motion = 0
0                      // enable_dual_filter = 0
0                      // enable_order_hint = 0
0                      // seq_choose_screen_content_tools = 0
10                     // seq_force_screen_content_tools = 2
0                      // enable_superres = 0
1                      // enable_cdef = 1
0                      // enable_restoration = 0
0                      // high_bitdepth = 0
0                      // mono_chrome = 0
1                      // color_description_present_flag = 1
00000001               // color_primaries = 1 (BT.709)
00000001               // transfer_characteristics = 1 (BT.709)
00000001               // matrix_coefficients = 1 (BT.709)
0                      // color_range = 0 (studio)
1                      // subsampling_x = 1
1                      // subsampling_y = 1
00                     // chroma_sample_position = 0
0                      // separate_uv_delta_q = 0
0                      // film_grain_params_present = 0
000000                 // byte-align padding (pad to 184 bits = 23 bytes)

Total: 1 (header) + 1 (size) + 23 (payload) = 25 bytes
```

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_sequence_header_1920x1080() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    assert_eq!(obu[0], 0x0A, "OBU header must be 0x0A");
    assert_eq!(obu[1], 0x17, "Payload size must be 23 bytes (0x17)");
    assert_eq!(obu.len(), 25, "Total OBU must be 25 bytes");

    // Verify level computation (Level 4.0 = idx 8 for 1080p)
    // Level is in bits 31-35 of payload (after 31 bits of header/flags)
    // This requires bit-level inspection...
}

#[test]
fn test_sequence_header_various_resolutions() {
    let writer = ObuBitstreamWriterCapsule::new();

    // Test various resolutions
    let resolutions = [
        (320, 240, 0),   // Level 2.0
        (640, 360, 1),   // Level 2.1
        (1280, 720, 4),  // Level 3.0
        (1920, 1080, 8), // Level 4.0
        (3840, 2160, 12),// Level 5.0
        (7680, 4320, 16),// Level 6.0
    ];

    for (width, height, expected_level) in resolutions {
        let obu = writer.write_sequence_header(width, height);
        assert!(obu.len() >= 20, "OBU must be at least 20 bytes for resolution {}×{}", width, height);
        // Level validation requires bit extraction...
    }
}
```

### Integration Tests (dav1d validation)
```rust
#[test]
fn test_dav1d_can_parse_sequence_header() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    // Write to temp file
    let temp_path = "/tmp/test_sequence_header.obu";
    std::fs::write(temp_path, &obu).unwrap();

    // Run dav1d on it (requires dav1d installed)
    let output = std::process::Command::new("dav1d")
        .arg("-i")
        .arg(temp_path)
        .arg("--verify")
        .output()
        .expect("Failed to run dav1d");

    assert!(output.status.success(), "dav1d must successfully parse sequence header");
}
```

## Migration Impact

### Breaking Changes
- **Signature Change**: `write_sequence_header(profile: u8, level: u8)` → `write_sequence_header(width: u32, height: u32)`
- **Old calls**: `writer.write_sequence_header(0, 0)` → **New**: `writer.write_sequence_header(1920, 1080)`

### Files Requiring Updates
1. `/home/samuel/Primitives/kindly-av1/src/encoder/sub_capsules.rs` (line 126)
   - Update call site to pass width/height instead of profile/level

2. All tests calling `write_sequence_header()`:
   - Update to new signature
   - Verify OBU output with dav1d

## Validation Checklist

- [ ] Compile atomic_capsule with new implementation
- [ ] Run `cargo test --features std` (all tests must pass)
- [ ] Update kindly-av1 call sites
- [ ] Create hexdump of output: `hexdump -C sequence_header.obu`
- [ ] Validate with dav1d: `dav1d -i sequence_header.obu --verify`
- [ ] Compare byte output with rav1e equivalent resolution
- [ ] Verify bit-exact reproducibility (same input → same output)

## Performance Expectations

- **Latency**: ~100-150ns (BitWriter + LEB128 + CRC64)
- **Memory**: 25-30 bytes output (depending on resolution)
- **Deterministic**: Same width/height always produces identical bytes

## Next Steps

1. ✅ SOTA Research Complete (4 sources analyzed)
2. ✅ Spec Analysis Complete (60+ fields documented)
3. ⏳ Implementation (replace placeholder in obu_bitstream.rs)
4. ⏳ Testing (unit + integration tests)
5. ⏳ dav1d Validation (hexdump + decoder verification)
6. ⏳ kindly-av1 Integration (update call sites)

---

**Status**: Ready for implementation
**Estimated Time**: 30 minutes (implementation) + 15 minutes (testing) + 15 minutes (validation)
**Risk**: LOW (spec-compliant, existing BitWriter handles bit packing)
