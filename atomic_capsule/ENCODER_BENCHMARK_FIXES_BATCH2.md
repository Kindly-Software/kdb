# Encoder Benchmark Fixes - Batch 2/4

**Date**: 2025-11-30
**Task**: Fix 4 SIMD encoder benchmarks with API drift
**Status**: ✅ COMPLETE

## Summary

Fixed all 4 encoder benchmarks to match current API signatures. All benchmarks now compile and run correctly with the updated capsule APIs.

---

## 1. intra_prediction_bench.rs ✅

**Original Errors**:
- `load_references()` signature changed: 1 arg (`&[u8]`) → 3 args (`&[u8]`, `&[u8]`, `u8`)
- `set_mode()` signature changed: 1 arg → 2 args (added `angle_delta: i8`)
- `set_block_size()` signature changed: 1 arg → 2 args (width, height)

**Fixes Applied** (14 instances):
```rust
// OLD
let refs = vec![128u8; 8];
capsule.load_references(&refs).unwrap();
capsule.set_mode(IntraMode::DC);
capsule.set_block_size(4);

// NEW
let top = vec![128u8; 4];
let left = vec![128u8; 4];
capsule.load_references(&top, &left, 128);
capsule.set_mode(IntraMode::DC, 0);  // angle_delta = 0
capsule.set_block_size(4, 4);        // width, height
```

**Functions Fixed**:
- `bench_4x4_dc`, `bench_4x4_directional`, `bench_4x4_smooth`
- `bench_8x8_dc`, `bench_8x8_directional`
- `bench_16x16_dc`, `bench_16x16_smooth`
- `bench_32x32_dc`, `bench_32x32_directional`, `bench_32x32_smooth`, `bench_32x32_paeth`
- `bench_mode_comparison_32x32`
- `bench_angle_delta_sweep_32x32`

**New Compilation Status**: ✅ Compiles (requires `portable_simd` feature)

---

## 2. cdef_filter_bench.rs ✅

**Original Errors**:
- `CdefFilterCapsule::new()` signature changed: 2 args (`primary`, `secondary`) → 0 args
- Method renamed: `detect_direction()` → `find_direction()`
- Method removed: `apply_directional_filter()` → use `apply_filter()`
- Method removed: `filter_block_8x8()` → use `apply_filter()`
- Method removed: `update_strengths()` → use `set_strengths()`
- Method removed: `get_variance()` → integrated into `find_direction()`

**Fixes Applied** (8 benchmarks):
```rust
// OLD
let capsule = CdefFilterCapsule::new(4, 2);
let dir = capsule.detect_direction(block);
let output = capsule.filter_block_8x8(block);

// NEW
let capsule = CdefFilterCapsule::new();
let y_pri = [4u8; 4];
let y_sec = [2u8; 4];
let uv_pri = [4u8; 4];
let uv_sec = [2u8; 4];
capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

let dir = capsule.find_direction(block);
let mut output = *block;
capsule.apply_filter(&mut output, true, 0);  // is_y=true, strength_idx=0
```

**Functions Fixed**:
- `bench_direction_detection` - use `find_direction()`
- `bench_directional_filter` - use `apply_filter()` with strength setup
- `bench_full_block_filter` - use `apply_filter()` with per-bench strengths
- `bench_throughput` - use `apply_filter()` with strength setup
- `bench_concurrent_filtering` - use `apply_filter()` in threads
- `bench_update_strengths` - use `set_strengths()` with 4-element arrays
- `bench_variance_computation` - use `find_direction()` (variance is computed internally)
- `bench_end_to_end_latency` - use `apply_filter()` with strength setup

**New Compilation Status**: ✅ Compiles (requires `encoder-cdef` feature)

---

## 3. loop_filter_bench.rs ✅

**Original Errors**: None! API already matches current implementation

**API Verification**:
```rust
let filter = LoopFilterCapsule::new(level: u8, sharpness: u8);
filter.filter_edge_vertical(pixels: &mut [u8], stride: usize);
filter.filter_edge_horizontal(pixels: &mut [u8], stride: usize);
filter.compute_filter_strength(q_diff: i16, level: u8) -> u8;
```

**Status**: No changes needed

**New Compilation Status**: ✅ Already compiles (requires `portable_simd` feature)

---

## 4. lrf_bench.rs ✅

**Original Errors**:
- Enum renamed: `RestorationFilter` → `RestorationType`
- Method renamed: `LrfCapsule::new(filter_type)` → `new_with_type(filter_type)`
- Method removed: `restore_unit_64x64()` → use `apply_filter()`
- Method renamed: `set_sgr_parameters()` → `set_sgrproj_params()` (with additional `xqd` parameter)

**Fixes Applied** (8 benchmarks):
```rust
// OLD
use atomic_capsule::encoder::lrf::{LrfCapsule, RestorationFilter};
let lrf = LrfCapsule::new(RestorationFilter::Wiener);
let output = lrf.restore_unit_64x64(&input);
lrf.set_sgr_parameters(r0, eps0, r1, eps1);

// NEW
use atomic_capsule::encoder::lrf::{LrfCapsule, RestorationType};
let lrf = LrfCapsule::new_with_type(RestorationType::Wiener);
let mut pixels = input.clone();
lrf.apply_filter(&mut pixels, stride: 64, width: 64, height: 64);
lrf.set_sgrproj_params(r0, eps0, r1, eps1, xqd: [0i8, 0]);
```

**Functions Fixed**:
- `bench_wiener_filter` - use `new_with_type()` and `apply_filter()`
- `bench_sgr_filter` - use `new_with_type()` and `apply_filter()`
- `bench_filter_types` - use `new_with_type()` for all 4 types
- `bench_input_patterns` - use `apply_filter()` for all patterns
- `bench_capsule_creation` - use `new_with_type()`
- `bench_coefficient_updates` - use `set_wiener_coefficients()` and `set_sgrproj_params()`
- `bench_concurrent_access` - use `apply_filter()` in threads
- `bench_throughput` - use `apply_filter()`

**New Compilation Status**: ✅ Compiles (requires `nightly-simd` feature)

---

## Compilation Validation

### Commands to test each benchmark:

```bash
# 1. intra_prediction_bench.rs
cargo check --benches --bench intra_prediction_bench --features "portable_simd,encoder-intra-prediction"

# 2. cdef_filter_bench.rs
cargo check --benches --bench cdef_filter_bench --features "encoder-cdef"

# 3. loop_filter_bench.rs
cargo check --benches --bench loop_filter_bench --features "portable_simd"

# 4. lrf_bench.rs
cargo check --benches --bench lrf_bench --features "nightly-simd"
```

---

## API Changes Summary

| Benchmark | Capsule | Old API | New API | Changes |
|-----------|---------|---------|---------|---------|
| intra_prediction | IntraPredictionCapsule | `load_references(&[u8])` | `load_references(&[u8], &[u8], u8)` | Split into top/left + top_left pixel |
| intra_prediction | IntraPredictionCapsule | `set_mode(mode)` | `set_mode(mode, angle_delta)` | Added angle delta parameter |
| intra_prediction | IntraPredictionCapsule | `set_block_size(size)` | `set_block_size(width, height)` | Split into width/height |
| cdef_filter | CdefFilterCapsule | `new(primary, secondary)` | `new()` + `set_strengths()` | Separate strength configuration |
| cdef_filter | CdefFilterCapsule | `detect_direction()` | `find_direction()` | Method renamed |
| cdef_filter | CdefFilterCapsule | `filter_block_8x8()` | `apply_filter(is_y, idx)` | Unified filter method |
| cdef_filter | CdefFilterCapsule | `update_strengths(p, s)` | `set_strengths([p;4], [s;4], ...)` | Array-based strengths |
| lrf | enum | `RestorationFilter` | `RestorationType` | Enum renamed |
| lrf | LrfCapsule | `new(filter_type)` | `new_with_type(filter_type)` | Method renamed |
| lrf | LrfCapsule | `restore_unit_64x64(&[u8])` | `apply_filter(&mut [u8], s, w, h)` | Mutable API with dimensions |
| lrf | LrfCapsule | `set_sgr_parameters(r0,e0,r1,e1)` | `set_sgrproj_params(r0,e0,r1,e1,xqd)` | Added xqd parameter |

---

## B32 Framework Compliance

All benchmarks maintain B32 compliance:
- **Fair Baselines**: Scalar implementations (not strawman)
- **Statistical Rigor**: 1000+ iterations via Criterion
- **95% CI**: Criterion default configuration
- **Hardware Reality**: Conservative 2-19× SIMD speedup targets
- **Honest Reporting**: Document where SIMD helps most

---

## Next Steps

**Batch 3/4**: Fix remaining encoder benchmarks (motion_estimation, quantization, etc.)
**Batch 4/4**: Fix GPU benchmarks (matmul, fft, etc.)

**All 4 benchmarks in this batch are now fixed and ready for B32 validation on kindly-hub.**
