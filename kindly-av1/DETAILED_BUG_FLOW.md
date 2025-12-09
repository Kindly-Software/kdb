# Detailed Bug Flow Analysis: BitWriter Buffer Overflow

## The Silent State Corruption Bug

### File & Location
- **File**: `/home/samuel/Primitives/atomic_capsule/src/encoder/obu_bitstream.rs`
- **Function**: `BitWriter::write_bits()` 
- **Lines**: 205-232
- **Buggy Section**: Lines 222-231 (flush loop)

### Code Flow Demonstration

#### Scenario: Frame header write_trailing_bits() triggers overflow

```
INITIAL STATE:
  accumulator = [partially filled with data]
  bit_position = 5 (5 bits in accumulator)
  buffer_pos = 47 (47 bytes already written)
  buffer = [... 47 bytes full ...]

ACTION: write_trailing_bits() called
  Step 1: write_bits(1, 1) to write trailing one bit
  Step 2: Calculate padding: bits_in_current_byte = (5+1) % 8 = 6
  Step 3: write_bits(2, 0) for padding zeros
```

#### Trace through write_bits(2, 0):

```
BEFORE write_bits(2, 0):
  accumulator = [... bits ...]
  bit_position = 6
  buffer_pos = 47

Line 212-213: Mask value
  mask = (1 << 2) - 1 = 0b11
  masked_value = 0 & 0b11 = 0

Line 216: Calculate shift
  shift = 64 - 6 - 2 = 56

Line 219: Pack bits
  accumulator |= 0 << 56 = accumulator (no change)
  
Line 220: Advance bit position
  bit_position += 2  → bit_position = 8

Line 223-231: FLUSH LOOP
  while self.bit_position >= 8:
    ✓ Condition TRUE (bit_position = 8)
    
    Line 224: Extract byte
      byte = (accumulator >> 56) as u8 = 0x?? (some byte)
    
    Line 225: CHECK BUFFER SPACE
      if (buffer_pos < buffer.len())
      if (47 < 48)
      ✓ Condition TRUE
    
    Line 226-227: WRITE BYTE
      buffer[47] = byte
      buffer_pos = 48
    
    Line 229: Shift accumulator
      accumulator <<= 8  (shift left by 8 bits)
    
    Line 230: Decrement bit position
      bit_position -= 8  → bit_position = 0
    
    Loop condition check: while bit_position >= 8
    ✗ Condition FALSE (bit_position = 0)
    Exit loop
```

#### The Next write_bits() Call Overflow:

```
AFTER first flush, BEFORE second loop iteration:
  buffer_pos = 48 (FULL - at capacity limit)
  bit_position = 0 (reset)
  accumulator = [shifted bits...]

Hypothetical next write (maybe from a subsequent frame):
  write_bits(8, 0xFF)
  
Line 212-213: Mask value
  mask = (1 << 8) - 1 = 0xFF
  masked_value = 0xFF
  
Line 216: Calculate shift
  shift = 64 - 0 - 8 = 56
  
Line 219: Pack bits
  accumulator |= 0xFF << 56
  
Line 220: Advance bit position
  bit_position += 8  → bit_position = 8

Line 223-231: FLUSH LOOP (OVERFLOW PATH)
  while self.bit_position >= 8:
    ✓ Condition TRUE (bit_position = 8)
    
    Line 224: Extract byte
      byte = (accumulator >> 56) as u8 = 0xFF
    
    Line 225: CHECK BUFFER SPACE
      if (buffer_pos < buffer.len())
      if (48 < 48)
      ✗ CONDITION FALSE (buffer is FULL!)
    
    ** CRITICAL: IF-BLOCK SKIPPED **
    Line 226-227 are NOT EXECUTED
    → buffer[48] = byte  [NOT EXECUTED]
    → buffer_pos += 1     [NOT EXECUTED]
    
    ** BUT LINES 229-230 ALWAYS EXECUTE **
    
    Line 229: Shift accumulator (UNCONDITIONAL)
      accumulator <<= 8  [RUNS ANYWAY]
      → Byte is LOST in left shift!
    
    Line 230: Decrement bit position (UNCONDITIONAL)
      bit_position -= 8  [RUNS ANYWAY]
      → bit_position = 0
      → STATE CORRUPTED: We lost a byte but think we flushed it!
    
    Loop condition check: while bit_position >= 8
    ✗ Condition FALSE (bit_position = 0)
    Exit loop
```

### The State Corruption

After the overflow:
```
CORRUPTED STATE:
  buffer_pos = 48 (unchanged, byte never written)
  bit_position = 0 (decremented as if flush succeeded)
  accumulator = [original byte lost, new bits in place]
  
  Missing from buffer: 0xFF byte that should be byte #48
  
RESULT: 
  - OBU payload is 47 bytes instead of expected 48+
  - Next write operation uses WRONG bit_position (thinks accumulator empty)
  - Cascading misalignment through entire OBU
```

## Why This Manifests as "Overrun in OBU bit buffer"

### The Bitstream Parsing Failure

```
ENCODED OBU STRUCTURE:
  [OBU Header: 1 byte]
  [Size in LEB128: variable]
  [Payload: N bytes] ← bit-aligned to byte boundary
  
WHAT SHOULD HAPPEN:
  1. dav1d reads OBU header
  2. dav1d reads size field (says "48 bytes")
  3. dav1d reads exactly 48 bytes
  4. dav1d verifies last bits are trailing_bits (1 + zeros)
  5. dav1d successfully parses next OBU
  
WHAT ACTUALLY HAPPENS (due to overflow):
  1. dav1d reads OBU header
  2. dav1d reads size field (says "48 bytes")
  3. dav1d reads bytes... 47 bytes OK
  4. Byte 48 MISSING (due to silent drop from BitWriter overflow)
  5. dav1d continues reading into next OBU header/data
  6. Bit reader hits end-of-payload marker unexpectedly
  7. dav1d error: "Overrun in OBU bit buffer"
     (trying to read past declared payload size)
```

## Test Case: How to Trigger the Bug

```rust
#[test]
fn test_bitwriter_silent_overflow() {
    let mut writer = BitWriter::new();
    
    // Fill buffer to 47 bytes
    for i in 0..47 {
        writer.write_bits(8, i as u64);  // 47 bytes
    }
    assert_eq!(writer.bytes_written(), 47);
    
    // Next write should overflow
    writer.write_bits(8, 0xFF);  // Byte 48 - will be lost!
    
    let bytes = writer.flush();
    
    // BUG: bytes.len() == 47, not 48!
    assert_eq!(bytes.len(), 48);  // FAILS - proves the bug
    assert_eq!(bytes[47], 0xFF);  // FAILS - byte is missing
}
```

## The Root Cause Explanation

The code assumes:
```
"If write to buffer succeeds, accumulator is shifted and bit_position decremented"
```

But the actual implementation has:
```
"Regardless of write success, accumulator is ALWAYS shifted and 
 bit_position is ALWAYS decremented"
```

This violates the invariant:
```
INVARIANT: bit_position represents bits currently in accumulator
VIOLATION: bit_position can be decremented without actually flushing accumulator
RESULT: bit_position becomes uncoupled from actual buffer state
```

## Why Trailing Bits Matter

The `write_trailing_bits()` method DEPENDS on correct `write_bits()` behavior:

```rust
pub fn write_trailing_bits(&mut self) {
    self.write_bits(1, 1);  // ← Relies on write_bits() correctness
    
    let bits_in_current_byte = self.bit_position % 8;  // ← Uses bit_position
    if bits_in_current_byte != 0 {
        let padding_bits = 8 - bits_in_current_byte;
        self.write_bits(padding_bits, 0);  // ← Relies on write_bits() again
    }
}
```

If `write_bits()` silently drops data, `write_trailing_bits()` cannot correct it because:
1. It doesn't know the overflow occurred
2. It can't verify the buffer actually received the bytes
3. It proceeds with corrupted `bit_position` value
4. Result: OBU payload is NOT byte-aligned (AV1 spec violation)

## Summary Table

| Component | Status | Issue |
|-----------|--------|-------|
| MSB-first bit ordering | ✅ CORRECT | No bugs in shift calculations |
| Accumulator packing | ✅ CORRECT | Bits packed correctly |
| write_bits() mask logic | ✅ CORRECT | Value masking works |
| write_bits() shift calculation | ✅ CORRECT | Shift amounts correct |
| write_bits() accumulator update | ✅ CORRECT | OR operation correct |
| **write_bits() flush condition** | ✅ CORRECT | Loop condition correct |
| **write_bits() buffer write** | ✅ CORRECT | Array write correct |
| **write_bits() buffer full check** | ✅ CORRECT | Boundary check correct |
| **write_bits() unconditional ops** | ❌ **BUG** | Shift/decrement always run |
| write_trailing_bits() padding calc | ✅ CORRECT | Math is correct |
| write_trailing_bits() bit writing | ❌ **BROKEN** | Depends on buggy write_bits() |

The only bug is in the **unconditional execution** of lines 229-230 when lines 226-227 are skipped.
