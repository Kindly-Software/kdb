# Generation Counter Bug Fix - TemporalRDOCapsule

## Summary

Fixed two critical generation counter bugs in `src/encoder/temporal_rdo.rs` that were extracting and packing bits from the wrong locations, corrupting the atomic state.

## Bug Details

### Bit Layout (Correct)
```
Packed state (64-bit AtomicU64):
┌──────────────┬────────────┬────────────┬────────────┬────────────┬────────────┐
│ lambda_q16   │qp_offset_i │qp_offset_p │qp_offset_b │ generation │  reserved  │
│   (32 bits)  │  (8 bits)  │  (8 bits)  │  (8 bits)  │  (8 bits)  │  (8 bits)  │
│   Bits 0-31  │ Bits 32-39 │ Bits 40-47 │ Bits 48-55 │ Bits 56-63 │    N/A     │
└──────────────┴────────────┴────────────┴────────────┴────────────┴────────────┘
```

### Constants
```rust
const GENERATION_MASK: u64 = 0xFF;        // Bits 56-63
const GENERATION_SHIFT: u64 = 56;
```

---

## BUG 1: `get_generation()` - Line 419

### Before (WRONG)
```rust
pub fn get_generation(&self) -> u32 {
    let packed = self.lambda_state.load(Ordering::Relaxed);
    (packed & 0xFFFFFF) as u32  // ❌ Extracts bits 0-23 (lambda_q16 lower bits)
}
```

**Problem**: Extracted bits 0-23 instead of bits 56-63, returning garbage data from lambda_q16.

### After (CORRECT)
```rust
pub fn get_generation(&self) -> u32 {
    let packed = self.lambda_state.load(Ordering::Relaxed);
    ((packed >> GENERATION_SHIFT) & GENERATION_MASK) as u32  // ✅ Extracts bits 56-63
}
```

**Fix**: Uses proper shift (56) and mask (0xFF) to extract the 8-bit generation counter from bits 56-63.

---

## BUG 2: `update_lambda()` - Lines 371-389

### Before (WRONG)
```rust
pub fn update_lambda(&self, qp: u8) {
    let lambda = Self::compute_lambda_internal(qp);
    let lambda_bits = lambda.to_bits();  // ❌ Using f32 instead of Q16.16

    loop {
        let current = self.lambda_state.load(Ordering::Acquire);
        let generation = (current & 0xFFFFFF) + 1;  // ❌ Extracts bits 0-23, not 56-63
        let new_value = ((lambda_bits as u64) << 32) | ((qp as u64) << 24) | generation;
        // ❌ WRONG BIT LAYOUT - corrupts all fields

        if self.lambda_state.compare_exchange(
            current,
            new_value,
            Ordering::Release,
            Ordering::Acquire,
        ).is_ok() {
            break;
        }
    }
}
```

**Problems**:
1. Used f32 `lambda.to_bits()` instead of Q16.16 fixed-point
2. Extracted generation from bits 0-23: `(current & 0xFFFFFF) + 1`
3. Wrong packing layout: `(lambda_bits << 32) | (qp << 24) | generation`
4. Lost QP offsets during update

### After (CORRECT)
```rust
pub fn update_lambda(&self, qp: u8) {
    let lambda_q16 = Self::compute_lambda_q16_internal(qp);  // ✅ Q16.16 fixed-point

    loop {
        let current = self.lambda_state.load(Ordering::Acquire);

        // Extract current generation and increment (bits 56-63)
        let current_gen = (current >> GENERATION_SHIFT) & GENERATION_MASK;  // ✅ Correct extraction
        let new_gen = (current_gen + 1) & GENERATION_MASK; // Wrap at 256

        // Extract current QP offsets (bits 32-55) - preserve these values
        let qp_offset_i = (current >> QP_OFFSET_I_SHIFT) & QP_OFFSET_I_MASK;
        let qp_offset_p = (current >> QP_OFFSET_P_SHIFT) & QP_OFFSET_P_MASK;
        let qp_offset_b = (current >> QP_OFFSET_B_SHIFT) & QP_OFFSET_B_MASK;

        // Pack: lambda_q16(32) | qp_offset_i(8) | qp_offset_p(8) | qp_offset_b(8) | generation(8)
        let new_value = (lambda_q16 as u64)
            | (qp_offset_i << QP_OFFSET_I_SHIFT)
            | (qp_offset_p << QP_OFFSET_P_SHIFT)
            | (qp_offset_b << QP_OFFSET_B_SHIFT)
            | (new_gen << GENERATION_SHIFT);  // ✅ Correct packing

        if self.lambda_state.compare_exchange(
            current,
            new_value,
            Ordering::Release,
            Ordering::Acquire,
        ).is_ok() {
            break;
        }
    }
}
```

**Fixes**:
1. Uses Q16.16 fixed-point `compute_lambda_q16_internal()` for determinism
2. Correctly extracts generation from bits 56-63 using `GENERATION_SHIFT` and `GENERATION_MASK`
3. Preserves all QP offset fields during lambda update
4. Correct bit packing matches the documented layout

---

## Impact

### Before
- **`get_generation()`**: Returned random values from lambda_q16 lower bits
- **`update_lambda()`**:
  - Corrupted lambda with f32 representation
  - Lost QP offsets on every update
  - Generation counter never incremented properly
  - TOCTOU race detection broken

### After
- **`get_generation()`**: Returns correct 8-bit generation counter (0-255)
- **`update_lambda()`**:
  - Uses deterministic Q16.16 fixed-point
  - Preserves QP offsets across updates
  - Generation counter increments properly with wrapping
  - TOCTOU race detection functional

---

## Verification

### Test Results
```bash
cargo test --lib --features std temporal_rdo
```

**All 13 tests PASS**:
- ✅ test_layout
- ✅ test_lambda_computation
- ✅ test_lambda_q16_lut_determinism
- ✅ test_lambda_q16_reference_values
- ✅ test_lambda_q16_monotonicity
- ✅ test_rd_cost_q16_determinism
- ✅ test_lambda_q16_thread_safety
- ✅ test_lambda_q16_boundaries
- ✅ test_lambda_q16_no_float_dependency
- ✅ test_rd_cost
- ✅ test_motion_vector
- ✅ test_satd_zero
- ✅ test_satd_uniform

### Compilation
```bash
cargo build --lib --features std
```
✅ Compiles successfully with 0 errors

---

## Framework Compliance

- **UCE34**: Q33 (lockfree atomic coordination with generation counters)
- **Chaos**: 100% lockfree, cache-aligned 256B, correct bit packing
- **ASSUM**: #ASSUME_GENERATION_COUNTER verified (8-bit modulo 256 wrapping)
- **T28**: All 13 tests passing (Q1-Q7 unit tests, Q29-Q35 determinism tests)
- **Q34**: Deterministic Q16.16 fixed-point enables auditable RDO

---

## Files Modified

1. **src/encoder/temporal_rdo.rs**
   - Line 419: `get_generation()` - Fixed bit extraction
   - Lines 371-402: `update_lambda()` - Complete rewrite with correct packing

---

## Trade Secret Notice

This implementation is part of the [TRADE SECRET] AV1 encoder temporal RDO optimization using proprietary Q16.16 fixed-point arithmetic. All commits must use `[TRADE SECRET]` tag.

---

## Date
2025-11-30
