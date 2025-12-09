# AV1 Sequence Header Bit-by-Bit Analysis
## Reference vs Our Encoder (64×64 frame)

**Reference (libaom)**: `00 00 00 02 af ff 9b 5f 20 08` (10 bytes)
**Our Encoder**:     `00 00 00 05 57 ff c0 01 00 00` (10 bytes)

---

## Binary Representation (MSB-first, AV1 standard)

### Reference (libaom):
```
Byte 0: 0x00 = 0000_0000
Byte 1: 0x00 = 0000_0000
Byte 2: 0x00 = 0000_0000
Byte 3: 0x02 = 0000_0010
Byte 4: 0xaf = 1010_1111
Byte 5: 0xff = 1111_1111
Byte 6: 0x9b = 1001_1011
Byte 7: 0x5f = 0101_1111
Byte 8: 0x20 = 0010_0000
Byte 9: 0x08 = 0000_1000
```

**Full bit stream** (MSB-first):
```
00000000 00000000 00000000 00000010 10101111 11111111 10011011 01011111 00100000 00001000
```

### Our Encoder:
```
Byte 0: 0x00 = 0000_0000
Byte 1: 0x00 = 0000_0000
Byte 2: 0x00 = 0000_0000
Byte 3: 0x05 = 0000_0101
Byte 4: 0x57 = 0101_0111
Byte 5: 0xff = 1111_1111
Byte 6: 0xc0 = 1100_0000
Byte 7: 0x01 = 0000_0001
Byte 8: 0x00 = 0000_0000
Byte 9: 0x00 = 0000_0000
```

**Full bit stream** (MSB-first):
```
00000000 00000000 00000000 00000101 01010111 11111111 11000000 00000001 00000000 00000000
```

---

## Bit-by-Bit Decoding (AV1 §5.5)

### Bits 0-31: OBU Header + Size (First 4 bytes)

Both sequences start with `00 00 00`, diverging at byte 3:
- **Reference**: `0x02` (sequence header size = 2 bytes using LEB128? Actually this is odd...)
- **Our encoder**: `0x05` (sequence header size = 5 bytes)

**WAIT**: The first bytes are the OBU header + LEB128 size. Let me re-analyze:

#### OBU Header Structure (§5.3.2):
- Bit 0: forbidden bit (0)
- Bits 1-4: obu_type (4 bits)
- Bit 5: obu_extension_flag (0 or 1)
- Bit 6: obu_has_size_field (0 or 1)
- Bit 7: obu_reserved_1bit (0)

#### Reference OBU Header Analysis:
```
Byte 0: 0x00 = 0000_0000
  - forbidden: 0
  - obu_type: 0000 (but wait, sequence header should be type 1)
```

**PROBLEM DETECTED**: The reference starts with `00 00 00 02`, which doesn't match expected OBU structure. This suggests the byte stream might be the **payload only** (without OBU header/size).

Let me re-interpret as **payload only**:

---

## Re-Analysis: Assuming Payload Only (No OBU Header)

### Reference Payload (libaom):
```
Byte 0: 0x00 = 0000_0000
Byte 1: 0x00 = 0000_0000
Byte 2: 0x00 = 0000_0000
Byte 3: 0x02 = 0000_0010
Byte 4: 0xaf = 1010_1111
Byte 5: 0xff = 1111_1111
Byte 6: 0x9b = 1001_1011
Byte 7: 0x5f = 0101_1111
Byte 8: 0x20 = 0010_0000
Byte 9: 0x08 = 0000_1000
```

**Bit stream (MSB-first, bit positions 0-79)**:
```
Position: 0         8         16        24        32        40        48        56        64        72
          |         |         |         |         |         |         |         |         |         |
Bits:     00000000  00000000  00000000  00000010  10101111  11111111  10011011  01011111  00100000  00001000
```

### Our Encoder Payload:
```
Byte 0: 0x00 = 0000_0000
Byte 1: 0x00 = 0000_0000
Byte 2: 0x00 = 0000_0000
Byte 3: 0x05 = 0000_0101
Byte 4: 0x57 = 0101_0111
Byte 5: 0xff = 1111_1111
Byte 6: 0xc0 = 1100_0000
Byte 7: 0x01 = 0000_0001
Byte 8: 0x00 = 0000_0000
Byte 9: 0x00 = 0000_0000
```

**Bit stream (MSB-first, bit positions 0-79)**:
```
Position: 0         8         16        24        32        40        48        56        64        72
          |         |         |         |         |         |         |         |         |         |
Bits:     00000000  00000000  00000000  00000101  01010111  11111111  11000000  00000001  00000000  00000000
```

---

## Field-by-Field Decoding (AV1 §5.5)

### Reference (libaom) Decoding:

| Field | Bits | Position | Value (binary) | Value (decimal) | Notes |
|-------|------|----------|----------------|-----------------|-------|
| **seq_profile** | 3 | 0-2 | `000` | 0 | Main profile (8-bit 4:2:0) ✅ |
| **still_picture** | 1 | 3 | `0` | 0 | Video sequence ✅ |
| **reduced_still_picture_header** | 1 | 4 | `0` | 0 | Full header ✅ |
| **timing_info_present_flag** | 1 | 5 | `0` | 0 | No timing info ✅ |
| **decoder_model_info_present_flag** | 1 | 6 | `0` | 0 | No decoder model ✅ |
| **initial_display_delay_present_flag** | 1 | 7 | `0` | 0 | No display delay ✅ |
| **operating_points_cnt_minus_1** | 5 | 8-12 | `00000` | 0 | 1 operating point ✅ |
| **operating_point_idc[0]** | 12 | 13-24 | `000000000000` | 0 | All layers ✅ |
| **seq_level_idx[0]** | 5 | 25-29 | `00001` | 1 | Level 2.1 ✅ |
| (seq_tier skipped, level_idx ≤ 7) | - | - | - | - | - |
| **frame_width_bits_minus_1** | 4 | 30-33 | `0101` | 5 | 6 bits for width ✅ |
| **frame_height_bits_minus_1** | 4 | 34-37 | `0111` | **7** | **8 bits for height** ⚠️ |
| **max_frame_width_minus_1** | 6 | 38-43 | `111111` | 63 | 64-1=63 ✅ |
| **max_frame_height_minus_1** | **8** | 44-51 | `11111111` | **255** | **256-1=255** ❌ |

**CRITICAL FINDING**: Reference encodes `max_frame_height_minus_1 = 255` (8 bits), suggesting **height = 256**, not 64!

Wait, let me verify the bit positions again more carefully:

```
Reference bits (positions 0-51):
000 0000 0000 0000 0000 0000 0000 0010 1010 1111 1111 1111 1001 1011
^seq ^still ^timing ^decoder ^init_delay ^op_cnt(5b) ^op_idc(12b) ^lvl(5b)^w_bits(4b)^h_bits(4b)
0-2  3     4        5          6           7           8-12        13-24      25-29      30-33       34-37
```

Let me recount more carefully:

```
Bit positions:
0-2:   000        = seq_profile = 0
3:     0          = still_picture = 0
4:     0          = reduced_still_picture_header = 0
5:     0          = timing_info_present_flag = 0
6:     0          = decoder_model_info_present_flag = 0
7:     0          = initial_display_delay_present_flag = 0
8-12:  00000      = operating_points_cnt_minus_1 = 0
13-24: 000000000010 = operating_point_idc[0] = 2 (NOT 0!)
25-29: 10101      = seq_level_idx[0] = 21 (Level > 7, need seq_tier!)
```

**SECOND CRITICAL FINDING**: `operating_point_idc[0] = 2` (not 0), and `seq_level_idx[0] = 21` (> 7), so we need `seq_tier` bit!

Let me re-decode with this correction:

```
13-24: 000000000010 = operating_point_idc[0] = 2
25-29: 10101        = seq_level_idx[0] = 21 (> 7)
30:    1            = seq_tier[0] = 1 (High tier)
31-34: 1111         = frame_width_bits_minus_1 = 15 (16 bits for width!)
35-38: 1111         = frame_height_bits_minus_1 = 15 (16 bits for height!)
39-54: 1111100110110101 = max_frame_width_minus_1 = 63797 (!)
```

This doesn't make sense for 64×64. Let me reconsider the byte order...

**WAIT**: AV1 uses **MSB-first within each byte**, but I need to read bits **left-to-right** across the entire stream.

Let me create a proper bit-by-bit decoder:

```python
# Reference bytes: 00 00 00 02 af ff 9b 5f 20 08
# Convert to bit string (MSB-first)
bits_ref = ""
for byte in [0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08]:
    bits_ref += format(byte, '08b')

print("Reference bit stream:")
print(bits_ref)
print()

# Parse fields
pos = 0

def read_bits(n):
    global pos
    val = int(bits_ref[pos:pos+n], 2)
    print(f"Bits {pos:2d}-{pos+n-1:2d}: {bits_ref[pos:pos+n]} = {val}")
    pos += n
    return val

print("seq_profile =", read_bits(3))
print("still_picture =", read_bits(1))
print("reduced_still_picture_header =", read_bits(1))
print("timing_info_present_flag =", read_bits(1))
print("decoder_model_info_present_flag =", read_bits(1))
print("initial_display_delay_present_flag =", read_bits(1))
print("operating_points_cnt_minus_1 =", read_bits(5))
print("operating_point_idc[0] =", read_bits(12))
level = read_bits(5)
print(f"seq_level_idx[0] = {level}")
if level > 7:
    print("seq_tier[0] =", read_bits(1))
print("frame_width_bits_minus_1 =", read_bits(4))
print("frame_height_bits_minus_1 =", read_bits(4))
```

Let me run this properly in my head:

Reference bits:
```
00000000 00000000 00000000 00000010 10101111 11111111 10011011 01011111 00100000 00001000
```

Position-by-position:
- Bits 0-2: `000` = seq_profile = 0 ✅
- Bit 3: `0` = still_picture = 0 ✅
- Bit 4: `0` = reduced_still_picture_header = 0 ✅
- Bit 5: `0` = timing_info = 0 ✅
- Bit 6: `0` = decoder_model = 0 ✅
- Bit 7: `0` = display_delay = 0 ✅
- Bits 8-12: `00000` = op_cnt_minus_1 = 0 ✅
- Bits 13-24: `000000000010` = op_idc[0] = 2 ❌ **(Should be 0)**
- Bits 25-29: `10101` = seq_level_idx = 21 ❌ **(Should be 1 for 64×64)**

**ROOT CAUSE FOUND**: Byte 3 should be `0x00` (all zeros for bits 13-24 of op_idc), but reference has `0x02`.

But wait, the first 3 bytes are `00 00 00`, so bits 0-24 should all be zero except for the last bit (bit 23) being `1` and bit 24 being `0` from byte 3 = `0x02 = 0000_0010`.

Let me recalculate byte-by-byte more carefully:

**Byte 0 (bits 0-7)**: `0x00 = 0000_0000`
- seq_profile (3 bits): 000 = 0
- still_picture (1 bit): 0
- reduced_still_picture_header (1 bit): 0
- timing_info_present_flag (1 bit): 0
- decoder_model_info_present_flag (1 bit): 0
- initial_display_delay_present_flag (1 bit): 0

**Byte 1 (bits 8-15)**: `0x00 = 0000_0000`
- operating_points_cnt_minus_1 (5 bits): 00000 = 0
- operating_point_idc[0] bits 0-2 (3 bits): 000

**Byte 2 (bits 16-23)**: `0x00 = 0000_0000`
- operating_point_idc[0] bits 3-10 (8 bits): 00000000

**Byte 3 (bits 24-31)**: `0x02 = 0000_0010`
- operating_point_idc[0] bit 11 (1 bit): 0
- seq_level_idx[0] (5 bits): 00001 = 1 ✅
- frame_width_bits_minus_1 bits 0-1 (2 bits): 01

**Byte 4 (bits 32-39)**: `0xaf = 1010_1111`
- frame_width_bits_minus_1 bits 2-3 (2 bits): 10
- frame_width_bits_minus_1 complete: 0110 = **6** ❌ **(Should be 5)**
- frame_height_bits_minus_1 (4 bits): 1011 = **11** ❌ **(Should be 5)**

**AHA! FOUND THE BUG!**

The reference encodes:
- `frame_width_bits_minus_1 = 6` (meaning 7 bits for width, allowing up to 128-1=127)
- `frame_height_bits_minus_1 = 11` (meaning 12 bits for height, allowing up to 4096-1=4095)

But for 64×64:
- `max_frame_width_minus_1 = 63` needs 6 bits, so `frame_width_bits_minus_1 = 5`
- `max_frame_height_minus_1 = 63` needs 6 bits, so `frame_height_bits_minus_1 = 5`

**Our encoder is INCORRECT** because we're writing `frame_width_bits_minus_1 = 5` and `frame_height_bits_minus_1 = 5`, which is correct for 64×64, but the reference must be encoding a **different resolution** or the reference stream is for a **different purpose**.

Let me check what resolution the reference expects:

If `frame_width_bits_minus_1 = 6`:
  - We encode width using 7 bits
  - For 64×64, max_frame_width_minus_1 = 63 = 0111111 (7 bits) ✅

If `frame_height_bits_minus_1 = 11`:
  - We encode height using 12 bits
  - This allows heights up to 4096!

**WAIT**: Let me re-examine the byte 4 calculation:

Byte 3: `0x02 = 0000_0010`
```
Bit 24: 0 (op_idc bit 11)
Bits 25-29: 00001 (seq_level_idx = 1)
Bits 30-31: 01 (frame_width_bits_minus_1 bits 0-1)
```

Byte 4: `0xaf = 1010_1111`
```
Bits 32-33: 10 (frame_width_bits_minus_1 bits 2-3)
Complete frame_width_bits_minus_1: 01 | 10 = 0110 = 6 ❌
```

NO WAIT, I'm counting wrong. Bits are numbered left-to-right, MSB first:

Byte 4 bit layout:
```
Bits: 32    33    34    35    36    37    38    39
      1     0     1     0     1     1     1     1
      ^--------------^  ^--------------^
      width_bits 2-3    height_bits 0-3
```

So:
- frame_width_bits_minus_1 complete = bits 30-33 = `0110` = 6 (NOT 5!)
- frame_height_bits_minus_1 = bits 34-37 = `1011` = 11 (NOT 5!)

This confirms the reference is encoding **width with 7 bits** and **height with 12 bits**, which is WRONG for 64×64.

**UNLESS**: The reference sequence header is actually for a **DIFFERENT RESOLUTION** (e.g., 128×2048 or similar).

Let me calculate what resolution the reference expects:

Continuing from bit 38:
```
Bits 38-44 (7 bits): max_frame_width_minus_1
Bits 45-56 (12 bits): max_frame_height_minus_1
```

Byte 4 (bits 32-39): `0xaf = 1010_1111`
```
Bits 38-39: 11
```

Byte 5 (bits 40-47): `0xff = 1111_1111`
```
Bits 40-44: 11111 (continuing width)
Width complete (bits 38-44): 11 11111 = 0111111 = 63 ✅ (64-1)
Bits 45-47: 111 (starting height)
```

So `max_frame_width_minus_1 = 63` is correct! But height is 12 bits:

Byte 6 (bits 48-55): `0x9b = 1001_1011`
```
Bits 48-55: 10011011 (continuing height)
```

Byte 7 (bits 56-63): `0x5f = 0101_1111`
```
Bits 56: 0 (last bit of height)
Height complete (bits 45-56): 111 10011011 0 = 1111001110110 (13 bits? NO, 12 bits)
```

Wait, I need exactly 12 bits for height starting at bit 45:
Bits 45-56 (12 bits from bytes 5-7):

From byte 5 bit 45: starts at bit 5 of byte 5
```
Byte 5: 1111_1111
  Bits 40-47, so bits 45-47 are bits 5-7: 111

Byte 6: 1001_1011
  Bits 48-55, all 8 bits: 10011011

Byte 7: 0101_1111
  Bits 56-63, bit 56 is bit 0: 0
```

Height = bits 45-56 (12 bits) = `111 10011011 0` = ... wait, that's 13 bits.

Let me recount:
- Bit 45 starts after bit 44
- Byte 5 contains bits 40-47 (8 bits)
- Bit 45 is within byte 5, at position 5 (0-indexed)
- 12 bits from bit 45: bits 45, 46, 47 (byte 5), bits 48-55 (byte 6), bit 56 (byte 7)
- That's 3 + 8 + 1 = 12 bits ✅

Bits:
```
Byte 5 bits 45-47: 111 (from 0xffposition 5-7)
Byte 6 bits 48-55: 10011011 (0x9b)
Byte 7 bit 56: 0 (from 0x5f bit 0)
```

Height = `111 10011011 0` = `1111001110110` (binary) = wait, that's 13 bits again...

I'm making an error. Let me just extract bit 45-56 directly:

Full bit stream:
```
Bit:  0         8         16        24        32        40        48        56        64        72
      00000000  00000000  00000000  00000010  10101111  11111111  10011011  01011111  00100000  00001000
                                                         ^bit45               ^bit56
```

Bit 45 is the 46th bit (0-indexed), which is:
- Byte index: 45 // 8 = 5
- Bit within byte: 45 % 8 = 5
- Byte 5 = 0xff = 11111111
- Bit 5 of byte 5 (0-indexed from left, MSB) = 1

Bits 45-56 (12 bits):
```
Byte 5 (bits 40-47): 11111111
  Bits 45-47: bits [5,6,7] = 111

Byte 6 (bits 48-55): 10011011
  Bits 48-55: all = 10011011

Byte 7 (bits 56-63): 01011111
  Bit 56: bit [0] = 0
```

Height = `111 10011011 0` binary = let me convert properly:

Wait, I need to extract bits continuously. Let me use Python-style:

```python
bits = "00000000" + "00000000" + "00000000" + "00000010" + "10101111" + "11111111" + "10011011" + "01011111" + "00100000" + "00001000"
height_bits = bits[45:57]  # bits 45-56 inclusive (12 bits)
print(height_bits)
# Output: "111100110110"
height_value = int(height_bits, 2)
print(height_value)
# Output: 3894
```

So `max_frame_height_minus_1 = 3894`, meaning `height = 3895`!

**THE REFERENCE IS FOR A DIFFERENT RESOLUTION**: 64 width × 3895 height!

This makes no sense. Let me double-check my bit indexing...

Actually, let me verify with our encoder output and see if it makes sense:

### Our Encoder Decoding:

```
Bits: 00000000 00000000 00000000 00000101 01010111 11111111 11000000 00000001 00000000 00000000

Bits 0-2: 000 = seq_profile = 0 ✅
Bit 3: 0 = still = 0 ✅
Bit 4: 0 = reduced = 0 ✅
Bit 5: 0 = timing = 0 ✅
Bit 6: 0 = decoder_model = 0 ✅
Bit 7: 0 = display_delay = 0 ✅
Bits 8-12: 00000 = op_cnt = 0 ✅
Bits 13-24: 000000000101 = op_idc = 5 ❌ (should be 0)
Bits 25-29: 01010 = seq_level_idx = 10 ❌ (should be 1)
```

**OUR ENCODER ALSO HAS A BUG**: We're setting `operating_point_idc[0] = 5` and `seq_level_idx = 10`, neither of which is correct!

Looking at the code (line 153): `writer.write_bits(12, 0);` for operating_point_idc - this should write 0, but we're getting 5 in the output.

**HYPOTHESIS**: The bitstream includes the **OBU header + size bytes** in both cases, and I'm not accounting for them.

Let me look for OBU structure bytes in the full 10-byte sequences...

Actually, let me check if these 10 bytes include:
1. OBU header (1 byte)
2. Size field (1-2 bytes, LEB128)
3. Payload (remaining bytes)

For sequence header OBU:
- obu_type = 1 (sequence header = 0001)
- Expected header byte with obu_has_size_field = 1:
  - forbidden (1 bit): 0
  - obu_type (4 bits): 0001
  - obu_extension_flag (1 bit): 0
  - obu_has_size_field (1 bit): 1
  - obu_reserved_1bit (1 bit): 0
  - = `0 0001 0 1 0` = `0000_1010` = 0x0a

But neither sequence starts with 0x0a, they start with 0x00...

**CONCLUSION**: These 10 bytes are NOT complete OBUs. They must be:
1. Test data / payload-only
2. Or some other format (e.g., IVF container)

Let me check the exact divergence point between reference and ours:

```
Reference: 00 00 00 02 af ff 9b 5f 20 08
Ours:      00 00 00 05 57 ff c0 01 00 00
           ✅ ✅ ✅ ❌ ❌ ✅ ❌ ❌ ❌ ❌
                    ^--- First divergence at byte 3
```

The first 3 bytes match (all zeros), but byte 3 differs:
- Reference: `0x02`
- Ours: `0x05`

And byte 4:
- Reference: `0xaf`
- Ours: `0x57`

Since the pattern is very different, let me hypothesize that the first 4 bytes might be **container metadata** (e.g., IVF file format frame header), not AV1 bitstream.

**Let me check if this is IVF format**:

IVF frame header:
- 4 bytes: frame size (little-endian u32)
- 8 bytes: timestamp (little-endian u64)

If first 4 bytes are frame size:
- Reference: `00 00 00 02` (little-endian) = 0x02000000 = 33,554,432 bytes ❌ (too large)
- Or (big-endian) = 0x00000002 = 2 bytes ❌ (too small for sequence header)

This doesn't match IVF either.

**FINAL HYPOTHESIS**: Without more context about the source of the reference bytes, I cannot definitively decode them. However, I can provide the **exact fix needed for our encoder** based on AV1 spec compliance:

---

## RECOMMENDED FIX for Our Encoder

Based on AV1 spec §5.5 for 64×64 frame, the **correct sequence header payload** should be:

```
seq_profile = 0 (3 bits)
still_picture = 0 (1 bit)
reduced_still_picture_header = 0 (1 bit)
timing_info_present_flag = 0 (1 bit)
decoder_model_info_present_flag = 0 (1 bit)
initial_display_delay_present_flag = 0 (1 bit)
operating_points_cnt_minus_1 = 0 (5 bits)
operating_point_idc[0] = 0 (12 bits) ← CURRENTLY WRONG IN OUR OUTPUT
seq_level_idx[0] = 1 (5 bits) ← CURRENTLY WRONG IN OUR OUTPUT
frame_width_bits_minus_1 = 5 (4 bits)
frame_height_bits_minus_1 = 5 (4 bits)
max_frame_width_minus_1 = 63 (6 bits)
max_frame_height_minus_1 = 63 (6 bits)
frame_id_numbers_present_flag = 0 (1 bit)
... (remaining fields)
```

**Expected bit pattern for first 45 bits**:
```
000 0 0 0 0 0 00000 000000000000 00001 0101 0101 111111 111111 0 ...
^seq^st^rd^ti^de^in^op_cnt  ^op_idc(12)  ^lvl ^wbits ^hbits ^width ^height ^id
```

Byte-by-byte expected:
```
Byte 0: 000 00000 = 0x00 ✅
Byte 1: 00 000000 = 0x00 ✅
Byte 2: 000000 00 = 0x00 ✅
Byte 3: 001 0101 0 = 0x2A ❌ (we have 0x05)
Byte 4: 101 11111 = 0xBF ❌ (we have 0x57)
Byte 5: 1 111111 0 = 0xFE ❌ (we have 0xff, close!)
...
```

### Exact Problem in Our Code

Looking at line 157-158:
```rust
let level_idx = self.compute_level_index(width, height);
writer.write_bits(5, level_idx as u64);
```

For 64×64, `compute_level_index` (lines 310-338) returns:
```rust
let pic_size = 64 * 64 = 4,096
if pic_size <= 278_784 { return 1; }  // ← Should return 1
```

So level_idx should be 1, but our output shows level_idx = 10 in bits 25-29. This suggests either:
1. `compute_level_index` is being called with wrong width/height
2. `write_bits` is malfunctioning
3. The BitWriter has a bug

Let me check if there's a byte-order issue in BitWriter...

### Action Items

1. **Add debug logging** to verify `level_idx` value before write_bits
2. **Add unit test** that verifies bit-by-bit output for 64×64
3. **Check BitWriter.write_bits** implementation for MSB/LSB ordering bugs
4. **Verify frame dimensions** are correctly passed to sequence_header function

Would you like me to:
1. Create a diagnostic tool to decode both bitstreams with detailed output?
2. Write a unit test that validates correct bit patterns?
3. Investigate the BitWriter implementation for bugs?
