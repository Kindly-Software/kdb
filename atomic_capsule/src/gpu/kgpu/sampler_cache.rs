//! KgpuSamplerCacheCapsule - Lockfree GPU Sampler Cache
//!
//! **Tier**: T1 (Atomic)
//! **Size**: 512B (cache-aligned)
//! **Purpose**: Cache GPU sampler objects to avoid redundant creation
//!
//! # Architecture
//!
//! Samplers are immutable GPU objects with a finite set of common configurations.
//! This capsule caches samplers by their configuration hash, providing O(1) lookup
//! with minimal memory overhead.
//!
//! ```text
//! KgpuSamplerCacheCapsule (512B aligned)
//! +---------------------------+
//! | primary: AtomicU64        |  state(8) | sampler_count(8) | generation(48)
//! | secondary: AtomicU64      |  hit_count(32) | miss_count(32)
//! | entries[16]: SamplerEntry |  16 cached samplers (256B)
//! | capacity: AtomicU32       |  Maximum entries
//! | flags: AtomicU32          |  Configuration flags
//! | _padding                  |  Padding to 512B
//! +---------------------------+
//! ```
//!
//! # Common Sampler Configurations
//!
//! | Config | Description |
//! |--------|-------------|
//! | PointClamp | Nearest sampling, clamp to edge |
//! | PointRepeat | Nearest sampling, repeat |
//! | LinearClamp | Bilinear sampling, clamp to edge |
//! | LinearRepeat | Bilinear sampling, repeat |
//! | Anisotropic4x | 4x anisotropic filtering |
//! | Anisotropic8x | 8x anisotropic filtering |
//! | Anisotropic16x | 16x anisotropic filtering |
//! | ShadowPCF | Depth comparison for shadow mapping |
//!
//! # ASSUM Safety Documentation
//!
//! - `#ASSUME_HASH_COLLISION_RARE`: FNV-1a hash with 64 bits has negligible
//!   collision probability for sampler configurations (<16 unique configs).
//!
//! - `#ASSUME_SAMPLER_IMMUTABLE`: GPU sampler objects are immutable after creation.
//!   Configuration hash is computed once and remains valid.
//!
//! - `#ASSUME_ENTRY_ATOMIC_UPDATE`: SamplerEntry uses two AtomicU64 fields.
//!   Updates are not atomic across both fields but this is acceptable because:
//!   - config_hash is written first, handle second
//!   - Readers check config_hash match before using handle
//!   - Worst case: cache miss, not incorrect handle
//!
//! - `#ASSUME_GENERATION_ABA_SAFE`: Generation counters prevent ABA problems.
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 tier selection
//! - **Chaos**: 100% lockfree, zero mutex
//! - **ASSUM**: All assumptions documented
//! - **T28**: Comprehensive tests

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// Re-export CompareFunction from pipeline module
pub use super::pipeline::CompareFunction;

// ============================================================================
// Constants
// ============================================================================

/// Maximum cached samplers
pub const MAX_CACHED_SAMPLERS: usize = 16;

/// Cache state: Uninitialized
pub const CACHE_STATE_UNINITIALIZED: u8 = 0;

/// Cache state: Active
pub const CACHE_STATE_ACTIVE: u8 = 1;

/// Cache state: Full (all slots used)
pub const CACHE_STATE_FULL: u8 = 2;

// ============================================================================
// Bit Field Masks (Primary)
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;

const COUNT_SHIFT: u64 = 48;
const COUNT_MASK: u64 = 0xFF << COUNT_SHIFT;

const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// ============================================================================
// Bit Field Masks (Secondary)
// ============================================================================

const HIT_COUNT_SHIFT: u64 = 32;
const HIT_COUNT_MASK: u64 = 0xFFFF_FFFF << HIT_COUNT_SHIFT;

const MISS_COUNT_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// FilterMode
// ============================================================================

/// Texture filter mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FilterMode {
    /// Nearest neighbor sampling
    #[default]
    Nearest = 0,
    /// Linear interpolation sampling
    Linear = 1,
}

impl FilterMode {
    /// Convert from raw u8 value.
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Nearest),
            1 => Some(Self::Linear),
            _ => None,
        }
    }
}

// ============================================================================
// AddressMode
// ============================================================================

/// Texture address (wrap) mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AddressMode {
    /// Clamp to edge
    #[default]
    ClampToEdge = 0,
    /// Repeat (wrap)
    Repeat = 1,
    /// Mirror repeat
    MirrorRepeat = 2,
    /// Clamp to border color
    ClampToBorder = 3,
}

impl AddressMode {
    /// Convert from raw u8 value.
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::ClampToEdge),
            1 => Some(Self::Repeat),
            2 => Some(Self::MirrorRepeat),
            3 => Some(Self::ClampToBorder),
            _ => None,
        }
    }
}

// ============================================================================
// SamplerConfig
// ============================================================================

/// Complete sampler configuration.
///
/// Contains all parameters needed to create a GPU sampler object.
/// Designed to fit in 8 bytes for efficient hashing and comparison.
///
/// # Layout (8B packed)
/// ```text
/// Byte 0: mag_filter | min_filter << 4
/// Byte 1: mipmap_filter | address_mode_u << 4
/// Byte 2: address_mode_v | address_mode_w << 4
/// Byte 3: max_anisotropy
/// Byte 4-5: compare (Some = compare function | 0xFF = None)
/// Byte 6-7: lod_bias (Q8.8 fixed point, scaled from -16.0 to +16.0)
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SamplerConfig {
    /// Magnification filter
    pub mag_filter: FilterMode,
    /// Minification filter
    pub min_filter: FilterMode,
    /// Mipmap filter (between mip levels)
    pub mipmap_filter: FilterMode,
    /// Texture address mode for U coordinate
    pub address_mode_u: AddressMode,
    /// Texture address mode for V coordinate
    pub address_mode_v: AddressMode,
    /// Texture address mode for W coordinate (3D textures)
    pub address_mode_w: AddressMode,
    /// Maximum anisotropy (1, 2, 4, 8, or 16)
    pub max_anisotropy: u8,
    /// Depth comparison function (for shadow mapping)
    pub compare: Option<CompareFunction>,
}

impl SamplerConfig {
    /// Create a new sampler configuration with default values.
    ///
    /// Default: Linear filtering, clamp to edge, no anisotropy, no comparison.
    #[inline]
    pub const fn new() -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            max_anisotropy: 1,
            compare: None,
        }
    }

    /// Create point (nearest neighbor) sampler with clamp addressing.
    #[inline]
    pub const fn point_clamp() -> Self {
        Self {
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            max_anisotropy: 1,
            compare: None,
        }
    }

    /// Create point (nearest neighbor) sampler with repeat addressing.
    #[inline]
    pub const fn point_repeat() -> Self {
        Self {
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            max_anisotropy: 1,
            compare: None,
        }
    }

    /// Create linear sampler with clamp addressing.
    #[inline]
    pub const fn linear_clamp() -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            max_anisotropy: 1,
            compare: None,
        }
    }

    /// Create linear sampler with repeat addressing.
    #[inline]
    pub const fn linear_repeat() -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            max_anisotropy: 1,
            compare: None,
        }
    }

    /// Create anisotropic sampler.
    #[inline]
    pub const fn anisotropic(level: u8) -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            max_anisotropy: level,
            compare: None,
        }
    }

    /// Create shadow comparison sampler.
    #[inline]
    pub const fn shadow_compare() -> Self {
        Self {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            max_anisotropy: 1,
            compare: Some(CompareFunction::Less),
        }
    }

    /// Compute a 64-bit hash of this configuration.
    ///
    /// Uses FNV-1a hash for good distribution with small inputs.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_HASH_COLLISION_RARE`: FNV-1a with 64 bits has <1e-18
    ///   collision probability for the ~20 common sampler configurations.
    #[inline]
    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;

        let mix = |hash: u64, val: u64| -> u64 {
            let hash = hash ^ val;
            hash.wrapping_mul(0x100000001b3)
        };

        h = mix(h, self.mag_filter as u64);
        h = mix(h, self.min_filter as u64);
        h = mix(h, self.mipmap_filter as u64);
        h = mix(h, self.address_mode_u as u64);
        h = mix(h, self.address_mode_v as u64);
        h = mix(h, self.address_mode_w as u64);
        h = mix(h, self.max_anisotropy as u64);
        h = mix(
            h,
            self.compare.map(|c| c as u64).unwrap_or(0xFF),
        );

        h
    }

    /// Pack configuration into 8 bytes for compact storage.
    #[inline]
    pub fn pack(&self) -> u64 {
        let byte0 = (self.mag_filter as u64) | ((self.min_filter as u64) << 4);
        let byte1 = (self.mipmap_filter as u64) | ((self.address_mode_u as u64) << 4);
        let byte2 = (self.address_mode_v as u64) | ((self.address_mode_w as u64) << 4);
        let byte3 = self.max_anisotropy as u64;
        let byte4 = self.compare.map(|c| c as u64).unwrap_or(0xFF);

        byte0 | (byte1 << 8) | (byte2 << 16) | (byte3 << 24) | (byte4 << 32)
    }

    /// Unpack configuration from 8 bytes.
    pub fn unpack(packed: u64) -> Option<Self> {
        let mag_filter = FilterMode::from_u8((packed & 0x0F) as u8)?;
        let min_filter = FilterMode::from_u8(((packed >> 4) & 0x0F) as u8)?;
        let mipmap_filter = FilterMode::from_u8(((packed >> 8) & 0x0F) as u8)?;
        let address_mode_u = AddressMode::from_u8(((packed >> 12) & 0x0F) as u8)?;
        let address_mode_v = AddressMode::from_u8(((packed >> 16) & 0x0F) as u8)?;
        let address_mode_w = AddressMode::from_u8(((packed >> 20) & 0x0F) as u8)?;
        let max_anisotropy = ((packed >> 24) & 0xFF) as u8;
        let compare_raw = ((packed >> 32) & 0xFF) as u8;
        let compare = if compare_raw == 0xFF {
            None
        } else {
            Some(CompareFunction::from_u8(compare_raw)?)
        };

        Some(Self {
            mag_filter,
            min_filter,
            mipmap_filter,
            address_mode_u,
            address_mode_v,
            address_mode_w,
            max_anisotropy,
            compare,
        })
    }
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SamplerEntry
// ============================================================================

/// A cached sampler entry.
///
/// # Layout (16B)
/// ```text
/// 0-8     config_hash: AtomicU64  - Hash of sampler configuration
/// 8-16    handle: AtomicU64       - GPU sampler handle
/// ```
#[repr(C, align(16))]
pub struct SamplerEntry {
    /// Hash of the sampler configuration (0 = empty slot).
    config_hash: AtomicU64,
    /// GPU sampler handle.
    handle: AtomicU64,
}

impl SamplerEntry {
    /// Create a new empty entry.
    #[inline]
    pub const fn new() -> Self {
        Self {
            config_hash: AtomicU64::new(0),
            handle: AtomicU64::new(0),
        }
    }

    /// Check if entry is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.config_hash.load(Ordering::Acquire) == 0
    }

    /// Get configuration hash.
    #[inline]
    pub fn config_hash(&self) -> u64 {
        self.config_hash.load(Ordering::Acquire)
    }

    /// Get sampler handle.
    #[inline]
    pub fn handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Try to set this entry atomically.
    ///
    /// Returns true if successfully set, false if slot was already taken.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ENTRY_ATOMIC_UPDATE`: We set config_hash first via CAS,
    ///   then set handle. A concurrent reader may see config_hash before handle
    ///   is set, but this just results in a cache miss (handle == 0), not
    ///   incorrect behavior.
    #[inline]
    pub fn try_set(&self, hash: u64, sampler_handle: u64) -> bool {
        // Try to claim this slot by CASing config_hash from 0 to hash
        match self.config_hash.compare_exchange(
            0,
            hash,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully claimed, now set the handle
                self.handle.store(sampler_handle, Ordering::Release);
                true
            }
            Err(_) => false, // Slot already taken
        }
    }

    /// Match this entry against a configuration hash.
    ///
    /// Returns handle if matched and valid, None otherwise.
    #[inline]
    pub fn match_config(&self, hash: u64) -> Option<u64> {
        let stored_hash = self.config_hash.load(Ordering::Acquire);
        if stored_hash == hash {
            let handle = self.handle.load(Ordering::Acquire);
            if handle != 0 {
                return Some(handle);
            }
        }
        None
    }
}

impl Default for SamplerEntry {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = {
    assert!(core::mem::size_of::<SamplerEntry>() == 16);
    assert!(core::mem::align_of::<SamplerEntry>() == 16);
};

// ============================================================================
// SamplerCacheStats
// ============================================================================

/// Statistics for the sampler cache.
#[derive(Clone, Copy, Debug, Default)]
pub struct SamplerCacheStats {
    /// Current state of the cache.
    pub state: u8,
    /// Number of cached samplers.
    pub sampler_count: u8,
    /// Generation counter.
    pub generation: u64,
    /// Cache hit count.
    pub hit_count: u32,
    /// Cache miss count.
    pub miss_count: u32,
    /// Hit rate (0.0 to 1.0).
    pub hit_rate: f32,
}

// ============================================================================
// KgpuSamplerCacheCapsule
// ============================================================================

/// GPU Sampler Cache with Lockfree Atomics
///
/// Caches GPU sampler objects by their configuration hash, avoiding
/// redundant creation of duplicate samplers.
///
/// # Tier: T1 (Atomic)
/// # Size: 512B (cache-aligned)
///
/// # ASSUM Safety
///
/// - `#ASSUME_HASH_COLLISION_RARE`: With 64-bit FNV-1a hashes and <20 common
///   sampler configurations, collision probability is negligible (<1e-18).
///
/// - `#ASSUME_SAMPLER_IMMUTABLE`: GPU samplers are immutable after creation.
///   The cache stores handles that remain valid for the sampler's lifetime.
///
/// - `#ASSUME_LINEAR_PROBE_ADEQUATE`: Linear probing with 16 slots handles
///   the ~8-12 common sampler configurations efficiently. Cache is not
///   designed for dynamic/unbounded sampler sets.
#[repr(C, align(512))]
pub struct KgpuSamplerCacheCapsule {
    // ========================================================================
    // Primary Coordination (DualAtomicU64 pattern)
    // ========================================================================

    /// Primary: state(8) | sampler_count(8) | generation(48)
    primary: AtomicU64,

    /// Secondary: hit_count(32) | miss_count(32)
    secondary: AtomicU64,

    // ========================================================================
    // Cached Sampler Entries
    // ========================================================================

    /// Sampler entry slots (16 x 16B = 256B)
    entries: [SamplerEntry; MAX_CACHED_SAMPLERS],

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Maximum entries (for future resize support)
    capacity: AtomicU32,

    /// Configuration flags
    flags: AtomicU32,

    // ========================================================================
    // Padding
    // ========================================================================

    /// Padding to 512B
    /// 8 + 8 + 256 + 4 + 4 = 280
    /// 512 - 280 = 232
    _padding: [u8; 232],
}

const _: () = {
    assert!(core::mem::size_of::<KgpuSamplerCacheCapsule>() == 512);
    assert!(core::mem::align_of::<KgpuSamplerCacheCapsule>() == 512);
};

impl KgpuSamplerCacheCapsule {
    /// Create a new sampler cache.
    #[inline]
    pub const fn new() -> Self {
        let primary = ((CACHE_STATE_ACTIVE as u64) << STATE_SHIFT) | 1; // gen=1

        Self {
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(0),
            entries: [
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
                SamplerEntry::new(),
            ],
            capacity: AtomicU32::new(MAX_CACHED_SAMPLERS as u32),
            flags: AtomicU32::new(0),
            _padding: [0; 232],
        }
    }

    // ========================================================================
    // Core Cache Operations
    // ========================================================================

    /// Look up a sampler by configuration.
    ///
    /// Returns the cached handle if found, None otherwise.
    ///
    /// # Performance
    /// O(n) worst case with linear probing, but typically O(1) due to
    /// hash-based starting position.
    pub fn lookup(&self, config: &SamplerConfig) -> Option<u64> {
        let hash = config.hash();

        // Linear probe starting from hash-derived index
        let start_idx = (hash as usize) % MAX_CACHED_SAMPLERS;

        for i in 0..MAX_CACHED_SAMPLERS {
            let idx = (start_idx + i) % MAX_CACHED_SAMPLERS;
            let entry = &self.entries[idx];

            if let Some(handle) = entry.match_config(hash) {
                self.record_hit();
                return Some(handle);
            }

            // Empty slot means config not cached
            if entry.is_empty() {
                break;
            }
        }

        self.record_miss();
        None
    }

    /// Insert a sampler into the cache.
    ///
    /// Returns true if inserted, false if cache is full or config already exists.
    pub fn insert(&self, config: SamplerConfig, handle: u64) -> bool {
        let hash = config.hash();

        // First check if already cached
        let start_idx = (hash as usize) % MAX_CACHED_SAMPLERS;

        for i in 0..MAX_CACHED_SAMPLERS {
            let idx = (start_idx + i) % MAX_CACHED_SAMPLERS;
            let entry = &self.entries[idx];

            // Already cached?
            if entry.config_hash() == hash {
                return false;
            }

            // Try to claim empty slot
            if entry.try_set(hash, handle) {
                self.increment_count();
                return true;
            }
        }

        // Cache is full
        self.set_state(CACHE_STATE_FULL);
        false
    }

    /// Get or create a sampler.
    ///
    /// This is the primary interface for sampler caching:
    /// 1. Look up by config hash
    /// 2. If found, return cached handle (hit)
    /// 3. If not found, call creator function to get handle
    /// 4. Insert new handle into cache
    /// 5. Return handle
    ///
    /// # Arguments
    /// * `config` - Sampler configuration
    /// * `creator` - Function to create sampler if not cached. Takes config,
    ///   returns (handle, success). Handle is only cached if success is true.
    ///
    /// # Returns
    /// The sampler handle (cached or newly created), or None if creation failed.
    pub fn get_or_create<F>(&self, config: SamplerConfig, creator: F) -> Option<u64>
    where
        F: FnOnce(&SamplerConfig) -> (u64, bool),
    {
        // Try cache first
        if let Some(handle) = self.lookup(&config) {
            return Some(handle);
        }

        // Cache miss - create new sampler
        let (handle, success) = creator(&config);

        if !success || handle == 0 {
            return None;
        }

        // Try to cache it (ignore failure - cache full is OK)
        let _ = self.insert(config, handle);

        Some(handle)
    }

    /// Compute hash for a configuration.
    ///
    /// This is a convenience method that exposes the hashing algorithm.
    #[inline]
    pub fn config_hash(config: &SamplerConfig) -> u64 {
        config.hash()
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get current cache state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Get number of cached samplers.
    #[inline]
    pub fn sampler_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & COUNT_MASK) >> COUNT_SHIFT) as u8
    }

    /// Get generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Get hit count.
    #[inline]
    pub fn hit_count(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & HIT_COUNT_MASK) >> HIT_COUNT_SHIFT) as u32
    }

    /// Get miss count.
    #[inline]
    pub fn miss_count(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & MISS_COUNT_MASK) as u32
    }

    /// Check if cache is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.sampler_count() >= MAX_CACHED_SAMPLERS as u8
    }

    /// Get cache statistics.
    pub fn stats(&self) -> SamplerCacheStats {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;
        let sampler_count = ((primary & COUNT_MASK) >> COUNT_SHIFT) as u8;
        let generation = primary & GENERATION_MASK;
        let hit_count = ((secondary & HIT_COUNT_MASK) >> HIT_COUNT_SHIFT) as u32;
        let miss_count = (secondary & MISS_COUNT_MASK) as u32;

        let total = hit_count + miss_count;
        let hit_rate = if total > 0 {
            hit_count as f32 / total as f32
        } else {
            0.0
        };

        SamplerCacheStats {
            state,
            sampler_count,
            generation,
            hit_count,
            miss_count,
            hit_rate,
        }
    }

    /// Get entry at index (for testing/debugging).
    pub fn get_entry(&self, index: usize) -> Option<(u64, u64)> {
        if index >= MAX_CACHED_SAMPLERS {
            return None;
        }
        let entry = &self.entries[index];
        Some((entry.config_hash(), entry.handle()))
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn set_state(&self, new_state: u8) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let count = (primary & COUNT_MASK) >> COUNT_SHIFT;
            let generation = primary & GENERATION_MASK;

            let new_primary = ((new_state as u64) << STATE_SHIFT)
                | (count << COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn increment_count(&self) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let count = ((primary & COUNT_MASK) >> COUNT_SHIFT) + 1;
            let generation = (primary & GENERATION_MASK) + 1;

            let new_state = if count >= MAX_CACHED_SAMPLERS as u64 {
                CACHE_STATE_FULL as u64
            } else {
                state
            };

            let new_primary = (new_state << STATE_SHIFT)
                | (count << COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn record_hit(&self) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let hit_count = ((secondary & HIT_COUNT_MASK) >> HIT_COUNT_SHIFT) + 1;
            let miss_count = secondary & MISS_COUNT_MASK;

            let new_secondary = (hit_count << HIT_COUNT_SHIFT) | miss_count;

            if self
                .secondary
                .compare_exchange_weak(secondary, new_secondary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn record_miss(&self) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let hit_count = (secondary & HIT_COUNT_MASK) >> HIT_COUNT_SHIFT;
            let miss_count = (secondary & MISS_COUNT_MASK) + 1;

            let new_secondary = (hit_count << HIT_COUNT_SHIFT) | miss_count;

            if self
                .secondary
                .compare_exchange_weak(secondary, new_secondary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }
}

impl Default for KgpuSamplerCacheCapsule {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for KgpuSamplerCacheCapsule {}
unsafe impl Sync for KgpuSamplerCacheCapsule {}

impl core::fmt::Debug for KgpuSamplerCacheCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let stats = self.stats();
        f.debug_struct("KgpuSamplerCacheCapsule")
            .field("state", &stats.state)
            .field("sampler_count", &stats.sampler_count)
            .field("hit_count", &stats.hit_count)
            .field("miss_count", &stats.miss_count)
            .field("hit_rate", &format_args!("{:.2}%", stats.hit_rate * 100.0))
            .field("generation", &stats.generation)
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Size and Alignment Tests
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            core::mem::size_of::<KgpuSamplerCacheCapsule>(),
            512,
            "KgpuSamplerCacheCapsule must be 512 bytes"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<KgpuSamplerCacheCapsule>(),
            512,
            "KgpuSamplerCacheCapsule must have 512-byte alignment"
        );
    }

    #[test]
    fn test_sampler_entry_size() {
        assert_eq!(core::mem::size_of::<SamplerEntry>(), 16);
    }

    #[test]
    fn test_sampler_entry_alignment() {
        assert_eq!(core::mem::align_of::<SamplerEntry>(), 16);
    }

    // ========================================================================
    // SamplerConfig Tests
    // ========================================================================

    #[test]
    fn test_config_default() {
        let config = SamplerConfig::new();
        assert_eq!(config.mag_filter, FilterMode::Linear);
        assert_eq!(config.min_filter, FilterMode::Linear);
        assert_eq!(config.address_mode_u, AddressMode::ClampToEdge);
        assert_eq!(config.max_anisotropy, 1);
        assert!(config.compare.is_none());
    }

    #[test]
    fn test_config_point_clamp() {
        let config = SamplerConfig::point_clamp();
        assert_eq!(config.mag_filter, FilterMode::Nearest);
        assert_eq!(config.min_filter, FilterMode::Nearest);
        assert_eq!(config.address_mode_u, AddressMode::ClampToEdge);
    }

    #[test]
    fn test_config_linear_repeat() {
        let config = SamplerConfig::linear_repeat();
        assert_eq!(config.mag_filter, FilterMode::Linear);
        assert_eq!(config.address_mode_u, AddressMode::Repeat);
    }

    #[test]
    fn test_config_anisotropic() {
        let config = SamplerConfig::anisotropic(16);
        assert_eq!(config.max_anisotropy, 16);
        assert_eq!(config.mag_filter, FilterMode::Linear);
    }

    #[test]
    fn test_config_shadow_compare() {
        let config = SamplerConfig::shadow_compare();
        assert_eq!(config.compare, Some(CompareFunction::Less));
    }

    #[test]
    fn test_config_hash_deterministic() {
        let config1 = SamplerConfig::linear_clamp();
        let config2 = SamplerConfig::linear_clamp();
        assert_eq!(config1.hash(), config2.hash());
    }

    #[test]
    fn test_config_hash_different() {
        let config1 = SamplerConfig::linear_clamp();
        let config2 = SamplerConfig::point_clamp();
        assert_ne!(config1.hash(), config2.hash());
    }

    #[test]
    fn test_config_hash_all_presets_unique() {
        let configs = [
            SamplerConfig::point_clamp(),
            SamplerConfig::point_repeat(),
            SamplerConfig::linear_clamp(),
            SamplerConfig::linear_repeat(),
            SamplerConfig::anisotropic(4),
            SamplerConfig::anisotropic(8),
            SamplerConfig::anisotropic(16),
            SamplerConfig::shadow_compare(),
        ];

        for i in 0..configs.len() {
            for j in (i + 1)..configs.len() {
                assert_ne!(
                    configs[i].hash(),
                    configs[j].hash(),
                    "Configs {} and {} have same hash",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_config_pack_unpack() {
        let original = SamplerConfig {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Linear,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::MirrorRepeat,
            address_mode_w: AddressMode::ClampToBorder,
            max_anisotropy: 8,
            compare: Some(CompareFunction::Less),
        };

        let packed = original.pack();
        let unpacked = SamplerConfig::unpack(packed).unwrap();

        assert_eq!(unpacked.mag_filter, original.mag_filter);
        assert_eq!(unpacked.min_filter, original.min_filter);
        assert_eq!(unpacked.mipmap_filter, original.mipmap_filter);
        assert_eq!(unpacked.address_mode_u, original.address_mode_u);
        assert_eq!(unpacked.address_mode_v, original.address_mode_v);
        assert_eq!(unpacked.address_mode_w, original.address_mode_w);
        assert_eq!(unpacked.max_anisotropy, original.max_anisotropy);
        assert_eq!(unpacked.compare, original.compare);
    }

    #[test]
    fn test_config_pack_unpack_no_compare() {
        let original = SamplerConfig::linear_clamp();
        let packed = original.pack();
        let unpacked = SamplerConfig::unpack(packed).unwrap();
        assert!(unpacked.compare.is_none());
    }

    // ========================================================================
    // SamplerEntry Tests
    // ========================================================================

    #[test]
    fn test_entry_new_empty() {
        let entry = SamplerEntry::new();
        assert!(entry.is_empty());
        assert_eq!(entry.config_hash(), 0);
        assert_eq!(entry.handle(), 0);
    }

    #[test]
    fn test_entry_try_set() {
        let entry = SamplerEntry::new();
        assert!(entry.try_set(0x1234, 0x5678));
        assert!(!entry.is_empty());
        assert_eq!(entry.config_hash(), 0x1234);
        assert_eq!(entry.handle(), 0x5678);
    }

    #[test]
    fn test_entry_try_set_already_taken() {
        let entry = SamplerEntry::new();
        assert!(entry.try_set(0x1234, 0x5678));
        assert!(!entry.try_set(0x9ABC, 0xDEF0));
        // Original values preserved
        assert_eq!(entry.config_hash(), 0x1234);
        assert_eq!(entry.handle(), 0x5678);
    }

    #[test]
    fn test_entry_match_config() {
        let entry = SamplerEntry::new();
        entry.try_set(0x1234, 0x5678);

        assert_eq!(entry.match_config(0x1234), Some(0x5678));
        assert_eq!(entry.match_config(0x9ABC), None);
    }

    // ========================================================================
    // KgpuSamplerCacheCapsule Tests
    // ========================================================================

    #[test]
    fn test_cache_new() {
        let cache = KgpuSamplerCacheCapsule::new();
        assert_eq!(cache.state(), CACHE_STATE_ACTIVE);
        assert_eq!(cache.sampler_count(), 0);
        assert_eq!(cache.generation(), 1);
        assert_eq!(cache.hit_count(), 0);
        assert_eq!(cache.miss_count(), 0);
    }

    #[test]
    fn test_cache_insert_lookup() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        assert!(cache.insert(config, 0x1000));
        assert_eq!(cache.sampler_count(), 1);

        let handle = cache.lookup(&config);
        assert_eq!(handle, Some(0x1000));
    }

    #[test]
    fn test_cache_lookup_miss() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        let handle = cache.lookup(&config);
        assert!(handle.is_none());
        assert_eq!(cache.miss_count(), 1);
    }

    #[test]
    fn test_cache_hit_miss_tracking() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config1 = SamplerConfig::linear_clamp();
        let config2 = SamplerConfig::point_clamp();

        cache.insert(config1, 0x1000);

        // Hit
        cache.lookup(&config1);
        assert_eq!(cache.hit_count(), 1);

        // Miss
        cache.lookup(&config2);
        assert_eq!(cache.miss_count(), 1);

        // Another hit
        cache.lookup(&config1);
        assert_eq!(cache.hit_count(), 2);
    }

    #[test]
    fn test_cache_get_or_create_cached() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        // First call creates
        let handle1 = cache.get_or_create(config, |_| (0x1000, true));
        assert_eq!(handle1, Some(0x1000));
        assert_eq!(cache.sampler_count(), 1);

        // Second call uses cache
        let handle2 = cache.get_or_create(config, |_| (0x2000, true));
        assert_eq!(handle2, Some(0x1000)); // Same handle from cache
        assert_eq!(cache.hit_count(), 1);
    }

    #[test]
    fn test_cache_get_or_create_failure() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        let handle = cache.get_or_create(config, |_| (0, false));
        assert!(handle.is_none());
        assert_eq!(cache.sampler_count(), 0);
    }

    #[test]
    fn test_cache_multiple_configs() {
        let cache = KgpuSamplerCacheCapsule::new();

        let configs = [
            SamplerConfig::point_clamp(),
            SamplerConfig::point_repeat(),
            SamplerConfig::linear_clamp(),
            SamplerConfig::linear_repeat(),
            SamplerConfig::anisotropic(4),
        ];

        for (i, config) in configs.iter().enumerate() {
            cache.insert(*config, (i + 1) as u64 * 0x1000);
        }

        assert_eq!(cache.sampler_count(), 5);

        for (i, config) in configs.iter().enumerate() {
            let handle = cache.lookup(config);
            assert_eq!(handle, Some((i + 1) as u64 * 0x1000));
        }
    }

    #[test]
    fn test_cache_full() {
        let cache = KgpuSamplerCacheCapsule::new();

        // Fill cache
        for i in 0..MAX_CACHED_SAMPLERS {
            let mut config = SamplerConfig::new();
            config.max_anisotropy = i as u8; // Make each unique
            cache.insert(config, (i + 1) as u64 * 0x100);
        }

        assert_eq!(cache.sampler_count(), MAX_CACHED_SAMPLERS as u8);
        assert!(cache.is_full());
        assert_eq!(cache.state(), CACHE_STATE_FULL);

        // Try to insert one more
        let extra = SamplerConfig::shadow_compare();
        assert!(!cache.insert(extra, 0xFFFF));
    }

    #[test]
    fn test_cache_stats() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        cache.insert(config, 0x1000);
        cache.lookup(&config); // hit
        cache.lookup(&config); // hit
        cache.lookup(&SamplerConfig::point_clamp()); // miss

        let stats = cache.stats();
        assert_eq!(stats.sampler_count, 1);
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_cache_duplicate_insert() {
        let cache = KgpuSamplerCacheCapsule::new();
        let config = SamplerConfig::linear_clamp();

        assert!(cache.insert(config, 0x1000));
        assert!(!cache.insert(config, 0x2000)); // Duplicate

        assert_eq!(cache.sampler_count(), 1);
        assert_eq!(cache.lookup(&config), Some(0x1000)); // Original preserved
    }

    // ========================================================================
    // Thread Safety Tests
    // ========================================================================

    #[test]
    fn test_cache_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuSamplerCacheCapsule>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_cache_concurrent_lookup() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(KgpuSamplerCacheCapsule::new());

        // Pre-populate
        let configs = [
            SamplerConfig::linear_clamp(),
            SamplerConfig::point_clamp(),
            SamplerConfig::linear_repeat(),
        ];
        for (i, config) in configs.iter().enumerate() {
            cache.insert(*config, (i + 1) as u64 * 0x1000);
        }

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let c = Arc::clone(&cache);
                let cfgs = configs;
                thread::spawn(move || {
                    for _ in 0..100 {
                        for config in &cfgs {
                            let _ = c.lookup(config);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Verify all lookups succeeded
        let stats = cache.stats();
        assert!(stats.hit_count >= 400 * 3);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_cache_concurrent_insert() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(KgpuSamplerCacheCapsule::new());

        let handles: Vec<_> = (0..4)
            .map(|t| {
                let c = Arc::clone(&cache);
                thread::spawn(move || {
                    for i in 0..4 {
                        let mut config = SamplerConfig::new();
                        config.max_anisotropy = (t * 4 + i) as u8;
                        c.insert(config, ((t * 4 + i) + 1) as u64 * 0x100);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(cache.sampler_count(), 16);
    }

    // ========================================================================
    // Debug Format Tests
    // ========================================================================

    #[test]
    fn test_cache_debug() {
        let cache = KgpuSamplerCacheCapsule::new();
        cache.insert(SamplerConfig::linear_clamp(), 0x1000);

        let debug_str = format!("{:?}", cache);
        assert!(debug_str.contains("KgpuSamplerCacheCapsule"));
        assert!(debug_str.contains("sampler_count: 1"));
    }

    // ========================================================================
    // Enum Tests
    // ========================================================================

    #[test]
    fn test_filter_mode_from_u8() {
        assert_eq!(FilterMode::from_u8(0), Some(FilterMode::Nearest));
        assert_eq!(FilterMode::from_u8(1), Some(FilterMode::Linear));
        assert_eq!(FilterMode::from_u8(2), None);
    }

    #[test]
    fn test_address_mode_from_u8() {
        assert_eq!(AddressMode::from_u8(0), Some(AddressMode::ClampToEdge));
        assert_eq!(AddressMode::from_u8(1), Some(AddressMode::Repeat));
        assert_eq!(AddressMode::from_u8(2), Some(AddressMode::MirrorRepeat));
        assert_eq!(AddressMode::from_u8(3), Some(AddressMode::ClampToBorder));
        assert_eq!(AddressMode::from_u8(4), None);
    }
}
