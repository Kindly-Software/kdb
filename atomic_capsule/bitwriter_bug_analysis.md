# BitWriter Bug Analysis - Root Cause Found

## The Bug

**Location**: `/home/samuel/Primitives/atomic_capsule/src/encoder/obu_bitstream.rs`, line 216

```rust
let shift = 64 - self.bit_position as u32 - n as u32;
```

## Root Cause: Bit Position Alignment

The bug is in the **shift calculation** for MSB-first bit packing. Let me trace through with a concrete example:

### Example: Writing seq_level_idx = 1 (5 bits) at bit position 25

**Setup**:
- Previous writes filled bits 0-24 (3 complete bytes in accumulator)
- `bit_position = 1` (1 bit remaining in accumulator after flushing 3 bytes)
- We want to write `n = 5` bits with `value = 1` (binary: `00001`)

**Current Code** (lines 216-220):
```rust
let shift = 64 - self.bit_position as u32 - n as u32;
// shift = 64 - 1 - 5 = 58

self.accumulator |= masked_value << shift;
// accumulator |= 1 << 58
// This places bit at position 58-62 (from left)
```

**The Problem**: After flushing 3 bytes (24 bits), `bit_position = 1` doesn't mean "1 bit from the left", it means "1 bit used in the current accumulator". But the shift calculation treats it as if all 64 bits are available.

### Correct Bit Packing Logic

When `bit_position = 1`:
- Accumulator has 1 bit used (at MSB position 63)
- Remaining space: bits 0-62 (63 bits)
- Next write should start at position 1 (0-indexed from MSB)

**Correct shift**:
```rust
// To place n bits starting at bit_position
// MSB is bit 63, LSB is bit 0
// If bit_position = 1, next write starts at bit 62 (from right, 0-indexed)
// So shift should be: 64 - bit_position - n = 64 - 1 - 5 = 58 ✅
```

Wait, that's the same! Let me trace more carefully...

## Detailed Trace Through Sequence Header

Let's trace the ACTUAL values being written for the first 30 bits:

### Reference Expected Output:
```
Bit stream: 000 0 0 0 0 0 00000 000000000000 00001 0101 0101
            ^seq ^st^rd^ti^de^in^op_cnt  ^op_idc(12)  ^lvl ^wbits
            0-2  3  4  5  6  7  8-12      13-24        25-29 30-33
```

### Our Code Execution:

**Write 1**: `writer.write_bits(3, 0);` - seq_profile = 0
- n=3, value=0, bit_position=0
- shift = 64 - 0 - 3 = 61
- accumulator = 0 << 61 = 0
- bit_position = 3
- Output: `000` at bits 0-2 ✅

**Write 2**: `writer.write_bits(1, 0);` - still_picture = 0
- n=1, value=0, bit_position=3
- shift = 64 - 3 - 1 = 60
- accumulator = 0 | (0 << 60) = 0
- bit_position = 4
- Output: `0` at bit 3 ✅

**Writes 3-8**: All zeros (5 more bits, total 8 bits)
- After write 8: bit_position = 8
- Flush triggered! `self.accumulator >> 56` gives first byte
- Byte 0 = 0x00 ✅
- accumulator <<= 8, bit_position -= 8, so bit_position = 0

**Write 9**: `writer.write_bits(5, 0);` - operating_points_cnt_minus_1 = 0
- n=5, value=0, bit_position=0
- shift = 64 - 0 - 5 = 59
- accumulator = 0 << 59 = 0
- bit_position = 5
- Output: `00000` at bits 8-12 ✅

**Write 10**: `writer.write_bits(12, 0);` - operating_point_idc[0] = 0
- n=12, value=0, bit_position=5
- shift = 64 - 5 - 12 = 47
- accumulator = 0 | (0 << 47) = 0
- bit_position = 17
- After 8 more bits (bit_position >= 8):
  - Flush byte 1 = 0x00 ✅
  - accumulator <<= 8, bit_position = 9
- After 8 more bits:
  - Flush byte 2 = 0x00 ✅
  - accumulator <<= 8, bit_position = 1
- Output: 12 zero bits at positions 13-24 ✅

**Write 11**: `writer.write_bits(5, level_idx);` - seq_level_idx[0] = 1
- **EXPECTED**: level_idx = 1
- **ACTUAL FROM OUTPUT**: Let's decode our output to see what was written

## Decoding Our Actual Output

Our output: `00 00 00 05 57 ff c0 01 00 00`

Convert to binary:
```
Byte 3: 0x05 = 0000_0101
Byte 4: 0x57 = 0101_0111
```

Bits 24-29 (seq_level_idx):
```
Byte 2 (bits 16-23): 0x00 = 0000_0000 (last bit of op_idc is bit 24)
Byte 3 (bits 24-31): 0x05 = 0000_0101
```

Wait, I need to account for the flushing...

After writing 24 bits (op_idc complete):
- 3 bytes flushed: 0x00, 0x00, 0x00
- bit_position = 1 (1 bit remaining: the last bit of op_idc, which is 0)
- accumulator has 1 bit at MSB position

**Write 11**: `writer.write_bits(5, 1);` - seq_level_idx = 1
- n=5, value=1 (binary: 00001)
- bit_position=1
- shift = 64 - 1 - 5 = 58
- masked_value = 1 & 0x1F = 1
- accumulator |= 1 << 58
- bit_position = 6

So the accumulator now has:
- Bit 63: 0 (last bit of op_idc)
- Bits 58-62: 00001 (seq_level_idx = 1)
- Remaining bits: unused

Binary representation (MSB to LSB):
```
Accumulator (bits 63-56, MSB first):
0 00001 XX = 0000_01XX
           = 0x04 or 0x05 depending on X values
```

But our output shows byte 3 = 0x05 = `0000_0101`, which suggests:
```
Bit 63: 0 (op_idc last bit)
Bits 58-62: 00001 (level_idx = 1) ✅
Bits 56-57: 01 (next 2 bits from frame_width_bits_minus_1)
```

**This is CORRECT!** The level_idx = 1 is correctly encoded.

So where's the problem? Let me check byte 4:

**Write 12**: `writer.write_bits(4, (width_bits - 1));` - frame_width_bits_minus_1 = 5
- For width=64, max_width_minus_1=63, width_bits=6, so value=5 (binary: 0101)
- bit_position=6 (after writing level_idx)
- shift = 64 - 6 - 4 = 54
- accumulator |= 5 << 54
- bit_position = 10
- Trigger flush when bit_position >= 8:
  - Flush byte 3 = (accumulator >> 56) as u8
  - accumulator <<= 8, bit_position = 2

Let's calculate byte 3:
```
Before flush:
Bit 63: 0 (op_idc last)
Bits 58-62: 00001 (level_idx=1)
Bits 54-57: 0101 (width_bits_minus_1=5)
Bits 0-53: zeros

accumulator >> 56 extracts bits 56-63:
= 0_00001_01 = 0x0A ❌

But our output shows 0x05, not 0x0A!
```

**FOUND THE BUG!**

The problem is that `compute_level_index(64, 64)` is returning the wrong value, OR there's a bug in how we're calling the sequence header function.

Let me check what parameters are being passed to `write_sequence_header_spec_compliant`:

## Hypothesis: Wrong Dimensions Passed

Looking at our output `05 57`, let me decode backwards:

Byte 3: `0x05 = 0000_0101`
```
Bit 7 (bit 24 global): 0 (op_idc last bit)
Bits 2-6 (bits 25-29 global): 00001 (level_idx = 1) ✅
Bits 0-1 (bits 30-31 global): 01 (width_bits first 2 bits)
```

Byte 4: `0x57 = 0101_0111`
```
Bits 6-7 (bits 32-33): 01 (width_bits last 2 bits)
Complete width_bits_minus_1 = 0101 = 5 ✅
Bits 2-5 (bits 34-37): 0111 = 7 (height_bits_minus_1) ❌
```

**CRITICAL**: `height_bits_minus_1 = 7` means we're encoding height with **8 bits**, allowing values up to 255!

This means `bits_needed(height - 1)` returned 8, which means `height - 1 >= 128`.

**CONCLUSION**: The function is being called with **height > 128**, not height = 64!

## The Real Bug

The bug is NOT in BitWriter. The bug is that `write_sequence_header_spec_compliant` is being called with the **wrong height parameter**.

Looking at byte 5-7 to decode the actual height encoded:
```
Bytes 5-7: 0xff 0xc0 0x01

After width (6 bits starting at bit 38):
Bit 38-43 (6 bits from bytes 4-5): Need to extract carefully

Byte 4 bits 0-1: 11 (from 0x57 = 0101_0111, last 2 bits)
Byte 5 bits 0-3: 1111 (from 0xff = 1111_1111, first 4 bits)
width = 11_1111 = 63 ✅ (64-1)

height (8 bits starting at bit 44):
Byte 5 bits 4-7: 1111 (from 0xff)
Byte 6 bits 0-3: 1100 (from 0xc0 = 1100_0000, first 4 bits)
height = 1111_1100 = 252 (so actual height = 253)
```

## Root Cause: Dimensions Mismatch

**HYPOTHESIS CONFIRMED**: Our encoder is being called with dimensions approximately **64 × 253**, NOT 64 × 64!

## Fix Required

1. **Verify the dimensions** passed to `write_sequence_header_spec_compliant(width, height)`
2. **Ensure** caller passes `(64, 64)` not `(64, 253)`
3. **Add assertion** in function to validate dimensions match expected test case

The BitWriter implementation is CORRECT. The bug is in how the function is being called or how dimensions are calculated before calling it.
