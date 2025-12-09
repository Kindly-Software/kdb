# Encoder Module V1/V2 Status Matrix

Quick reference for encoder module deprecation status and migration paths.

## Deprecated V1 Modules (Use V2 Instead)

| V1 Module | Status | V2 Replacement | Speedup | Since |
|-----------|--------|----------------|---------|-------|
| `dct_transform` | 🔴 Deprecated | `dct_transform_simd` | SOTA Chen-Wang | 0.9.0 |
| `obu_bitstream` | 🔴 Deprecated | `obu_bitstream_v2` | 4× SIMD | 0.9.0 |
| `reference_frame` | 🔴 Deprecated | `reference_frame_v2` | Improved mgmt | 0.9.0 |
| `intra_prediction` | 🔴 Deprecated | `intra_prediction_v2` | 10-20× pruning | 0.9.0 |
| `cdef_filter` | 🔴 Deprecated | `cdef_filter_v2` | 8-dir SIMD | 0.9.0 |
| `lrf` | 🔴 Deprecated | `loop_restoration_v2` | O(1) integral | 0.9.0 |
| `film_grain` | 🔴 Deprecated | `film_grain_v2` | 10× Netflix/JPEG-XL | 0.9.0 |
| `superresolution` | 🔴 Deprecated | `superresolution_v2` | 4× speedup | 0.9.0 |
| `motion_estimation` | 🔴 Deprecated | `motion_estimation_v2` | AVX2 diamond | 0.9.0 |
| `inter_prediction` | 🔴 Deprecated | `inter_prediction_v2` | SIMD 8-tap | 0.9.0 |

## Active V1 Modules (No V2 Replacement)

| Module | Status | Reason | Tests |
|--------|--------|--------|-------|
| `temporal_rdo` | ✅ Active | No V2, all tests pass | 23/23 |
| `gop_coordinator` | ✅ Active | V2 coexists (different features) | Varies |
| `entropy_coder` | ✅ Active | `entropy_coder_simd` different use case | Varies |

## V2 SOTA 2025 Modules (Preferred)

| V2 Module | Tier | Size | Key Techniques | Status |
|-----------|------|------|----------------|--------|
| `dct_transform_simd` | T2 SIMD | 256B | Chen-Wang butterfly ops | ✅ Production |
| `obu_bitstream_v2` | T5 Streaming | 128B | SIMD bit packing | ✅ Production |
| `reference_frame_v2` | T1 Atomic | 256B | Improved ref mgmt | ✅ Production |
| `intra_prediction_v2` | T2 SIMD | 128B | Fast mode pruning | ✅ Production |
| `cdef_filter_v2` | T2 SIMD | 256B | 8-dir + noise adaptive | ✅ Production |
| `loop_restoration_v2` | T2 SIMD | 512B | Integral image O(1) | ✅ Production |
| `film_grain_v2` | T2 SIMD | 256B | Netflix AR(1) + JPEG-XL | ✅ Production |
| `superresolution_v2` | T2 SIMD | 256B | AOM 2024 spec | ✅ Production |
| `motion_estimation_v2` | T2+T4 | 512B | AVX2 + diamond search | ✅ Production |
| `inter_prediction_v2` | T6 Mixed | 512B | SIMD 8-tap + warped | ✅ Production |

## Migration Example

```rust
// ❌ Old (Deprecated V1)
use atomic_capsule::encoder::{
    DctTransformCapsule,
    IntraPredictionCapsule,
    IntraMode,
    CdefFilterCapsule,
};

// ✅ New (V2 SOTA 2025)
use atomic_capsule::encoder::{
    DctTransformCapsuleSIMD,
    IntraPredictionCapsuleV2,
    IntraModeV2,
    CdefFilterCapsuleV2,
};
```

## Quick Decision Tree

```
Need encoder module?
├─ Check if V2 exists (see matrix above)
│  ├─ Yes → Use V2 (SOTA 2025, 2-20× speedup)
│  └─ No → Use V1 (still maintained)
├─ V1 deprecated warning?
│  └─ Migrate to V2 (see deprecation message)
└─ Both V1/V2 available?
   └─ Use V2 unless specific V1 feature needed
```

## Performance Comparison (Estimated)

| Module | V1 Baseline | V2 Speedup | Technique |
|--------|-------------|------------|-----------|
| DCT Transform | 1× | SOTA | Chen-Wang butterfly |
| OBU Bitstream | 1× | 4× | SIMD bit packing |
| Intra Prediction | 1× | 10-20× | Fast mode pruning |
| CDEF Filter | 1× | 8× | 8-direction SIMD |
| Loop Restoration | 1× | O(1) | Integral image |
| Film Grain | 1× | 10× | Netflix/JPEG-XL |
| Superresolution | 1× | 4× | AOM 2024 optimizations |
| Motion Estimation | 1× | AVX2 | Diamond search |
| Inter Prediction | 1× | SIMD | 8-tap interpolation |

## Backward Compatibility

All V1 modules remain available with deprecation warnings. Zero breaking changes.

```rust
// Still compiles (with warnings)
let capsule = DctTransformCapsule::new();

// Warning: use of deprecated struct `encoder::dct_transform::DctTransformCapsule`:
//          Use dct_transform_simd instead - SOTA 2025 Chen-Wang DCT with portable_simd
```

## See Also

- **Full Details**: `V1_ENCODER_DEPRECATION_SUMMARY.md`
- **Migration Guide**: `ENCODER_MIGRATION_GUIDE.md` (TODO)
- **V2 Performance**: Run `cargo bench --bench encoder_*_bench` (B32 validation)
- **V2 Documentation**: See module-level docs in `src/encoder/*_v2.rs`
