# Motion Estimation Compute Shader Design

**File**: `kernels/motion_estimation.comp`
**Lines**: 238
**Tier**: T7 Heterogeneous (GPU compute)
**Target**: 50-200× speedup vs CPU baseline

## Algorithm Overview

### Diamond Search Pattern
1. **Large Diamond Search** (coarse): ±search_range/2 with adaptive step reduction
2. **Small Diamond Refinement**: ±1 pixel refinement iterations
3. **Quarter-Pixel Output**: Results scaled to quarter-pel precision for AV1 spec

### Performance Strategy

#### Parallel SAD Computation
- **16×16 workgroup**: Each thread processes one pixel of the macroblock
- **Shared memory reduction**: 8-stage parallel sum (256 → 1)
- **Warp-optimized**: Final 32 reductions avoid barriers (GPU warp size)

#### Early Termination
- **Static block detection**: SAD < 256 indicates near-identical blocks
- **Adaptive search step**: Halves step size if no improvement found
- **Iteration limit**: Maximum 4 refinement iterations

## Memory Layout

### Input Buffers
```glsl
set = 0, binding = 0: CurrentFrame (packed uint, 4 pixels per uint)
set = 0, binding = 1: ReferenceFrame (packed uint, 4 pixels per uint)
```

**Packing Format**: `[pixel3|pixel2|pixel1|pixel0]` in single uint (8-bit luma each)

### Output Buffer
```glsl
set = 0, binding = 2: MotionVectors (uvec2 per macroblock)
  - .x = packed (qpel_x: i16, qpel_y: i16)
  - .y = sad: u32
```

### Push Constants
```glsl
width, height: Frame dimensions
search_range: Typically 16 or 32 pixels
mb_cols, mb_rows: Macroblock grid dimensions
```

## Key Design Considerations

### 1. Bounds Checking
- `get_pixel()` returns neutral gray (128) for out-of-bounds accesses
- Prevents reading invalid memory during edge macroblock searches

### 2. Deterministic Results (Chaos Compliance)
- Fixed diamond search patterns (no random sampling)
- Deterministic reduction order (always sums in same sequence)
- Consistent early termination thresholds

### 3. Workgroup Coordination
- **Thread 0 coordinates** search direction (updates best_x, best_y)
- **All threads participate** in SAD computation (parallel reduction)
- **Single writer**: Only thread 0 writes final motion vector

### 4. Memory Bandwidth Optimization
- **Packed pixels**: 4× reduction in memory transfers
- **Shared memory**: 256 bytes for intermediate SAD results
- **Coalesced reads**: 16×16 threads read contiguous pixel regions

## Performance Targets

| Resolution | CPU Baseline | GPU Target | Expected Speedup |
|------------|--------------|------------|------------------|
| 1080p      | 1.37ms       | <2ms       | ~1× (baseline)   |
| 4K         | ~5.5ms       | <8ms       | ~1× (baseline)   |
| 8K         | Unknown      | <30ms      | TBD              |

**Note**: Initial implementation focuses on correctness. Optimization iterations will target 50-200× via:
- Sub-pixel refinement (half-pel, quarter-pel)
- Multi-reference frame support
- Texture cache optimization
- Warp shuffle reductions

## Compilation

**Manual (if glslc available)**:
```bash
glslc kernels/motion_estimation.comp -o kernels/motion_estimation.spv
```

**Runtime (via Vulkan backend)**:
```rust
// Shader compilation handled by gpu::vulkan::compile_shader()
// SPIR-V generated on-the-fly or cached
```

## Next Steps (P-GPU.3)

1. **Vulkan Backend Integration**: Load shader, create pipeline, dispatch workgroups
2. **Host Buffers**: Upload current/reference frames, download motion vectors
3. **Pipeline Barriers**: Ensure compute completion before encoder reads results
4. **Benchmarking**: B32 validation vs CPU baseline (1.37ms @ 1080p)

## Trade Secret Notice

[TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL

Diamond search implementation with optimized reduction patterns is confidential.
Do not share publicly without authorization.
