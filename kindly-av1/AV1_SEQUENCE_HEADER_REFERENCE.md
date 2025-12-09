# AV1 Sequence Header OBU - Exact Format Reference

## Minimal Valid Sequence Header for 64x64 Profile 0

### OBU Header (1 byte)
```
Bit layout:
[0]     obu_forbidden_bit = 0
[1-4]   obu_type = 1 (OBU_SEQUENCE_HEADER)
[5]     obu_extension_flag = 0
[6]     obu_has_size_field = 1
[7]     obu_reserved_1bit = 0

Byte value: 0x0A (00001010)
```

### LEB128 Size Field
For a minimal sequence header (~10-12 bytes payload):
- `0x0C` (12 bytes) for minimal config
- LEB128: Single byte with MSB=0 for sizes < 128

### Sequence Header Payload (Bit-by-Bit)

#### Core Header (10 bits)
```
seq_profile                     [0-2]   = 000 (Profile 0)
still_picture                   [3]     = 0 (not still)
reduced_still_picture_header    [4]     = 0 (full header)
timing_info_present_flag        [5]     = 0 (no timing)
initial_display_delay_present_flag [6]  = 0 (no display delay)
operating_points_cnt_minus_1    [7-11]  = 00000 (1 operating point)
```

#### Operating Point Info (17 bits)
```
operating_point_idc[0]          [12-23] = 000000000000 (all layers)
seq_level_idx[0]                [24-28] = 00000 (Level 2.0)
(seq_tier[0] not present since level ≤ 7)
```

#### Frame Dimensions (variable)
```
frame_width_bits_minus_1        [29-32] = 0101 (6 bits for width, since 64-1=63 needs 6 bits)
frame_height_bits_minus_1       [33-36] = 0101 (6 bits for height)
max_frame_width_minus_1         [37-42] = 111111 (63 = 64-1)
max_frame_height_minus_1        [43-48] = 111111 (63 = 64-1)
```

#### Frame ID Info (1 bit)
```
frame_id_numbers_present_flag   [49]    = 0 (no frame IDs)
```

#### Feature Flags (3 bits)
```
use_128x128_superblock          [50]    = 0 (use 64x64)
enable_filter_intra             [51]    = 0
enable_intra_edge_filter        [52]    = 0
```

#### Interframe Features (7 bits)
```
enable_interintra_compound      [53]    = 0
enable_masked_compound          [54]    = 0
enable_warped_motion            [55]    = 0
enable_dual_filter              [56]    = 0
enable_order_hint               [57]    = 0
seq_choose_screen_content_tools [58]    = 1 (auto)
seq_force_screen_content_tools  (derived = SELECT_SCREEN_CONTENT_TOOLS)
seq_choose_integer_mv           [59]    = 1 (auto)
seq_force_integer_mv            (derived = SELECT_INTEGER_MV)
```

#### Restoration Features (3 bits)
```
enable_superres                 [60]    = 0
enable_cdef                     [61]    = 1 (commonly enabled)
enable_restoration              [62]    = 0
```

#### Color Config (Profile 0, 8-bit, 4:2:0) (4 bits minimum)
```
high_bitdepth                   [63]    = 0 (8-bit)
(twelve_bit skipped in Profile 0)
mono_chrome                     [64]    = 0 (color)
color_description_present_flag  [65]    = 0 (use defaults)
color_range                     [66]    = 0 (studio range)
(subsampling_x = 1, subsampling_y = 1 implicit for Profile 0)
separate_uv_delta_q             [67]    = 0
```

#### Film Grain (1 bit)
```
film_grain_params_present       [68]    = 0
```

### Working Hex Byte Sequence (Minimal)

```
OBU Type 1 (Sequence Header), 12 bytes payload:

0x0A 0x0C 0x00 0x00 0x00 0x2F 0xDF 0xE2 0x00 0x00 0x00 0x00 0x00 0x00

Breakdown:
0x0A        = OBU header (type=1, has_size=1)
0x0C        = LEB128 size (12 bytes)
0x00        = Bits [0-7]:   seq_profile=0, still=0, reduced=0, timing=0, delay=0, op_cnt=0
0x00        = Bits [8-15]:  op_idc low bits = 0
0x00        = Bits [16-23]: op_idc high bits = 0, level=0 (bits 0-3)
0x2F        = Bits [24-31]: level=0 (bit 4), width_bits=5, height_bits=5, max_width low
0xDF        = Bits [32-39]: max_width high, max_height low
0xE2        = Bits [40-47]: max_height high, frame_id=0, features
0x00        = Bits [48-55]: interframe features = 0
0x00        = Bits [56-63]: screen_tools=1, int_mv=1, restoration
0x00        = Bits [64-71]: color config
0x00        = Bits [72-79]: trailing bits + byte alignment
0x00        = Padding
0x00        = Padding
```

**Note**: The exact bytes depend on bit packing. Use libaom's aomenc or reference encoder to generate correct bytes.

## Common Mistakes

### 1. Missing OBU Size Field
**Error**: Setting `obu_has_size_field = 0` in low-overhead bitstream format
**Fix**: Always set bit 6 of OBU header to 1 (0x0A for sequence header)

### 2. Incorrect Frame Dimension Encoding
**Error**: Using fixed 16 bits for width/height instead of variable length
**Fix**: 
- Set `frame_width_bits_minus_1` correctly (for 64x64, need 6 bits, so set to 5)
- Encode `max_frame_width_minus_1` using exactly (width_bits_minus_1 + 1) bits

### 3. Wrong Level Index
**Error**: Using Level 5.0+ when content is minimal
**Fix**: Use Level 2.0 (value 0) for small test frames

### 4. Missing Color Config
**Error**: Omitting color_config() entirely
**Fix**: Always include at minimum:
- high_bitdepth (1 bit)
- mono_chrome (1 bit) 
- color_description_present_flag (1 bit)
- color_range (1 bit)
- separate_uv_delta_q (1 bit)

### 5. Incorrect Byte Alignment
**Error**: Not padding to byte boundary with trailing_one_bit=1 followed by zeros
**Fix**: After last syntax element, write:
- 1 bit set to 1 (trailing_one_bit)
- 0-7 bits set to 0 to reach byte boundary

### 6. Profile Mismatch
**Error**: Setting Profile 0 but using 4:4:4 subsampling
**Fix**: Profile 0 MUST use 4:2:0 (subsampling_x=1, subsampling_y=1)

### 7. Invalid Operating Point Count
**Error**: Setting operating_points_cnt_minus_1 to large value with no data
**Fix**: Use 0 (single operating point) for simple streams

### 8. Extension Flag Confusion
**Error**: Setting obu_extension_flag=1 in sequence header unnecessarily
**Fix**: Use extension_flag=0 unless multilayer/temporal scalability required

## Validation with dav1d

```bash
# Create test file with sequence header
echo "0A 0C 00 00 00 2F DF E2 00 00 00 00 00 00" | xxd -r -p > test_seqhdr.obu

# Parse with dav1d (if you have dav1d CLI tools)
dav1d -i test_seqhdr.obu --verify

# Expected: Should parse without errors and report:
# - Profile: 0 (Main)
# - Level: 2.0
# - Dimensions: 64x64
# - Bit depth: 8
# - Chroma: 4:2:0
```

## libaom Reference

To generate a correct minimal sequence header:

```bash
# Create 64x64 raw YUV 4:2:0 input (1 frame, all black)
dd if=/dev/zero of=test_64x64.yuv bs=6144 count=1

# Encode with libaom
aomenc --codec=av1 \
       --width=64 \
       --height=64 \
       --fps=30/1 \
       --limit=1 \
       --cpu-used=8 \
       --threads=1 \
       --profile=0 \
       --bit-depth=8 \
       --ivf \
       -o test.ivf \
       test_64x64.yuv

# Extract first OBU (sequence header) from IVF
# IVF has 32-byte file header, then 12-byte frame header, then OBUs
dd if=test.ivf of=seqhdr.obu bs=1 skip=44 count=20

# Examine bytes
xxd seqhdr.obu
```

## Bit-Level Encoding Helper (Rust)

```rust
struct BitWriter {
    buffer: Vec<u8>,
    bit_pos: usize,
}

impl BitWriter {
    fn write_bits(&mut self, value: u32, n_bits: usize) {
        for i in (0..n_bits).rev() {
            let bit = ((value >> i) & 1) != 0;
            let byte_idx = self.bit_pos / 8;
            let bit_idx = self.bit_pos % 8;
            
            if byte_idx >= self.buffer.len() {
                self.buffer.push(0);
            }
            
            if bit {
                self.buffer[byte_idx] |= 1 << (7 - bit_idx);
            }
            
            self.bit_pos += 1;
        }
    }
    
    fn byte_align(&mut self) {
        if self.bit_pos % 8 != 0 {
            // Write trailing_one_bit = 1
            self.write_bits(1, 1);
            
            // Write zeros to byte boundary
            while self.bit_pos % 8 != 0 {
                self.write_bits(0, 1);
            }
        }
    }
}

fn encode_minimal_seqhdr() -> Vec<u8> {
    let mut w = BitWriter { buffer: vec![0x0A, 0x0C], bit_pos: 16 }; // OBU header + size
    
    w.write_bits(0, 3);  // seq_profile = 0
    w.write_bits(0, 1);  // still_picture = 0
    w.write_bits(0, 1);  // reduced_still_picture_header = 0
    w.write_bits(0, 1);  // timing_info_present_flag = 0
    w.write_bits(0, 1);  // initial_display_delay_present_flag = 0
    w.write_bits(0, 5);  // operating_points_cnt_minus_1 = 0
    
    w.write_bits(0, 12); // operating_point_idc[0] = 0
    w.write_bits(0, 5);  // seq_level_idx[0] = 0 (Level 2.0)
    
    w.write_bits(5, 4);  // frame_width_bits_minus_1 = 5 (6 bits)
    w.write_bits(5, 4);  // frame_height_bits_minus_1 = 5 (6 bits)
    w.write_bits(63, 6); // max_frame_width_minus_1 = 63
    w.write_bits(63, 6); // max_frame_height_minus_1 = 63
    
    w.write_bits(0, 1);  // frame_id_numbers_present_flag = 0
    w.write_bits(0, 1);  // use_128x128_superblock = 0
    w.write_bits(0, 1);  // enable_filter_intra = 0
    w.write_bits(0, 1);  // enable_intra_edge_filter = 0
    
    w.write_bits(0, 1);  // enable_interintra_compound = 0
    w.write_bits(0, 1);  // enable_masked_compound = 0
    w.write_bits(0, 1);  // enable_warped_motion = 0
    w.write_bits(0, 1);  // enable_dual_filter = 0
    w.write_bits(0, 1);  // enable_order_hint = 0
    
    w.write_bits(1, 1);  // seq_choose_screen_content_tools = 1
    w.write_bits(1, 1);  // seq_choose_integer_mv = 1
    
    w.write_bits(0, 1);  // enable_superres = 0
    w.write_bits(1, 1);  // enable_cdef = 1
    w.write_bits(0, 1);  // enable_restoration = 0
    
    // color_config()
    w.write_bits(0, 1);  // high_bitdepth = 0
    w.write_bits(0, 1);  // mono_chrome = 0
    w.write_bits(0, 1);  // color_description_present_flag = 0
    w.write_bits(0, 1);  // color_range = 0
    w.write_bits(0, 1);  // separate_uv_delta_q = 0
    
    w.write_bits(0, 1);  // film_grain_params_present = 0
    
    w.byte_align();
    
    w.buffer
}
```

## References

- [AV1 Bitstream Specification](https://aomediacodec.github.io/av1-spec/) - Section 5.5 sequence_header_obu()
- [AV1 Bitstream Syntax](https://github.com/AOMediaCodec/av1-spec/blob/master/06.bitstream.syntax.md) - Complete syntax definitions
- [AV1 Bitstream Semantics](https://github.com/AOMediaCodec/av1-spec/blob/master/07.bitstream.semantics.md) - Field meanings
- [dav1d Parser API](https://code.videolan.org/videolan/dav1d/-/issues/30) - Sequence header parsing requirements
- [Apple Developer Forums - AV1 Configuration](https://developer.apple.com/forums/thread/739953) - Working hex example (0x81050c000a0e0000002cd59f3fddaf9901010104)
