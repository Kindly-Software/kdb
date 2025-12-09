use atomic_capsule::los::{GridLosAdapter, Q16_16};
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::mem::size_of;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::alloc::{alloc_zeroed, dealloc, Layout};
#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
use std::arch::x86_64::*;
#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::slice;
use std::sync::Arc;

/// 32-byte aligned, fixed-length i32 buffer (SoA for LOS)
#[derive(Debug)]
struct AlignedI32Buffer {
    ptr: NonNull<i32>,
    len: usize,
    layout: Layout,
}

impl AlignedI32Buffer {
    fn new(len: usize, fill: i32) -> Self {
        if len == 0 {
            return Self {
                ptr: NonNull::dangling(),
                len: 0,
                layout: Layout::from_size_align(0, 32).expect("layout"),
            };
        }
        let layout =
            Layout::from_size_align(len.saturating_mul(size_of::<i32>()), 32).expect("layout");
        // Safety: layout is valid; allocation is zeroed then filled.
        let raw = unsafe { alloc_zeroed(layout) as *mut i32 };
        let ptr = NonNull::new(raw).expect("alloc_zeroed returned null");
        // Initialize with the provided fill value.
        for idx in 0..len {
            unsafe { ptr.as_ptr().add(idx).write(fill) };
        }
        Self { ptr, len, layout }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn as_ptr(&self) -> *const i32 {
        self.ptr.as_ptr()
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut i32 {
        self.ptr.as_ptr()
    }

    #[inline]
    fn get(&self, idx: usize) -> Option<&i32> {
        if idx < self.len {
            // Safety: bounds checked.
            Some(unsafe { &*self.ptr.as_ptr().add(idx) })
        } else {
            None
        }
    }

    #[inline]
    fn get_mut(&mut self, idx: usize) -> Option<&mut i32> {
        if idx < self.len {
            // Safety: bounds checked.
            Some(unsafe { &mut *self.ptr.as_ptr().add(idx) })
        } else {
            None
        }
    }
}

impl Deref for AlignedI32Buffer {
    type Target = [i32];

    fn deref(&self) -> &Self::Target {
        // Safety: buffer is valid for len elements.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl DerefMut for AlignedI32Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: buffer is valid for len elements.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for AlignedI32Buffer {
    fn drop(&mut self) {
        if self.len > 0 {
            // Safety: pointer/layout come from alloc_zeroed with the same layout.
            unsafe { dealloc(self.ptr.as_ptr() as *mut u8, self.layout) };
        }
    }
}

/// Reusable scratch SoA for LOS staging to avoid per-call allocations/zeroing.
#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
struct LosScratchSlab {
    cover: Vec<i32>,
    mud: Vec<i32>,
    cache: LosStageCache,
}

#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
impl LosScratchSlab {
    fn new() -> Self {
        Self {
            cover: Vec::new(),
            mud: Vec::new(),
            cache: LosStageCache::default(),
        }
    }

    #[inline]
    fn ensure_len(&mut self, len: usize) {
        if self.cover.len() < len {
            self.cover.resize(len, 0);
        }
        if self.mud.len() < len {
            self.mud.resize(len, 0);
        }
    }
}

#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
thread_local! {
    static LOS_SCRATCH: RefCell<LosScratchSlab> = RefCell::new(LosScratchSlab::new());
}

#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
#[inline(always)]
fn fingerprint_indices(indices: &[usize]) -> u64 {
    if indices.is_empty() {
        return 0;
    }
    let len = indices.len() as u64;
    let first = indices.first().copied().unwrap_or(0) as u64;
    let last = indices.last().copied().unwrap_or(0) as u64;
    let mid = indices.get(indices.len() / 2).copied().unwrap_or(0) as u64;
    let tap = indices.get(indices.len() / 3).copied().unwrap_or(0) as u64;
    let mut h = len.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= first.wrapping_mul(0x85EB_CA6B);
    h = h.rotate_left(13) ^ last.wrapping_mul(0xC2B2_AE35);
    h ^= mid.wrapping_mul(0x27D4_EB2D);
    h ^ tap
}

#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn sum_m256i(acc: __m256i) -> u64 {
    let mut buf = [0i32; 8];
    // Safety: buf is valid for 8 i32 elements; storeu permits unaligned stores.
    unsafe { _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, acc) };
    buf.iter().map(|&v| v as u64).sum()
}

#[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
#[derive(Default)]
struct LosStageCache {
    epoch: u64,
    fingerprint: u64,
    len: usize,
    valid: bool,
}

/// Terrain tile capsule (single tile of height/material with friction/mud).
///
/// - Alignment: 64B to stay on one cache line.
/// - Size: 64B padded.
#[repr(C, align(64))]
#[derive(Debug)]
pub struct TerrainTileCapsule {
    height_mm: AtomicU64,
    slope_q16: AtomicU64,
    cover_q16: AtomicU64,
    mud_q16: AtomicU64,
    material: AtomicU64,
    _padding: [u8; 24],
}

impl TerrainTileCapsule {
    pub fn new(
        height_mm: u32,
        slope_q16: u32,
        cover_q16: u32,
        mud_q16: u32,
        material: u32,
    ) -> Self {
        Self {
            height_mm: AtomicU64::new(height_mm as u64),
            slope_q16: AtomicU64::new(slope_q16 as u64),
            cover_q16: AtomicU64::new(cover_q16 as u64),
            mud_q16: AtomicU64::new(mud_q16 as u64),
            material: AtomicU64::new(material as u64),
            _padding: [0; 24],
        }
    }

    pub fn snapshot(&self) -> TerrainSnapshot {
        TerrainSnapshot {
            height_mm: self.height_mm.load(Ordering::Relaxed) as u32,
            slope_q16: self.slope_q16.load(Ordering::Relaxed) as u32,
            cover_q16: self.cover_q16.load(Ordering::Relaxed) as u32,
            mud_q16: self.mud_q16.load(Ordering::Relaxed) as u32,
            material: self.material.load(Ordering::Relaxed) as u32,
        }
    }

    pub fn set_height(&self, height_mm: u32) {
        self.height_mm.store(height_mm as u64, Ordering::Release);
        // Terrain edit: mark epoch.
        // Safety: epoch bumps are idempotent and lock-free.
        // Caller ensures grid-level epoch will be bumped.
    }

    pub fn set_mud(&self, mud_q16: u32) {
        self.mud_q16.store(mud_q16 as u64, Ordering::Release);
        // Terrain edit: mark epoch via parent grid bump.
    }

    pub fn set_slope(&self, slope_q16: u32) {
        self.slope_q16.store(slope_q16 as u64, Ordering::Release);
        // Terrain edit: mark epoch via parent grid bump.
    }

    pub fn set_cover(&self, cover_q16: u32) {
        self.cover_q16.store(cover_q16 as u64, Ordering::Release);
        // Terrain edit: mark epoch via parent grid bump.
    }

    pub fn set_material(&self, material: u32) {
        self.material.store(material as u64, Ordering::Release);
    }
}

verify_capsule_properties!(TerrainTileCapsule, 64, 64);

#[derive(Debug, Clone, Copy)]
pub struct TerrainSnapshot {
    pub height_mm: u32,
    pub slope_q16: u32,
    pub cover_q16: u32,
    pub mud_q16: u32,
    pub material: u32,
}

/// Terrain grid capsule (shared immutable layout, mutable tile contents).
#[repr(C, align(128))]
#[derive(Debug)]
pub struct TerrainGridCapsule {
    width: u32,
    height: u32,
    tiles: Arc<Vec<TerrainTileCapsule>>,
    cost_strip: Vec<AtomicU32>,
    // Scalar path (default): uses per-tile snapshot.
    // SIMD (experimental, feature-gated): uses read-only slices captured via snapshot().
    cover_strip: Vec<AtomicU32>,
    mud_strip: Vec<AtomicU32>,
    cover_plain: AlignedI32Buffer,
    mud_plain: AlignedI32Buffer,
    cost_plain: AlignedI32Buffer,
    lod_masks: Option<LodMasks>,
    ray_cache: Vec<RayCacheEntry>,
    los_adapter: GridLosAdapter,
    terrain_epoch: AtomicU64,
    _padding: [u8; 32],
}

impl TerrainGridCapsule {
    pub fn new(width: u32, height: u32, default_tile: TerrainSnapshot) -> Self {
        let count = (width as usize).saturating_mul(height as usize);
        let los_adapter = GridLosAdapter::new(width, height, 1.0);
        let mut tiles = Vec::with_capacity(count);
        let mut cover_strip = Vec::with_capacity(count);
        let mut mud_strip = Vec::with_capacity(count);
        let mut cost_strip = Vec::with_capacity(count);
        let default_cost = compute_cost(
            default_tile.slope_q16,
            default_tile.mud_q16,
            default_tile.cover_q16,
        );
        let cover_plain = AlignedI32Buffer::new(count, default_tile.cover_q16 as i32);
        let mud_plain = AlignedI32Buffer::new(count, default_tile.mud_q16 as i32);
        let cost_plain = AlignedI32Buffer::new(count, default_cost as i32);
        for _ in 0..count {
            tiles.push(TerrainTileCapsule::new(
                default_tile.height_mm,
                default_tile.slope_q16,
                default_tile.cover_q16,
                default_tile.mud_q16,
                default_tile.material,
            ));
            cover_strip.push(AtomicU32::new(default_tile.cover_q16));
            mud_strip.push(AtomicU32::new(default_tile.mud_q16));
            cost_strip.push(AtomicU32::new(default_cost));
        }
        let mut grid = Self {
            width,
            height,
            tiles: Arc::new(tiles),
            cost_strip,
            cover_strip,
            mud_strip,
            cover_plain,
            mud_plain,
            cost_plain,
            lod_masks: None,
            ray_cache: Vec::new(),
            los_adapter,
            terrain_epoch: AtomicU64::new(0),
            _padding: [0; 32],
        };
        // Attach buffers to LOS adapter (SoA pointers). Unsafe: pointers remain valid for grid lifetime.
        let cover_ptr = grid.cover_plain.as_mut_ptr();
        let mud_ptr = grid.mud_plain.as_mut_ptr();
        let cost_ptr = grid.cost_plain.as_mut_ptr();
        unsafe {
            grid.los_adapter
                .attach_buffers(cover_ptr, mud_ptr, cost_ptr);
        }
        grid
    }

    #[inline(always)]
    fn bump_terrain_epoch(&self) {
        self.terrain_epoch.fetch_add(1, Ordering::AcqRel);
    }

    #[inline(always)]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Return a traction factor (Q16.16) at a tile based on mud/cover/cost (1.0 = perfect).
    pub fn traction_at(&self, x: u32, y: u32) -> Q16_16 {
        let idx = match self.index(x, y) {
            Some(i) => i,
            None => return Q16_16::from_f32(1.0),
        };
        let mud = self.mud_strip[idx].load(Ordering::Acquire);
        let cover = self.cover_strip[idx].load(Ordering::Acquire);
        let cost = self.cost_strip[idx].load(Ordering::Acquire);
        let penalty = (mud as u64 / 2)
            .saturating_add(cover as u64 / 4)
            .saturating_add(cost as u64 / 8)
            .min(50_000);
        let factor_q16 = 65_536u64.saturating_sub(penalty);
        Q16_16::from_raw(factor_q16 as i32)
    }

    pub fn set_tile(&mut self, x: u32, y: u32, snap: TerrainSnapshot) {
        if let Some(tile) = self.get_tile(x, y) {
            tile.set_height(snap.height_mm);
            tile.set_mud(snap.mud_q16);
            tile.set_slope(snap.slope_q16);
            tile.set_cover(snap.cover_q16);
            tile.set_material(snap.material);
            if let Some(idx) = self.index(x, y) {
                if let Some(cell) = self.cover_strip.get(idx) {
                    cell.store(snap.cover_q16, Ordering::Release);
                }
                if let Some(cell) = self.mud_strip.get(idx) {
                    cell.store(snap.mud_q16, Ordering::Release);
                }
                if let Some(cell) = self.cost_strip.get(idx) {
                    let cost = compute_cost(snap.slope_q16, snap.mud_q16, snap.cover_q16);
                    cell.store(cost, Ordering::Release);
                }
                if let Some(raw) = self.cover_plain.get_mut(idx) {
                    *raw = snap.cover_q16 as i32;
                }
                if let Some(raw) = self.mud_plain.get_mut(idx) {
                    *raw = snap.mud_q16 as i32;
                }
                if let Some(raw) = self.cost_plain.get_mut(idx) {
                    *raw = compute_cost(snap.slope_q16, snap.mud_q16, snap.cover_q16) as i32;
                }
            }
            self.bump_terrain_epoch();
        }
    }

    pub fn get_tile(&self, x: u32, y: u32) -> Option<&TerrainTileCapsule> {
        let idx = self.index(x, y)?;
        self.tiles.get(idx)
    }

    /// Simple LOS: samples along line and returns (clear, avg_cover_q16).
    ///
    /// Default (scalar) path uses per-tile snapshots.
    /// SIMD path (feature `simd-los`) uses read-only SoA slices gathered via `snapshot_strips`.
    pub fn los_clear(&self, start: (u32, u32), end: (u32, u32)) -> (bool, u32) {
        // Default adapter-driven path.
        let visibility = self.los_adapter.los_visibility(start, end);
        let clear = visibility.raw() >= Q16_16::from_f32(0.999).raw();
        let avg_cover = (Q16_16::ONE.raw().saturating_sub(visibility.raw())) as u32;
        (clear, avg_cover)
    }

    /// LOS that accounts for structure cover overlays. Uses the terrain adapter plus a lightweight
    /// structure AABB pass to accumulate additional cover along the ray.
    pub fn los_clear_with_structures(
        &self,
        start: (u32, u32),
        end: (u32, u32),
        structures: &[crate::structure::StructureSnapshot],
    ) -> (bool, u32) {
        if structures.is_empty() {
            return self.los_clear(start, end);
        }
        let (terrain_clear, terrain_cover) = self.los_clear(start, end);
        let indices = self.compute_ray_indices(start, end);
        if indices.is_empty() {
            return (terrain_clear, terrain_cover);
        }
        let width = self.width as usize;
        let mut structure_sum: u64 = 0;

        for idx in indices.iter().copied() {
            let x = (idx % width) as u32;
            let y = (idx / width) as u32;
            let mut tile_cover = 0u32;
            for s in structures {
                let min_x = s.position_x_q16.saturating_sub(s.half_extent_x_q16) >> 16;
                let max_x = s.position_x_q16.saturating_add(s.half_extent_x_q16) >> 16;
                let min_z = s.position_z_q16.saturating_sub(s.half_extent_z_q16) >> 16;
                let max_z = s.position_z_q16.saturating_add(s.half_extent_z_q16) >> 16;
                if x >= min_x && x <= max_x && y >= min_z && y <= max_z {
                    // Average the four faces; if breached, scale down cover.
                    let mut cov = ((s.cover_q16[0] as u64
                        + s.cover_q16[1] as u64
                        + s.cover_q16[2] as u64
                        + s.cover_q16[3] as u64)
                        / 4) as u32;
                    if s.breach_mask != 0 {
                        cov = ((cov as u64 * 32_768) / 65_536) as u32;
                    }
                    tile_cover = tile_cover.max(cov);
                }
            }
            structure_sum = structure_sum.saturating_add(tile_cover as u64);
        }

        let avg_structure = (structure_sum / indices.len() as u64).min(u32::MAX as u64) as u32;
        let mut combined_cover = terrain_cover.saturating_add(avg_structure);
        if combined_cover > 65_536 {
            combined_cover = 65_536;
        }
        // Treat heavy structural cover as blocking.
        let clear = terrain_clear && avg_structure < 8_192;
        (clear, combined_cover)
    }

    /// Experimental gather-free LOS path: tries AVX2 row/stride/monotonic/staged/contiguous strip loaders before
    /// falling back to the adapter. Only active with `avx2-los` on x86_64.
    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    pub fn los_clear_gather_free(&self, start: (u32, u32), end: (u32, u32)) -> (bool, u32) {
        let indices = self.compute_ray_indices(start, end);
        if indices.is_empty() {
            return (true, 0);
        }
        let len = indices.len();

        unsafe {
            // Pure horizontal can stream a contiguous row.
            if start.1 == end.1 {
                if let Some(res) = self.los_clear_avx2_row(start, end) {
                    return res;
                }
            }
            if let Some(res) = self.los_clear_avx2_const_stride(&indices) {
                return res;
            }
            if let Some(res) = self.los_clear_avx2_monotonic(&indices) {
                return res;
            }
            // Avoid the heaviest staging on extremely long rays; cap at generous threshold so we still
            // hit the AVX2 path for most battlefield spans.
            if len <= 2_048 {
                if let Some(res) = self.los_clear_avx2_staged(&indices) {
                    return res;
                }
                if let Some(res) = self.los_clear_avx2_contiguous(&indices) {
                    return res;
                }
            }
        }

        // Fallback: original adapter path.
        self.los_clear(start, end)
    }

    #[cfg(not(all(feature = "avx2-los", target_arch = "x86_64")))]
    pub fn los_clear_gather_free(&self, start: (u32, u32), end: (u32, u32)) -> (bool, u32) {
        self.los_clear(start, end)
    }

    /// Precompute and store a ray path for re-use; later los_clear will reuse it.
    pub fn precompute_ray(&mut self, start: (u32, u32), end: (u32, u32)) {
        if self
            .ray_cache
            .iter()
            .any(|entry| entry.start == start && entry.end == end)
        {
            return;
        }
        let indices = self.compute_ray_indices(start, end);
        if !indices.is_empty() {
            self.ray_cache.push(RayCacheEntry {
                start,
                end,
                indices: Arc::from(indices),
            });
        }
    }

    /// Build coarse LOD masks (2×/4×) for cover/mud; call after terrain edits.
    pub fn rebuild_lod_masks(&mut self) {
        let w2 = (self.width + 1) / 2;
        let h2 = (self.height + 1) / 2;
        let w4 = (self.width + 3) / 4;
        let h4 = (self.height + 3) / 4;
        let mut cover2 = vec![0u32; (w2 * h2) as usize];
        let mut mud2 = vec![0u32; (w2 * h2) as usize];
        let mut cover4 = vec![0u32; (w4 * h4) as usize];
        let mut mud4 = vec![0u32; (w4 * h4) as usize];

        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(idx) = self.index(x, y) {
                    let cover = self.cover_strip[idx].load(Ordering::Acquire);
                    let mud = self.mud_strip[idx].load(Ordering::Acquire);
                    let x2 = x / 2;
                    let y2 = y / 2;
                    let i2 = (y2 * w2 + x2) as usize;
                    cover2[i2] = cover2[i2].max(cover);
                    mud2[i2] = mud2[i2].max(mud);

                    let x4 = x / 4;
                    let y4 = y / 4;
                    let i4 = (y4 * w4 + x4) as usize;
                    cover4[i4] = cover4[i4].max(cover);
                    mud4[i4] = mud4[i4].max(mud);
                }
            }
        }

        self.lod_masks = Some(LodMasks {
            cover2: Arc::from(cover2),
            mud2: Arc::from(mud2),
            cover4: Arc::from(cover4),
            mud4: Arc::from(mud4),
            w2,
            h2,
            w4,
            h4,
        });
    }

    pub(crate) fn compute_ray_indices(&self, start: (u32, u32), end: (u32, u32)) -> Vec<usize> {
        let (sx, sy) = (start.0 as i32, start.1 as i32);
        let (ex, ey) = (end.0 as i32, end.1 as i32);
        let dx = (ex - sx).abs();
        let dy = (ey - sy).abs();
        let step_x = if sx < ex { 1 } else { -1 };
        let step_y = if sy < ey { 1 } else { -1 };
        let mut err = dx - dy;
        let max_dist = dx.max(dy).max(1);
        let stride = if max_dist > 2_400 {
            8
        } else if max_dist > 1_400 {
            4
        } else if max_dist > 700 {
            2
        } else {
            1
        };
        let steps = (max_dist as usize / stride as usize) + 2;

        let mut x = sx;
        let mut y = sy;
        let mut out = Vec::with_capacity(steps);
        let mut step_ctr = 0;

        loop {
            if step_ctr % stride == 0 {
                if let Some(idx) = self.index(x as u32, y as u32) {
                    out.push(idx);
                }
            }

            if x == ex && y == ey {
                break;
            }

            let err2 = err * 2;
            if err2 > -dy {
                err -= dy;
                x += step_x;
            }
            if err2 < dx {
                err += dx;
                y += step_y;
            }
            step_ctr += 1;
        }

        // Ensure the endpoint is always included even if stride skipped it.
        if let Some(last_idx) = self.index(ex as u32, ey as u32) {
            if out.last().copied() != Some(last_idx) {
                out.push(last_idx);
            }
        }

        out
    }

    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2(&self, indices: &[usize], stride: usize) -> Option<(bool, u32)> {
        if indices.is_empty() {
            return Some((true, 0));
        }
        let mut sum: u64 = 0;
        let mut blocked = false;
        let mut samples: u32 = 0;
        let cover_ptr = self.cover_plain.as_ptr() as *const i32;
        let mud_ptr = self.mud_plain.as_ptr() as *const i32;

        let mut i = 0;
        while i + 8 <= indices.len() {
            let idxs = &indices[i..i + 8];
            let idx_vec = _mm256_setr_epi32(
                idxs[0] as i32,
                idxs[1] as i32,
                idxs[2] as i32,
                idxs[3] as i32,
                idxs[4] as i32,
                idxs[5] as i32,
                idxs[6] as i32,
                idxs[7] as i32,
            );
            let cover = unsafe { _mm256_i32gather_epi32(cover_ptr, idx_vec, 4) };
            let mud = unsafe { _mm256_i32gather_epi32(mud_ptr, idx_vec, 4) };

            let mut cover_buf = [0i32; 8];
            let mut mud_buf = [0i32; 8];
            unsafe {
                _mm256_storeu_si256(cover_buf.as_mut_ptr() as *mut __m256i, cover);
                _mm256_storeu_si256(mud_buf.as_mut_ptr() as *mut __m256i, mud);
            }

            // Prefetch a future block to hide latency on long rays (stride>1 increases spacing).
            let pf_span = if stride <= 2 { 16 } else { 0 };
            if pf_span > 0 {
                let pf_idx = i + pf_span;
                if pf_idx < indices.len() {
                    let next_idx = indices[pf_idx] as isize;
                    unsafe {
                        _mm_prefetch(cover_ptr.offset(next_idx) as *const i8, _MM_HINT_T0);
                        _mm_prefetch(mud_ptr.offset(next_idx) as *const i8, _MM_HINT_T0);
                    }
                }
            }

            for j in 0..8 {
                let c = cover_buf[j] as u32;
                let m = mud_buf[j] as u32;
                sum += c as u64;
                if c > 40_000 || m > 45_000 {
                    blocked = true;
                }
            }
            samples += 8;
            if blocked {
                return Some((false, (sum / samples as u64) as u32));
            }
            i += 8;
        }

        for &idx in &indices[i..] {
            let c = *self.cover_plain.get(idx)?;
            let m = *self.mud_plain.get(idx)?;
            sum += c as u64;
            samples += 1;
            if c > 40_000 || m > 45_000 {
                blocked = true;
                break;
            }
        }

        if samples == 0 {
            Some((true, 0))
        } else {
            Some((!blocked, (sum / samples as u64) as u32))
        }
    }

    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2_row(&self, start: (u32, u32), end: (u32, u32)) -> Option<(bool, u32)> {
        let y = start.1;
        if y >= self.height {
            return None;
        }
        let (sx, ex) = if start.0 <= end.0 {
            (start.0, end.0)
        } else {
            (end.0, start.0)
        };
        let len = (ex.saturating_sub(sx) as usize).saturating_add(1);
        if len == 0 {
            return Some((true, 0));
        }
        let base_idx = self.index(sx, y)?;
        let end_idx = base_idx + len;
        if end_idx > self.cover_plain.len() {
            return None;
        }

        let cover_ptr = self.cover_plain.as_ptr();
        let mud_ptr = self.mud_plain.as_ptr();

        let mut sum: u64 = 0;
        let mut blocked = false;
        let mut i = 0;
        while i + 8 <= len {
            let cover =
                unsafe { _mm256_loadu_si256(cover_ptr.add(base_idx + i) as *const __m256i) };
            let mud = unsafe { _mm256_loadu_si256(mud_ptr.add(base_idx + i) as *const __m256i) };

            let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
            let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
            let any_block = _mm256_or_si256(cover_gt, mud_gt);
            if _mm256_movemask_epi8(any_block) != 0 {
                blocked = true;
            }

            let mut buf = [0i32; 8];
            unsafe { _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, cover) };
            sum += buf.iter().map(|&v| v as u64).sum::<u64>();

            i += 8;
            if blocked {
                let samples = i as u32;
                return Some((false, (sum / samples as u64) as u32));
            }
        }

        let mut samples = i as u32;
        for j in i..len {
            let c = unsafe { *cover_ptr.add(base_idx + j) as u32 };
            let m = unsafe { *mud_ptr.add(base_idx + j) as u32 };
            sum += c as u64;
            samples += 1;
            if c > 40_000 || m > 45_000 {
                blocked = true;
                break;
            }
        }

        Some((
            !blocked,
            if samples > 0 {
                (sum / samples as u64) as u32
            } else {
                0
            },
        ))
    }

    /// Gather-free path for constant-stride rays (e.g., vertical or diagonal with fixed delta).
    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2_const_stride(&self, indices: &[usize]) -> Option<(bool, u32)> {
        if indices.len() < 24 {
            return None; // small rays stay on existing paths
        }
        let first = *indices.first()?;
        let second = *indices.get(1)?;
        let step = second as isize - first as isize;
        if step <= 0 {
            return None;
        }
        // Verify constant positive stride (monotonic ray).
        for (i, &idx) in indices.iter().enumerate().skip(1) {
            let expect = first as isize + (i as isize * step);
            if expect != idx as isize {
                return None;
            }
        }
        let len = indices.len();
        let cover_ptr = self.cover_plain.as_ptr();
        let mud_ptr = self.mud_plain.as_ptr();
        // Bounds check tail
        let last = first as isize + (len as isize - 1) * step;
        if last < 0
            || (last as usize) >= self.cover_plain.len()
            || (last as usize) >= self.mud_plain.len()
        {
            return None;
        }

        let mut sum: u64 = 0;
        let mut blocked = false;
        let mut i = 0usize;
        while i + 8 <= len {
            let mut cover_buf = [0i32; 8];
            let mut mud_buf = [0i32; 8];
            // Load eight samples manually to avoid gathers; step is constant.
            for lane in 0..8 {
                let idx = first as isize + (i as isize + lane as isize) * step;
                cover_buf[lane] = unsafe { *cover_ptr.add(idx as usize) as i32 };
                mud_buf[lane] = unsafe { *mud_ptr.add(idx as usize) as i32 };
            }
            let cover = unsafe { _mm256_loadu_si256(cover_buf.as_ptr() as *const __m256i) };
            let mud = unsafe { _mm256_loadu_si256(mud_buf.as_ptr() as *const __m256i) };
            let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
            let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
            let any_block = _mm256_or_si256(cover_gt, mud_gt);
            if _mm256_movemask_epi8(any_block) != 0 {
                blocked = true;
            }
            sum += cover_buf.iter().map(|&v| v as u64).sum::<u64>();
            i += 8;
            if blocked {
                let samples = i as u32;
                return Some((false, (sum / samples as u64) as u32));
            }
        }
        let mut samples = i as u32;
        for lane in i..len {
            let idx = first as isize + lane as isize * step;
            let c = unsafe { *cover_ptr.add(idx as usize) as u32 };
            let m = unsafe { *mud_ptr.add(idx as usize) as u32 };
            sum += c as u64;
            samples += 1;
            if c > 40_000 || m > 45_000 {
                blocked = true;
                break;
            }
        }
        Some((
            !blocked,
            if samples > 0 {
                (sum / samples as u64) as u32
            } else {
                0
            },
        ))
    }

    /// Gather-free AVX2 path for arbitrary rays: stage 8 indices at a time into stack buffers to avoid hardware gathers.
    /// Requires indices to be in-bounds but makes no monotonicity assumption.
    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2_staged(&self, indices: &[usize]) -> Option<(bool, u32)> {
        if indices.len() < 24 {
            return None;
        }
        let max_idx = *indices.iter().max()?;
        if max_idx >= self.cover_plain.len() || max_idx >= self.mud_plain.len() {
            return None;
        }

        let cover_ptr = self.cover_plain.as_ptr();
        let mud_ptr = self.mud_plain.as_ptr();
        let indices_ptr = indices.as_ptr();
        let mut sum: u64 = 0;
        let mut blocked = false;
        let mut i = 0usize;
        let len = indices.len();

        // Process in tiles to improve cache locality.
        const TILE: usize = 64;
        while i < len {
            let tile_end = core::cmp::min(i + TILE, len);

            // Vectorized inner loop over the current tile.
            while i + 8 <= tile_end {
                let (cover, mud, chunk_sum) = unsafe {
                    let base = indices_ptr.add(i);
                    let idx0 = *base as usize;
                    let idx1 = *base.add(1) as usize;
                    let idx2 = *base.add(2) as usize;
                    let idx3 = *base.add(3) as usize;
                    let idx4 = *base.add(4) as usize;
                    let idx5 = *base.add(5) as usize;
                    let idx6 = *base.add(6) as usize;
                    let idx7 = *base.add(7) as usize;

                    let c0 = *cover_ptr.add(idx0) as i32;
                    let c1 = *cover_ptr.add(idx1) as i32;
                    let c2 = *cover_ptr.add(idx2) as i32;
                    let c3 = *cover_ptr.add(idx3) as i32;
                    let c4 = *cover_ptr.add(idx4) as i32;
                    let c5 = *cover_ptr.add(idx5) as i32;
                    let c6 = *cover_ptr.add(idx6) as i32;
                    let c7 = *cover_ptr.add(idx7) as i32;
                    let m0 = *mud_ptr.add(idx0) as i32;
                    let m1 = *mud_ptr.add(idx1) as i32;
                    let m2 = *mud_ptr.add(idx2) as i32;
                    let m3 = *mud_ptr.add(idx3) as i32;
                    let m4 = *mud_ptr.add(idx4) as i32;
                    let m5 = *mud_ptr.add(idx5) as i32;
                    let m6 = *mud_ptr.add(idx6) as i32;
                    let m7 = *mud_ptr.add(idx7) as i32;

                    let cover = _mm256_set_epi32(c7, c6, c5, c4, c3, c2, c1, c0);
                    let mud = _mm256_set_epi32(m7, m6, m5, m4, m3, m2, m1, m0);
                    let chunk_sum = c0 as u64
                        + c1 as u64
                        + c2 as u64
                        + c3 as u64
                        + c4 as u64
                        + c5 as u64
                        + c6 as u64
                        + c7 as u64;
                    (cover, mud, chunk_sum)
                };

                let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
                let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
                let any_block = _mm256_or_si256(cover_gt, mud_gt);
                if _mm256_movemask_epi8(any_block) != 0 {
                    blocked = true;
                }

                sum += chunk_sum;
                i += 8;
                if blocked {
                    let samples = i as u32;
                    return Some((false, (sum / samples as u64) as u32));
                }
            }

            // Scalar tail of the tile.
            while i < tile_end {
                let idx = unsafe { *indices_ptr.add(i) };
                let c = unsafe { *cover_ptr.add(idx) as u32 };
                let m = unsafe { *mud_ptr.add(idx) as u32 };
                sum += c as u64;
                i += 1;
                if c > 40_000 || m > 45_000 {
                    let samples = i as u32;
                    return Some((false, (sum / samples as u64) as u32));
                }
            }
        }

        let samples = len as u32;
        Some((
            !blocked,
            if samples > 0 {
                (sum / samples as u64) as u32
            } else {
                0
            },
        ))
    }

    /// Gather-free AVX2 path for monotonic rays with varying step (e.g., mixed width/diag Bresenham).
    ///
    /// Loads 8 samples at a time into stack lanes to avoid hardware gathers while keeping
    /// vectorized threshold checks and accumulation.
    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2_monotonic(&self, indices: &[usize]) -> Option<(bool, u32)> {
        if indices.len() < 32 {
            return None;
        }
        let first = *indices.first()?;
        let last = *indices.last()?;
        // Require monotonic order to keep cache-friendly forward or reverse scans.
        let increasing = last >= first;
        let mut prev = first;
        for &idx in indices.iter().skip(1) {
            if increasing {
                if idx < prev {
                    return None;
                }
            } else if idx > prev {
                return None;
            }
            prev = idx;
        }

        let cover_ptr = self.cover_plain.as_ptr();
        let mud_ptr = self.mud_plain.as_ptr();
        let mut sum: u64 = 0;
        let mut blocked = false;
        let mut i = 0usize;
        let len = indices.len();

        while i + 8 <= len {
            let mut cover_buf = [0i32; 8];
            let mut mud_buf = [0i32; 8];
            // Manual loads keep addresses explicit and avoid the gather micro-op.
            for lane in 0..8 {
                let idx = *indices.get(i + lane)?;
                cover_buf[lane] = unsafe { *cover_ptr.add(idx) as i32 };
                mud_buf[lane] = unsafe { *mud_ptr.add(idx) as i32 };
            }
            let cover = unsafe { _mm256_loadu_si256(cover_buf.as_ptr() as *const __m256i) };
            let mud = unsafe { _mm256_loadu_si256(mud_buf.as_ptr() as *const __m256i) };
            let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
            let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
            let any_block = _mm256_or_si256(cover_gt, mud_gt);
            if _mm256_movemask_epi8(any_block) != 0 {
                blocked = true;
            }
            sum += cover_buf.iter().map(|&v| v as u64).sum::<u64>();
            i += 8;
            if blocked {
                let samples = i as u32;
                return Some((false, (sum / samples as u64) as u32));
            }
        }

        let mut samples = i as u32;
        for lane in i..len {
            let idx = *indices.get(lane)?;
            let c = unsafe { *cover_ptr.add(idx) as u32 };
            let m = unsafe { *mud_ptr.add(idx) as u32 };
            sum += c as u64;
            samples += 1;
            if c > 40_000 || m > 45_000 {
                blocked = true;
                break;
            }
        }

        Some((
            !blocked,
            if samples > 0 {
                (sum / samples as u64) as u32
            } else {
                0
            },
        ))
    }

    /// Gather-free path that stages a full ray into contiguous scratch buffers, then runs AVX2 reductions over them.
    /// Avoids hardware gathers entirely; trades an upfront copy for predictable sequential loads.
    #[cfg(all(feature = "avx2-los", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn los_clear_avx2_contiguous(&self, indices: &[usize]) -> Option<(bool, u32)> {
        if indices.len() < 48 {
            return None;
        }
        let max_idx = *indices.iter().max()?;
        if max_idx >= self.cover_plain.len() || max_idx >= self.mud_plain.len() {
            return None;
        }

        let cover_ptr = self.cover_plain.as_ptr();
        let mud_ptr = self.mud_plain.as_ptr();
        let epoch = self.terrain_epoch.load(Ordering::Acquire);
        LOS_SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            let len = indices.len();
            let fingerprint = fingerprint_indices(indices);
            let cache_hit = scratch.cache.valid
                && scratch.cache.epoch == epoch
                && scratch.cache.len == len
                && scratch.cache.fingerprint == fingerprint;
            scratch.ensure_len(len);
            let mut update_cache = false;
            let result = {
                // Split through raw pointers to appease the borrow checker while keeping SoA slices.
                let cover_ptr_mut = scratch.cover.as_mut_ptr();
                let mud_ptr_mut = scratch.mud.as_mut_ptr();
                let cover_buf = unsafe { core::slice::from_raw_parts_mut(cover_ptr_mut, len) };
                let mud_buf = unsafe { core::slice::from_raw_parts_mut(mud_ptr_mut, len) };
                if len == 0 {
                    return Some((true, 0));
                }

                let mut sum: u64 = 0;
                let mut samples: u32 = 0;
                let mut blocked = false;
                let mut i = 0usize;
                let mut acc = _mm256_setzero_si256();

                if cache_hit {
                    while i + 8 <= len {
                        let cover = unsafe {
                            _mm256_loadu_si256(cover_buf.as_ptr().add(i) as *const __m256i)
                        };
                        let mud = unsafe {
                            _mm256_loadu_si256(mud_buf.as_ptr().add(i) as *const __m256i)
                        };

                        let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
                        let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
                        let any_block = _mm256_or_si256(cover_gt, mud_gt);

                        acc = _mm256_add_epi32(acc, cover);
                        samples += 8;
                        i += 8;

                        if _mm256_movemask_epi8(any_block) != 0 {
                            blocked = true;
                            break;
                        }
                    }

                    sum += unsafe { sum_m256i(acc) };

                    if !blocked {
                        while i < len {
                            let c = cover_buf[i] as u32;
                            let m = mud_buf[i] as u32;
                            sum += c as u64;
                            samples += 1;
                            if c > 40_000 || m > 45_000 {
                                blocked = true;
                                break;
                            }
                            i += 1;
                        }
                    }
                } else {
                    // Fused staging + reduction: copy into scratch once while reducing.
                    let mut accumulate = true;
                    while i + 8 <= len {
                        let mut cover_lane = [0i32; 8];
                        let mut mud_lane = [0i32; 8];
                        for lane in 0..8 {
                            let idx = *indices.get(i + lane)?;
                            let c = unsafe { *cover_ptr.add(idx) as i32 };
                            let m = unsafe { *mud_ptr.add(idx) as i32 };
                            cover_lane[lane] = c;
                            mud_lane[lane] = m;
                            cover_buf[i + lane] = c;
                            mud_buf[i + lane] = m;
                        }
                        let cover =
                            unsafe { _mm256_loadu_si256(cover_lane.as_ptr() as *const __m256i) };
                        let mud =
                            unsafe { _mm256_loadu_si256(mud_lane.as_ptr() as *const __m256i) };
                        let cover_gt = _mm256_cmpgt_epi32(cover, _mm256_set1_epi32(40_000));
                        let mud_gt = _mm256_cmpgt_epi32(mud, _mm256_set1_epi32(45_000));
                        let any_block = _mm256_or_si256(cover_gt, mud_gt);

                        if accumulate {
                            acc = _mm256_add_epi32(acc, cover);
                            samples += 8;
                        }
                        i += 8;

                        if _mm256_movemask_epi8(any_block) != 0 {
                            blocked = true;
                            accumulate = false;
                            // Continue staging the rest to keep cache warm even if blocked.
                            break;
                        }
                    }

                    sum += unsafe { sum_m256i(acc) };

                    while i < len {
                        let idx = *indices.get(i)?;
                        let c = unsafe { *cover_ptr.add(idx) as i32 };
                        let m = unsafe { *mud_ptr.add(idx) as i32 };
                        cover_buf[i] = c;
                        mud_buf[i] = m;
                        if accumulate {
                            sum += c as u64;
                            samples += 1;
                            if c > 40_000 || m as u32 > 45_000 {
                                blocked = true;
                                accumulate = false;
                            }
                        }
                        i += 1;
                    }

                    update_cache = true;
                }

                if samples == 0 {
                    Some((true, 0))
                } else {
                    Some((!blocked, (sum / samples as u64) as u32))
                }
            };

            if update_cache {
                scratch.cache = LosStageCache {
                    epoch,
                    fingerprint,
                    len,
                    valid: true,
                };
            }
            result
        })
    }

    #[cfg(feature = "simd-los")]
    #[inline(always)]
    fn xy_from_index(&self, idx: usize) -> (u32, u32) {
        let x = (idx % self.width as usize) as u32;
        let y = (idx / self.width as usize) as u32;
        (x, y)
    }

    #[inline(always)]
    pub fn sample_cover_mud(&self, x: u32, y: u32, stride: usize) -> (u32, u32) {
        if stride >= 4 {
            if let Some(mask) = &self.lod_masks {
                let ix = (x / 4).min(mask.w4.saturating_sub(1));
                let iy = (y / 4).min(mask.h4.saturating_sub(1));
                let idx = (iy * mask.w4 + ix) as usize;
                return (
                    *mask.cover4.get(idx).unwrap_or(&0),
                    *mask.mud4.get(idx).unwrap_or(&0),
                );
            }
        } else if stride >= 2 {
            if let Some(mask) = &self.lod_masks {
                let ix = (x / 2).min(mask.w2.saturating_sub(1));
                let iy = (y / 2).min(mask.h2.saturating_sub(1));
                let idx = (iy * mask.w2 + ix) as usize;
                return (
                    *mask.cover2.get(idx).unwrap_or(&0),
                    *mask.mud2.get(idx).unwrap_or(&0),
                );
            }
        }
        // Default fine sample.
        if let Some(idx) = self.index(x, y) {
            (
                self.cover_strip[idx].load(Ordering::Acquire),
                self.mud_strip[idx].load(Ordering::Acquire),
            )
        } else {
            (0, 0)
        }
    }

    #[inline(always)]
    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * self.width as usize + x as usize)
        } else {
            None
        }
    }

    /// Expose precomputed terrain cost (0 = no penalty).
    pub fn cost_at(&self, x: u32, y: u32) -> Option<u32> {
        let idx = self.index(x, y)?;
        self.cost_strip.get(idx).map(|c| c.load(Ordering::Acquire))
    }

    /// Apply an artillery crater (cover reduced, mud increased) within a radius (tile units).
    ///
    /// Returns the number of tiles updated. Deterministic and lock-free (atomic stores on strips).
    pub fn apply_crater_q16(
        &mut self,
        center_x: u32,
        center_y: u32,
        radius_tiles: u32,
        cover_delta_q16: i32,
        mud_delta_q16: i32,
    ) -> usize {
        let mut updated = 0;
        let r2 = (radius_tiles as i64).saturating_mul(radius_tiles as i64);
        let width = self.width as i64;
        let height = self.height as i64;
        for dy in -(radius_tiles as i64)..=(radius_tiles as i64) {
            for dx in -(radius_tiles as i64)..=(radius_tiles as i64) {
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let tx = center_x as i64 + dx;
                let ty = center_y as i64 + dy;
                if tx < 0 || ty < 0 || tx >= width || ty >= height {
                    continue;
                }
                let idx = self.index(tx as u32, ty as u32).expect("in bounds");
                let tile = &self.tiles[idx];
                let cover = tile.cover_q16.load(Ordering::Acquire) as i64;
                let mud = tile.mud_q16.load(Ordering::Acquire) as i64;
                let new_cover = (cover + cover_delta_q16 as i64).clamp(0, u32::MAX as i64) as u32;
                let new_mud = (mud + mud_delta_q16 as i64).clamp(0, u32::MAX as i64) as u32;
                tile.cover_q16.store(new_cover as u64, Ordering::Release);
                tile.mud_q16.store(new_mud as u64, Ordering::Release);
                self.cover_strip[idx].store(new_cover, Ordering::Release);
                self.mud_strip[idx].store(new_mud, Ordering::Release);
                if let Some(c) = self.cover_plain.get_mut(idx) {
                    *c = new_cover as i32;
                }
                if let Some(m) = self.mud_plain.get_mut(idx) {
                    *m = new_mud as i32;
                }
                let slope = tile.slope_q16.load(Ordering::Acquire) as u32;
                let cost = compute_cost(slope, new_mud, new_cover);
                self.cost_strip[idx].store(cost, Ordering::Release);
                if let Some(c) = self.cost_plain.get_mut(idx) {
                    *c = cost as i32;
                }
                updated += 1;
            }
        }
        if updated > 0 {
            self.bump_terrain_epoch();
        }
        updated
    }

    /// Zero-copy overlay view for renderer/analytics (cover/cost strips + LOD masks).
    pub fn overlay_view(&self) -> TerrainOverlayView<'_> {
        let lod2 = self.lod_masks.as_ref().map(|m| TerrainLodView {
            width: m.w2,
            height: m.h2,
            stride: 2,
            cover: m.cover2.as_ref(),
            mud: m.mud2.as_ref(),
        });
        let lod4 = self.lod_masks.as_ref().map(|m| TerrainLodView {
            width: m.w4,
            height: m.h4,
            stride: 4,
            cover: m.cover4.as_ref(),
            mud: m.mud4.as_ref(),
        });
        TerrainOverlayView {
            width: self.width,
            height: self.height,
            cover_strip: &self.cover_strip,
            cost_strip: &self.cost_strip,
            lod2,
            lod4,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

#[derive(Debug, Clone)]
struct RayCacheEntry {
    start: (u32, u32),
    end: (u32, u32),
    indices: Arc<[usize]>,
}

#[derive(Debug)]
struct LodMasks {
    cover2: Arc<[u32]>,
    mud2: Arc<[u32]>,
    cover4: Arc<[u32]>,
    mud4: Arc<[u32]>,
    w2: u32,
    h2: u32,
    w4: u32,
    h4: u32,
}

/// Read-only view over terrain strips for renderer ingestion.
pub struct TerrainOverlayView<'a> {
    pub width: u32,
    pub height: u32,
    pub cover_strip: &'a [AtomicU32],
    pub cost_strip: &'a [AtomicU32],
    pub lod2: Option<TerrainLodView<'a>>,
    pub lod4: Option<TerrainLodView<'a>>,
}

/// LOD mask view (2× or 4×).
pub struct TerrainLodView<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub cover: &'a [u32],
    pub mud: &'a [u32],
}

#[inline(always)]
fn compute_cost(slope_q16: u32, mud_q16: u32, cover_q16: u32) -> u32 {
    // Simple heuristic: combine slope + mud + cover as additive penalties (clamped).
    let slope_pen = (slope_q16 / 4).min(30_000); // steeper slopes hurt speed
    let mud_pen = (mud_q16 / 2).min(30_000); // mud hurts more
    let cover_pen = (cover_q16 / 8).min(20_000); // dense cover slows a bit
    slope_pen + mud_pen + cover_pen
}

verify_alignment_only!(TerrainGridCapsule, 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_updates() {
        let tile = TerrainTileCapsule::new(1000, 0, 500, 200, 1);
        let snap = tile.snapshot();
        assert_eq!(snap.height_mm, 1000);
        tile.set_height(1200);
        tile.set_mud(300);
        tile.set_cover(750);
        tile.set_material(9);
        let snap2 = tile.snapshot();
        assert_eq!(snap2.height_mm, 1200);
        assert_eq!(snap2.mud_q16, 300);
        assert_eq!(snap2.cover_q16, 750);
        assert_eq!(snap2.material, 9);
    }

    #[test]
    fn los_sampling_basic() {
        let grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let (clear, cover) = grid.los_clear((0, 0), (3, 3));
        assert!(clear);
        assert_eq!(cover, 0);
    }

    #[test]
    fn crater_application_reduces_cover_and_increases_mud() {
        let mut grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 5_000,
                cover_q16: 10_000,
                mud_q16: 5_000,
                material: 0,
            },
        );
        let updated = grid.apply_crater_q16(1, 1, 1, -2_000, 3_000);
        assert!(updated > 0);
        let idx = grid.index(1, 1).unwrap();
        assert!(grid.cover_strip[idx].load(Ordering::Acquire) <= 8_000);
        assert!(grid.mud_strip[idx].load(Ordering::Acquire) >= 8_000);
        let cost = grid.cost_strip[idx].load(Ordering::Acquire);
        assert!(cost > 0);
    }

    #[test]
    fn set_tile_updates_cover_and_material() {
        let mut grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 10,
                mud_q16: 0,
                material: 1,
            },
        );
        grid.set_tile(
            1,
            1,
            TerrainSnapshot {
                height_mm: 5,
                slope_q16: 2,
                cover_q16: 700,
                mud_q16: 1,
                material: 4,
            },
        );
        let snap = grid.get_tile(1, 1).unwrap().snapshot();
        assert_eq!(snap.cover_q16, 700);
        assert_eq!(snap.material, 4);
    }
}
