//! Terminal Post-Processing Effects Capsule
//!
//! T7 Heterogeneous tier GPU compute shader-based post-processing effects
//! for terminal rendering: CRT, bloom, chromatic aberration, vignette, etc.
//!
//! # UCE34 Compliance
//!
//! - Q10: T7 Heterogeneous tier (GPU compute shaders for post-processing)
//! - Q33: 100% lockfree (atomic effect parameters)
//! - Q34: Effect state audit trail (generation counter)
//!
//! # Design
//!
//! - **Size**: 128B (cache-line aligned)
//! - **Alignment**: 64B (cache-line)
//! - **Coordination**: Atomic parameter updates for GPU shader uniforms
//! - **Performance**: <20ns parameter set, <50ns uniforms pack, <5ns time update
//!
//! # Examples
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::render::effects::{
//!     TerminalEffectsCapsule, EffectFlags, EffectUniforms,
//! };
//!
//! // Create effects capsule
//! let effects = TerminalEffectsCapsule::new();
//!
//! // Enable CRT effects
//! effects.enable_effects(EffectFlags::CRT_FULL);
//! effects.set_crt_params(0.8, 0.02, 0.5);
//!
//! // Enable bloom
//! effects.enable_effects(EffectFlags::BLOOM);
//! effects.set_bloom_params(0.7, 1.5, 3.0);
//!
//! // Update time for animated effects
//! effects.update_time(0.016); // 60 FPS
//!
//! // Pack uniforms for GPU upload
//! let uniforms = effects.get_shader_uniforms();
//! ```

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// EFFECT FLAGS
// ============================================================================

/// Effect type flags (can be combined via bitwise OR)
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule::terminal::render::effects::EffectFlags;
///
/// // Single effect
/// let crt = EffectFlags::CRT_SCANLINES;
///
/// // Combined effects
/// let combined = EffectFlags::CRT_FULL | EffectFlags::BLOOM;
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct EffectFlags(pub u32);

impl EffectFlags {
    /// No effects enabled
    pub const NONE: Self = Self(0);

    // Individual CRT effects
    /// CRT scanlines effect
    pub const CRT_SCANLINES: Self = Self(1 << 0);
    /// CRT screen curvature effect
    pub const CRT_CURVATURE: Self = Self(1 << 1);
    /// CRT phosphor persistence effect
    pub const CRT_PHOSPHOR: Self = Self(1 << 2);

    /// Bloom/glow effect
    pub const BLOOM: Self = Self(1 << 3);
    /// Chromatic aberration (color fringing)
    pub const CHROMATIC_ABERRATION: Self = Self(1 << 4);
    /// Vignette (darkened corners)
    pub const VIGNETTE: Self = Self(1 << 5);
    /// Film grain noise
    pub const NOISE: Self = Self(1 << 6);
    /// Color grading/LUT
    pub const COLOR_GRADING: Self = Self(1 << 7);

    /// All CRT effects combined
    pub const CRT_FULL: Self = Self(0b111);

    /// Combine two effect flags
    #[inline]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if flag contains effect
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl core::ops::BitOr for EffectFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for EffectFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for EffectFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

// ============================================================================
// EFFECT UNIFORMS (GPU SHADER DATA)
// ============================================================================

/// Uniforms for effect compute shader
///
/// Packed for efficient GPU upload. 16-byte aligned for GPU requirements.
///
/// # Layout
///
/// - Control: 16 bytes (enabled_flags, time, frame, _pad)
/// - CRT params: 16 bytes (scanline, curvature, phosphor, _pad)
/// - Bloom params: 16 bytes (threshold, intensity, radius, _pad)
/// - Color params: 16 bytes (gamma, contrast, saturation, temperature)
/// - Vignette params: 16 bytes (intensity, radius, _pad, _pad)
/// - Chroma params: 16 bytes (r_offset, g_offset, b_offset, _pad)
///
/// Total: 96 bytes (6 × 16-byte registers)
#[derive(Copy, Clone, Debug)]
#[repr(C, align(16))]
pub struct EffectUniforms {
    /// Enabled effect flags (bitfield)
    pub enabled_flags: u32,
    /// Time accumulator (seconds, for animated effects)
    pub time: f32,
    /// Frame counter (for noise seed)
    pub frame: u32,
    /// Padding to 16 bytes
    pub _pad0: u32,

    /// CRT parameters: [scanline_intensity, curvature, phosphor_blur, _pad]
    pub crt: [f32; 4],

    /// Bloom parameters: [threshold, intensity, radius, _pad]
    pub bloom: [f32; 4],

    /// Color grading: [gamma, contrast, saturation, temperature]
    pub color: [f32; 4],

    /// Vignette: [intensity, radius, _pad, _pad]
    pub vignette: [f32; 4],

    /// Chromatic aberration: [red_offset, green_offset, blue_offset, _pad]
    pub chroma: [f32; 4],
}

impl Default for EffectUniforms {
    #[inline]
    fn default() -> Self {
        Self {
            enabled_flags: 0,
            time: 0.0,
            frame: 0,
            _pad0: 0,
            crt: [0.0; 4],
            bloom: [0.0; 4],
            color: [1.0, 1.0, 1.0, 6500.0], // Default: gamma=1, contrast=1, saturation=1, temp=6500K
            vignette: [0.0; 4],
            chroma: [0.0; 4],
        }
    }
}

const _: () = assert!(core::mem::size_of::<EffectUniforms>() == 96);
const _: () = assert!(core::mem::align_of::<EffectUniforms>() == 16);

// ============================================================================
// TERMINAL EFFECTS CAPSULE
// ============================================================================

/// T7 Heterogeneous - GPU post-processing effects capsule
///
/// Manages post-processing effect parameters for GPU compute shaders.
/// All parameters are lockfree atomics for safe concurrent updates from
/// main thread while GPU renders.
///
/// # UCE34 Compliance
///
/// - Q10: T7 Heterogeneous tier (GPU compute shaders)
/// - Q33: 100% lockfree (atomic effect parameters)
/// - Q34: Effect state audit trail (generation counter)
///
/// # Performance Targets (B32)
///
/// - Parameter set: <20ns
/// - Uniforms pack: <50ns
/// - Time update: <5ns
///
/// # Memory Layout
///
/// ```text
/// Offset | Field              | Size | Alignment
/// -------|--------------------|----- |----------
/// 0      | state              | 8    | 8
/// 8      | crt_params         | 8    | 8
/// 16     | bloom_params       | 8    | 8
/// 24     | color_params       | 8    | 8
/// 32     | vn_params          | 8    | 8
/// 40     | chroma_params      | 8    | 8
/// 48     | frame_count        | 4    | 4
/// 52     | time_accumulator   | 4    | 4
/// 56     | workgroup_size     | 4    | 4
/// 60     | dispatch_count     | 4    | 4
/// 64     | _pad               | 8    | -
/// -------|--------------------|----- |----------
/// Total: 128 bytes
/// ```
#[repr(C, align(64))]
pub struct TerminalEffectsCapsule {
    // Effect state (64 bits)
    /// Generation (32 bits) | enabled_effects (32 bits)
    state: AtomicU64,

    // CRT effect parameters (Q8.8 fixed-point, 64 bits)
    /// Scanline intensity (16) | curvature (16) | phosphor_blur (16) | _pad (16)
    crt_params: AtomicU64,

    // Bloom parameters (Q8.8 fixed-point, 64 bits)
    /// Threshold (16) | intensity (16) | radius (16) | _pad (16)
    bloom_params: AtomicU64,

    // Color grading (Q8.8 fixed-point, 64 bits)
    /// Gamma (16) | contrast (16) | saturation (16) | temperature (16)
    color_params: AtomicU64,

    // Vignette/Noise (Q8.8 fixed-point, 64 bits)
    /// Vignette intensity (16) | vignette_radius (16) | noise_intensity (16) | noise_speed (16)
    vn_params: AtomicU64,

    // Chromatic aberration (Q8.8 fixed-point, 64 bits)
    /// Red offset (16) | green offset (16) | blue offset (16) | _pad (16)
    chroma_params: AtomicU64,

    // Timing for animated effects
    /// Frame count for noise seed
    frame_count: AtomicU32,
    /// Time accumulator (Q16.16 fixed-point, seconds)
    time_accumulator: AtomicU32,

    // Compute shader dispatch info
    /// Workgroup size X (10 bits) | Y (10 bits) | Z (10 bits) | _pad (2 bits)
    workgroup_size: AtomicU32,
    /// Dispatch count (last frame, for profiling)
    dispatch_count: AtomicU32,

    /// Padding to 128 bytes
    _pad: [u8; 56],
}

const _: () = assert!(core::mem::size_of::<TerminalEffectsCapsule>() == 128);
const _: () = assert!(core::mem::align_of::<TerminalEffectsCapsule>() == 64);

// ============================================================================
// HELPER FUNCTIONS (Q8.8 FIXED-POINT CONVERSION)
// ============================================================================

/// Convert f32 to Q8.8 fixed-point (16 bits)
#[inline]
fn f32_to_q8_8(val: f32) -> u16 {
    let fixed = (val * 256.0) as i32;
    let clamped = if fixed < -32768 {
        -32768
    } else if fixed > 32767 {
        32767
    } else {
        fixed
    };
    clamped as u16
}

/// Convert Q8.8 fixed-point to f32
#[inline]
fn q8_8_to_f32(val: u16) -> f32 {
    (val as i16) as f32 / 256.0
}

/// Pack four Q8.8 values into u64
#[inline]
fn pack_q8_8_x4(a: f32, b: f32, c: f32, d: f32) -> u64 {
    let a16 = f32_to_q8_8(a) as u64;
    let b16 = f32_to_q8_8(b) as u64;
    let c16 = f32_to_q8_8(c) as u64;
    let d16 = f32_to_q8_8(d) as u64;
    a16 | (b16 << 16) | (c16 << 32) | (d16 << 48)
}

/// Unpack u64 to four Q8.8 values as f32
#[inline]
fn unpack_q8_8_x4(packed: u64) -> [f32; 4] {
    [
        q8_8_to_f32((packed & 0xFFFF) as u16),
        q8_8_to_f32(((packed >> 16) & 0xFFFF) as u16),
        q8_8_to_f32(((packed >> 32) & 0xFFFF) as u16),
        q8_8_to_f32(((packed >> 48) & 0xFFFF) as u16),
    ]
}

/// Convert f32 to Q16.16 fixed-point (32 bits)
#[inline]
fn f32_to_q16_16(val: f32) -> u32 {
    (val * 65536.0) as i32 as u32
}

/// Convert Q16.16 fixed-point to f32
#[inline]
fn q16_16_to_f32(val: u32) -> f32 {
    val as i32 as f32 / 65536.0
}

// ============================================================================
// IMPLEMENTATION
// ============================================================================

impl TerminalEffectsCapsule {
    /// Create new effects capsule with defaults
    ///
    /// # Performance
    ///
    /// - Time: <10ns (constant initialization)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use atomic_capsule::terminal::render::effects::TerminalEffectsCapsule;
    ///
    /// let effects = TerminalEffectsCapsule::new();
    /// assert_eq!(effects.generation(), 0);
    /// ```
    #[inline]
    pub fn new() -> Self {
        // Default color grading: gamma=1.0, contrast=1.0, saturation=1.0, temperature=6500K
        let default_color = pack_q8_8_x4(1.0, 1.0, 1.0, 25.390625); // 6500/256 = 25.390625

        Self {
            state: AtomicU64::new(0),
            crt_params: AtomicU64::new(0),
            bloom_params: AtomicU64::new(0),
            color_params: AtomicU64::new(default_color),
            vn_params: AtomicU64::new(0),
            chroma_params: AtomicU64::new(0),
            frame_count: AtomicU32::new(0),
            time_accumulator: AtomicU32::new(0),
            workgroup_size: AtomicU32::new(0x00100010), // Default: 16x16x1
            dispatch_count: AtomicU32::new(0),
            _pad: [0; 56],
        }
    }

    /// Enable effects atomically
    ///
    /// # Performance
    ///
    /// - Time: <20ns (atomic RMW + generation increment)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.enable_effects(EffectFlags::CRT_FULL | EffectFlags::BLOOM);
    /// ```
    #[inline]
    pub fn enable_effects(&self, flags: EffectFlags) {
        self.state.fetch_update(Ordering::Release, Ordering::Acquire, |state| {
            let gen = (state >> 32) as u32;
            let mut enabled = (state & 0xFFFF_FFFF) as u32;
            enabled |= flags.0;
            Some((gen.wrapping_add(1) as u64) << 32 | enabled as u64)
        }).ok();
    }

    /// Disable effects atomically
    ///
    /// # Performance
    ///
    /// - Time: <20ns (atomic RMW + generation increment)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.disable_effects(EffectFlags::BLOOM);
    /// ```
    #[inline]
    pub fn disable_effects(&self, flags: EffectFlags) {
        self.state.fetch_update(Ordering::Release, Ordering::Acquire, |state| {
            let gen = (state >> 32) as u32;
            let mut enabled = (state & 0xFFFF_FFFF) as u32;
            enabled &= !flags.0;
            Some((gen.wrapping_add(1) as u64) << 32 | enabled as u64)
        }).ok();
    }

    /// Set CRT parameters
    ///
    /// # Parameters
    ///
    /// - `scanline`: Scanline intensity (0.0 = none, 1.0 = full)
    /// - `curvature`: Screen curvature (0.0 = flat, 0.1 = typical)
    /// - `phosphor`: Phosphor persistence/blur (0.0 = none, 1.0 = full)
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_crt_params(0.8, 0.02, 0.5);
    /// ```
    #[inline]
    pub fn set_crt_params(&self, scanline: f32, curvature: f32, phosphor: f32) {
        let packed = pack_q8_8_x4(scanline, curvature, phosphor, 0.0);
        self.crt_params.store(packed, Ordering::Release);
    }

    /// Set bloom parameters
    ///
    /// # Parameters
    ///
    /// - `threshold`: Brightness threshold (0.0-1.0)
    /// - `intensity`: Bloom intensity multiplier
    /// - `radius`: Bloom radius in pixels
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_bloom_params(0.7, 1.5, 3.0);
    /// ```
    #[inline]
    pub fn set_bloom_params(&self, threshold: f32, intensity: f32, radius: f32) {
        let packed = pack_q8_8_x4(threshold, intensity, radius, 0.0);
        self.bloom_params.store(packed, Ordering::Release);
    }

    /// Set color grading parameters
    ///
    /// # Parameters
    ///
    /// - `gamma`: Gamma correction (1.0 = linear, <1 = brighter, >1 = darker)
    /// - `contrast`: Contrast multiplier (1.0 = normal)
    /// - `saturation`: Saturation multiplier (1.0 = normal, 0.0 = grayscale)
    /// - `temperature`: Color temperature in Kelvin (6500 = daylight)
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_color_grading(1.2, 1.1, 0.9, 5500.0);
    /// ```
    #[inline]
    pub fn set_color_grading(&self, gamma: f32, contrast: f32, saturation: f32, temperature: f32) {
        // Store temperature / 256 to fit in Q8.8
        let packed = pack_q8_8_x4(gamma, contrast, saturation, temperature / 256.0);
        self.color_params.store(packed, Ordering::Release);
    }

    /// Set vignette parameters
    ///
    /// # Parameters
    ///
    /// - `intensity`: Vignette darkness (0.0 = none, 1.0 = full black)
    /// - `radius`: Vignette radius (0.0 = center only, 1.0 = full screen)
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_vignette(0.3, 0.8);
    /// ```
    #[inline]
    pub fn set_vignette(&self, intensity: f32, radius: f32) {
        let current = self.vn_params.load(Ordering::Acquire);
        let noise_params = ((current >> 32) & 0xFFFF_FFFF) as u32; // Preserve noise params
        let vignette = pack_q8_8_x4(intensity, radius, 0.0, 0.0) & 0xFFFF_FFFF;
        let packed = vignette | ((noise_params as u64) << 32);
        self.vn_params.store(packed, Ordering::Release);
    }

    /// Set noise parameters
    ///
    /// # Parameters
    ///
    /// - `intensity`: Noise intensity (0.0 = none, 1.0 = full)
    /// - `speed`: Noise animation speed (0.0 = static, 1.0 = fast)
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_noise(0.05, 0.5);
    /// ```
    #[inline]
    pub fn set_noise(&self, intensity: f32, speed: f32) {
        let current = self.vn_params.load(Ordering::Acquire);
        let vignette_params = (current & 0xFFFF_FFFF) as u32; // Preserve vignette params
        let noise = pack_q8_8_x4(intensity, speed, 0.0, 0.0) & 0xFFFF_FFFF;
        let packed = vignette_params as u64 | ((noise as u64) << 32);
        self.vn_params.store(packed, Ordering::Release);
    }

    /// Set chromatic aberration parameters
    ///
    /// # Parameters
    ///
    /// - `red`: Red channel offset in pixels
    /// - `green`: Green channel offset in pixels
    /// - `blue`: Blue channel offset in pixels
    ///
    /// # Performance
    ///
    /// - Time: <15ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_chromatic_aberration(-1.0, 0.0, 1.0);
    /// ```
    #[inline]
    pub fn set_chromatic_aberration(&self, red: f32, green: f32, blue: f32) {
        let packed = pack_q8_8_x4(red, green, blue, 0.0);
        self.chroma_params.store(packed, Ordering::Release);
    }

    /// Update time accumulator for animated effects
    ///
    /// # Performance
    ///
    /// - Time: <5ns (atomic fetch_add)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.update_time(0.016); // 60 FPS
    /// ```
    #[inline]
    pub fn update_time(&self, delta_seconds: f32) {
        let delta_fixed = f32_to_q16_16(delta_seconds);
        self.time_accumulator.fetch_add(delta_fixed, Ordering::Relaxed);
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get shader uniforms (packed for GPU upload)
    ///
    /// # Performance
    ///
    /// - Time: <50ns (6 atomic loads + unpacking)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let uniforms = effects.get_shader_uniforms();
    /// // Upload uniforms to GPU...
    /// ```
    #[inline]
    pub fn get_shader_uniforms(&self) -> EffectUniforms {
        // Load all state atomically (Acquire for happens-before)
        let state = self.state.load(Ordering::Acquire);
        let crt_packed = self.crt_params.load(Ordering::Acquire);
        let bloom_packed = self.bloom_params.load(Ordering::Acquire);
        let color_packed = self.color_params.load(Ordering::Acquire);
        let vn_packed = self.vn_params.load(Ordering::Acquire);
        let chroma_packed = self.chroma_params.load(Ordering::Acquire);
        let time_fixed = self.time_accumulator.load(Ordering::Relaxed);
        let frame = self.frame_count.load(Ordering::Relaxed);

        // Unpack state
        let enabled_flags = (state & 0xFFFF_FFFF) as u32;

        // Unpack parameters
        let crt = unpack_q8_8_x4(crt_packed);
        let bloom = unpack_q8_8_x4(bloom_packed);

        let color_unpacked = unpack_q8_8_x4(color_packed);
        let color = [
            color_unpacked[0], // gamma
            color_unpacked[1], // contrast
            color_unpacked[2], // saturation
            color_unpacked[3] * 256.0, // temperature (restore from Q8.8)
        ];

        let vn_unpacked = unpack_q8_8_x4(vn_packed);
        let vignette = [
            vn_unpacked[0], // intensity
            vn_unpacked[1], // radius
            vn_unpacked[2], // noise intensity
            vn_unpacked[3], // noise speed
        ];

        let chroma = unpack_q8_8_x4(chroma_packed);

        EffectUniforms {
            enabled_flags,
            time: q16_16_to_f32(time_fixed),
            frame,
            _pad0: 0,
            crt,
            bloom,
            color,
            vignette,
            chroma,
        }
    }

    /// Get enabled effects
    ///
    /// # Performance
    ///
    /// - Time: <5ns (atomic load)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let enabled = effects.enabled_effects();
    /// if enabled.contains(EffectFlags::BLOOM) {
    ///     // Bloom is enabled
    /// }
    /// ```
    #[inline]
    pub fn enabled_effects(&self) -> EffectFlags {
        let state = self.state.load(Ordering::Acquire);
        EffectFlags((state & 0xFFFF_FFFF) as u32)
    }

    /// Get generation counter (for change detection)
    ///
    /// # Performance
    ///
    /// - Time: <5ns (atomic load)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let gen = effects.generation();
    /// ```
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        (state >> 32) as u32
    }

    /// Set workgroup size for compute shader dispatch
    ///
    /// # Parameters
    ///
    /// - `x`: Workgroup X dimension (1-1024)
    /// - `y`: Workgroup Y dimension (1-1024)
    /// - `z`: Workgroup Z dimension (1-1024)
    ///
    /// # Performance
    ///
    /// - Time: <10ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.set_workgroup_size(16, 16, 1);
    /// ```
    #[inline]
    pub fn set_workgroup_size(&self, x: u16, y: u16, z: u16) {
        let packed = (x as u32 & 0x3FF)
                   | ((y as u32 & 0x3FF) << 10)
                   | ((z as u32 & 0x3FF) << 20);
        self.workgroup_size.store(packed, Ordering::Release);
    }

    /// Record dispatch count (for profiling)
    ///
    /// # Performance
    ///
    /// - Time: <5ns (atomic store)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// effects.record_dispatch(1);
    /// ```
    #[inline]
    pub fn record_dispatch(&self, count: u32) {
        self.dispatch_count.store(count, Ordering::Release);
    }
}

impl Default for TerminalEffectsCapsule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: UNIT TESTS (10 tests)
    // ========================================================================

    #[test]
    fn test_new() {
        let effects = TerminalEffectsCapsule::new();
        assert_eq!(effects.generation(), 0);
        assert_eq!(effects.enabled_effects(), EffectFlags::NONE);
    }

    #[test]
    fn test_enable_disable_effects() {
        let effects = TerminalEffectsCapsule::new();

        // Enable CRT
        effects.enable_effects(EffectFlags::CRT_FULL);
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_SCANLINES));
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_CURVATURE));
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_PHOSPHOR));
        assert_eq!(effects.generation(), 1);

        // Enable bloom
        effects.enable_effects(EffectFlags::BLOOM);
        assert!(effects.enabled_effects().contains(EffectFlags::BLOOM));
        assert_eq!(effects.generation(), 2);

        // Disable CRT scanlines only
        effects.disable_effects(EffectFlags::CRT_SCANLINES);
        assert!(!effects.enabled_effects().contains(EffectFlags::CRT_SCANLINES));
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_CURVATURE));
        assert!(effects.enabled_effects().contains(EffectFlags::BLOOM));
        assert_eq!(effects.generation(), 3);
    }

    #[test]
    fn test_crt_params() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_crt_params(0.8, 0.02, 0.5);
        let uniforms = effects.get_shader_uniforms();

        // Q8.8 precision: ~0.004
        assert!((uniforms.crt[0] - 0.8).abs() < 0.01);
        assert!((uniforms.crt[1] - 0.02).abs() < 0.01);
        assert!((uniforms.crt[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bloom_params() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_bloom_params(0.7, 1.5, 3.0);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.bloom[0] - 0.7).abs() < 0.01);
        assert!((uniforms.bloom[1] - 1.5).abs() < 0.01);
        assert!((uniforms.bloom[2] - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_color_grading() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_color_grading(1.2, 1.1, 0.9, 5500.0);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.color[0] - 1.2).abs() < 0.01);
        assert!((uniforms.color[1] - 1.1).abs() < 0.01);
        assert!((uniforms.color[2] - 0.9).abs() < 0.01);
        assert!((uniforms.color[3] - 5500.0).abs() < 50.0); // Temperature precision
    }

    #[test]
    fn test_vignette() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_vignette(0.3, 0.8);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.vignette[0] - 0.3).abs() < 0.01);
        assert!((uniforms.vignette[1] - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_noise() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_noise(0.05, 0.5);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.vignette[2] - 0.05).abs() < 0.01); // Noise stored in vignette array
        assert!((uniforms.vignette[3] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_chromatic_aberration() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_chromatic_aberration(-1.0, 0.0, 1.0);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.chroma[0] - (-1.0)).abs() < 0.01);
        assert!((uniforms.chroma[1] - 0.0).abs() < 0.01);
        assert!((uniforms.chroma[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_time_update() {
        let effects = TerminalEffectsCapsule::new();

        effects.update_time(0.016);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.time - 0.016).abs() < 0.001); // Q16.16 precision
        assert_eq!(uniforms.frame, 1);

        effects.update_time(0.016);
        let uniforms = effects.get_shader_uniforms();

        assert!((uniforms.time - 0.032).abs() < 0.001);
        assert_eq!(uniforms.frame, 2);
    }

    #[test]
    fn test_workgroup_size() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_workgroup_size(8, 8, 1);
        let packed = effects.workgroup_size.load(Ordering::Acquire);

        let x = packed & 0x3FF;
        let y = (packed >> 10) & 0x3FF;
        let z = (packed >> 20) & 0x3FF;

        assert_eq!(x, 8);
        assert_eq!(y, 8);
        assert_eq!(z, 1);
    }

    // ========================================================================
    // Q8-Q14: PROPERTY TESTS (4 tests)
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_property_parameter_ranges() {
        use std::vec::Vec;

        let effects = TerminalEffectsCapsule::new();

        // Test Q8.8 range (-128.0 to 127.99609375)
        let test_values: Vec<f32> = vec![
            -128.0, -10.5, -1.0, -0.5, 0.0, 0.5, 1.0, 10.5, 127.0
        ];

        for &val in &test_values {
            effects.set_crt_params(val, val, val);
            let uniforms = effects.get_shader_uniforms();

            // Q8.8 precision: ~0.004
            assert!((uniforms.crt[0] - val).abs() < 0.5, "Failed for {}", val);
        }
    }

    #[test]
    fn test_property_flag_combinations() {
        let effects = TerminalEffectsCapsule::new();

        // Test all single flags
        let flags = [
            EffectFlags::CRT_SCANLINES,
            EffectFlags::CRT_CURVATURE,
            EffectFlags::CRT_PHOSPHOR,
            EffectFlags::BLOOM,
            EffectFlags::CHROMATIC_ABERRATION,
            EffectFlags::VIGNETTE,
            EffectFlags::NOISE,
            EffectFlags::COLOR_GRADING,
        ];

        for flag in &flags {
            effects.enable_effects(*flag);
            assert!(effects.enabled_effects().contains(*flag));
        }

        // Test combined flags
        let combined = EffectFlags::CRT_FULL | EffectFlags::BLOOM | EffectFlags::VIGNETTE;
        effects.disable_effects(EffectFlags(0xFFFF_FFFF)); // Clear all
        effects.enable_effects(combined);

        assert!(effects.enabled_effects().contains(EffectFlags::CRT_SCANLINES));
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_CURVATURE));
        assert!(effects.enabled_effects().contains(EffectFlags::CRT_PHOSPHOR));
        assert!(effects.enabled_effects().contains(EffectFlags::BLOOM));
        assert!(effects.enabled_effects().contains(EffectFlags::VIGNETTE));
    }

    #[test]
    fn test_property_generation_increments() {
        let effects = TerminalEffectsCapsule::new();
        assert_eq!(effects.generation(), 0);

        effects.enable_effects(EffectFlags::BLOOM);
        assert_eq!(effects.generation(), 1);

        effects.disable_effects(EffectFlags::BLOOM);
        assert_eq!(effects.generation(), 2);

        effects.enable_effects(EffectFlags::CRT_FULL);
        assert_eq!(effects.generation(), 3);
    }

    #[test]
    fn test_property_time_accumulation() {
        let effects = TerminalEffectsCapsule::new();

        let mut expected_time = 0.0_f32;
        let mut expected_frame = 0_u32;

        for _ in 0..100 {
            effects.update_time(0.016);
            expected_time += 0.016;
            expected_frame += 1;

            let uniforms = effects.get_shader_uniforms();
            assert!((uniforms.time - expected_time).abs() < 0.1); // Q16.16 accumulation
            assert_eq!(uniforms.frame, expected_frame);
        }
    }

    // ========================================================================
    // Q15-Q21: INTEGRATION TESTS (4 tests)
    // ========================================================================

    #[test]
    fn test_integration_full_pipeline() {
        let effects = TerminalEffectsCapsule::new();

        // Configure all effects
        effects.enable_effects(
            EffectFlags::CRT_FULL
            | EffectFlags::BLOOM
            | EffectFlags::CHROMATIC_ABERRATION
            | EffectFlags::VIGNETTE
            | EffectFlags::NOISE
            | EffectFlags::COLOR_GRADING
        );

        effects.set_crt_params(0.8, 0.02, 0.5);
        effects.set_bloom_params(0.7, 1.5, 3.0);
        effects.set_color_grading(1.2, 1.1, 0.9, 5500.0);
        effects.set_vignette(0.3, 0.8);
        effects.set_noise(0.05, 0.5);
        effects.set_chromatic_aberration(-1.0, 0.0, 1.0);
        effects.update_time(0.016);

        let uniforms = effects.get_shader_uniforms();

        // Verify all parameters present
        assert_ne!(uniforms.enabled_flags, 0);
        assert!(uniforms.time > 0.0);
        assert_eq!(uniforms.frame, 1);
        assert!(uniforms.crt[0] > 0.0);
        assert!(uniforms.bloom[0] > 0.0);
        assert!(uniforms.color[0] > 0.0);
        assert!(uniforms.vignette[0] > 0.0);
        assert!(uniforms.chroma[0] < 0.0); // Negative red offset
    }

    #[test]
    fn test_integration_uniform_packing() {
        let effects = TerminalEffectsCapsule::new();

        effects.set_crt_params(1.0, 2.0, 3.0);
        effects.set_bloom_params(4.0, 5.0, 6.0);

        let uniforms = effects.get_shader_uniforms();

        // Verify 16-byte alignment
        assert_eq!(core::mem::align_of_val(&uniforms), 16);
        assert_eq!(core::mem::size_of_val(&uniforms), 96);

        // Verify array layout
        assert_eq!(uniforms.crt.len(), 4);
        assert_eq!(uniforms.bloom.len(), 4);
        assert_eq!(uniforms.color.len(), 4);
        assert_eq!(uniforms.vignette.len(), 4);
        assert_eq!(uniforms.chroma.len(), 4);
    }

    #[test]
    fn test_integration_concurrent_updates() {
        let effects = TerminalEffectsCapsule::new();

        // Simulate concurrent parameter updates
        effects.set_crt_params(0.5, 0.01, 0.25);
        effects.enable_effects(EffectFlags::CRT_FULL);
        effects.set_bloom_params(0.8, 2.0, 4.0);
        effects.enable_effects(EffectFlags::BLOOM);
        effects.update_time(0.016);

        // Should be able to read consistent state
        let uniforms = effects.get_shader_uniforms();

        assert!(uniforms.enabled_flags & EffectFlags::CRT_FULL.0 != 0);
        assert!(uniforms.enabled_flags & EffectFlags::BLOOM.0 != 0);
        assert!(uniforms.time > 0.0);
    }

    #[test]
    fn test_integration_vignette_noise_independence() {
        let effects = TerminalEffectsCapsule::new();

        // Set vignette
        effects.set_vignette(0.5, 0.7);
        let uniforms1 = effects.get_shader_uniforms();
        assert!((uniforms1.vignette[0] - 0.5).abs() < 0.01);
        assert!((uniforms1.vignette[1] - 0.7).abs() < 0.01);

        // Set noise (should preserve vignette)
        effects.set_noise(0.1, 0.3);
        let uniforms2 = effects.get_shader_uniforms();
        assert!((uniforms2.vignette[0] - 0.5).abs() < 0.01); // Vignette preserved
        assert!((uniforms2.vignette[1] - 0.7).abs() < 0.01);
        assert!((uniforms2.vignette[2] - 0.1).abs() < 0.01); // Noise added
        assert!((uniforms2.vignette[3] - 0.3).abs() < 0.01);

        // Update vignette again (should preserve noise)
        effects.set_vignette(0.6, 0.8);
        let uniforms3 = effects.get_shader_uniforms();
        assert!((uniforms3.vignette[0] - 0.6).abs() < 0.01); // Vignette updated
        assert!((uniforms3.vignette[1] - 0.8).abs() < 0.01);
        assert!((uniforms3.vignette[2] - 0.1).abs() < 0.01); // Noise preserved
        assert!((uniforms3.vignette[3] - 0.3).abs() < 0.01);
    }
}
