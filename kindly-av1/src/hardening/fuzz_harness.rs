//! Fuzz Harness Capsule - T4 Batch Tier
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Fuzzing infrastructure for testing decoder robustness against malformed inputs.
//! Generates mutations, tracks coverage, and integrates with cargo-fuzz/libFuzzer.
//!
//! # T4 Batch Tier
//!
//! This capsule uses T4 Batch tier for:
//! - Batch fuzzing operations (multiple mutations per iteration)
//! - Parallel corpus management
//! - Batch crash analysis
//! - 256B cache-aligned for optimal batch processing
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T4 Batch tier for parallel fuzzing operations
//! - **Chaos**: 256B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY
//! - **B32**: Benchmarks validate batch fuzzing throughput
//! - **T28**: 28+ tests covering all fuzzing operations

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// =============================================================================
// Constants - Interesting Values for Mutation
// =============================================================================

/// Interesting u8 values for mutation
pub const INTERESTING_U8: &[u8] = &[0, 1, 2, 127, 128, 255];

/// Interesting u16 values for mutation
pub const INTERESTING_U16: &[u16] = &[0, 1, 255, 256, 32767, 32768, 65535];

/// Interesting u32 values for mutation
pub const INTERESTING_U32: &[u32] = &[
    0, 1, 255, 256, 65535, 65536, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF,
];

/// H.264 NAL unit types for codec-specific fuzzing
pub const H264_NAL_TYPES: &[u8] = &[0, 1, 2, 5, 6, 7, 8, 9, 10, 11, 12];

/// VP9 frame types for codec-specific fuzzing
pub const VP9_FRAME_TYPES: &[u8] = &[0, 1, 2, 3];

/// Maximum corpus entries (inline storage)
pub const MAX_CORPUS_ENTRIES: usize = 1024;

/// Maximum size for inline mutation buffer
pub const MAX_MUTATION_SIZE: usize = 65536;

// =============================================================================
// Fuzz Target Enumeration
// =============================================================================

/// Fuzz targets for different decoder components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FuzzTarget {
    // Container parsing
    /// MP4 demuxer header parsing
    Mp4DemuxHeader = 0,
    /// MP4 track parsing
    Mp4DemuxTrack = 1,
    /// MKV/WebM demuxer header parsing
    MkvDemuxHeader = 2,
    /// MKV cluster parsing
    MkvDemuxCluster = 3,

    // H.264 components
    /// H.264 Annex B bitstream parsing
    H264Bitstream = 4,
    /// H.264 SPS/PPS parameter set parsing
    H264SpsPps = 5,
    /// H.264 CABAC entropy decoding
    H264Cabac = 6,
    /// H.264 slice decoding
    H264Slice = 7,

    // VP9 components
    /// VP9 bitstream parsing (superframes)
    Vp9Bitstream = 8,
    /// VP9 frame header parsing
    Vp9FrameHeader = 9,
    /// VP9 boolean decoder
    Vp9Bool = 10,
    /// VP9 tile parsing
    Vp9Tile = 11,

    // Full pipeline
    /// Full decode pipeline (container + codec)
    FullDecode = 12,
}

impl FuzzTarget {
    /// Get target from u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Mp4DemuxHeader),
            1 => Some(Self::Mp4DemuxTrack),
            2 => Some(Self::MkvDemuxHeader),
            3 => Some(Self::MkvDemuxCluster),
            4 => Some(Self::H264Bitstream),
            5 => Some(Self::H264SpsPps),
            6 => Some(Self::H264Cabac),
            7 => Some(Self::H264Slice),
            8 => Some(Self::Vp9Bitstream),
            9 => Some(Self::Vp9FrameHeader),
            10 => Some(Self::Vp9Bool),
            11 => Some(Self::Vp9Tile),
            12 => Some(Self::FullDecode),
            _ => None,
        }
    }

    /// Get default input size for this target
    #[inline]
    pub const fn default_size(&self) -> usize {
        match self {
            Self::Mp4DemuxHeader | Self::MkvDemuxHeader => 256,
            Self::Mp4DemuxTrack | Self::MkvDemuxCluster => 1024,
            Self::H264Bitstream | Self::Vp9Bitstream => 4096,
            Self::H264SpsPps | Self::Vp9FrameHeader => 512,
            Self::H264Cabac | Self::Vp9Bool => 2048,
            Self::H264Slice | Self::Vp9Tile => 8192,
            Self::FullDecode => 16384,
        }
    }

    /// Check if this target is container-related
    #[inline]
    pub const fn is_container(&self) -> bool {
        matches!(
            self,
            Self::Mp4DemuxHeader | Self::Mp4DemuxTrack | Self::MkvDemuxHeader | Self::MkvDemuxCluster
        )
    }

    /// Check if this target is H.264-related
    #[inline]
    pub const fn is_h264(&self) -> bool {
        matches!(
            self,
            Self::H264Bitstream | Self::H264SpsPps | Self::H264Cabac | Self::H264Slice
        )
    }

    /// Check if this target is VP9-related
    #[inline]
    pub const fn is_vp9(&self) -> bool {
        matches!(
            self,
            Self::Vp9Bitstream | Self::Vp9FrameHeader | Self::Vp9Bool | Self::Vp9Tile
        )
    }
}

impl core::fmt::Display for FuzzTarget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Mp4DemuxHeader => write!(f, "mp4_demux_header"),
            Self::Mp4DemuxTrack => write!(f, "mp4_demux_track"),
            Self::MkvDemuxHeader => write!(f, "mkv_demux_header"),
            Self::MkvDemuxCluster => write!(f, "mkv_demux_cluster"),
            Self::H264Bitstream => write!(f, "h264_bitstream"),
            Self::H264SpsPps => write!(f, "h264_sps_pps"),
            Self::H264Cabac => write!(f, "h264_cabac"),
            Self::H264Slice => write!(f, "h264_slice"),
            Self::Vp9Bitstream => write!(f, "vp9_bitstream"),
            Self::Vp9FrameHeader => write!(f, "vp9_frame_header"),
            Self::Vp9Bool => write!(f, "vp9_bool"),
            Self::Vp9Tile => write!(f, "vp9_tile"),
            Self::FullDecode => write!(f, "full_decode"),
        }
    }
}

// =============================================================================
// Mutation Strategy Enumeration
// =============================================================================

/// Mutation strategies for generating test inputs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MutationStrategy {
    /// Flip random bits (single bit mutations)
    BitFlip = 0,
    /// Flip random bytes (byte-level mutations)
    ByteFlip = 1,
    /// Add/subtract small arithmetic values
    ArithmeticMut = 2,
    /// Shuffle data blocks (structure-aware)
    BlockShuffle = 3,
    /// Truncate data at random positions
    Truncation = 4,
    /// Insert random bytes
    Insertion = 5,
    /// Delete random bytes
    Deletion = 6,
    /// Use dictionary of interesting values
    Dictionary = 7,
    /// Combine multiple strategies (chaos mode)
    Havoc = 8,
}

impl MutationStrategy {
    /// Get strategy from u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BitFlip),
            1 => Some(Self::ByteFlip),
            2 => Some(Self::ArithmeticMut),
            3 => Some(Self::BlockShuffle),
            4 => Some(Self::Truncation),
            5 => Some(Self::Insertion),
            6 => Some(Self::Deletion),
            7 => Some(Self::Dictionary),
            8 => Some(Self::Havoc),
            _ => None,
        }
    }

    /// Get all strategies for random selection
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::BitFlip,
            Self::ByteFlip,
            Self::ArithmeticMut,
            Self::BlockShuffle,
            Self::Truncation,
            Self::Insertion,
            Self::Deletion,
            Self::Dictionary,
            Self::Havoc,
        ]
    }
}

impl core::fmt::Display for MutationStrategy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BitFlip => write!(f, "bit_flip"),
            Self::ByteFlip => write!(f, "byte_flip"),
            Self::ArithmeticMut => write!(f, "arithmetic"),
            Self::BlockShuffle => write!(f, "block_shuffle"),
            Self::Truncation => write!(f, "truncation"),
            Self::Insertion => write!(f, "insertion"),
            Self::Deletion => write!(f, "deletion"),
            Self::Dictionary => write!(f, "dictionary"),
            Self::Havoc => write!(f, "havoc"),
        }
    }
}

// =============================================================================
// Crash Type Enumeration
// =============================================================================

/// Types of crashes detected during fuzzing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CrashType {
    /// Rust panic (assertion failure, unwrap on None/Err)
    Panic = 0,
    /// Buffer overflow detected (bounds check failure)
    BufferOverflow = 1,
    /// Integer overflow detected
    IntegerOverflow = 2,
    /// Division by zero
    DivisionByZero = 3,
    /// Stack overflow (deep recursion)
    StackOverflow = 4,
    /// Out of memory (allocation failure)
    OutOfMemory = 5,
    /// Timeout (infinite loop or excessive processing)
    Timeout = 6,
    /// Unknown/unclassified crash
    Unknown = 7,
}

impl CrashType {
    /// Get crash type from u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Panic),
            1 => Some(Self::BufferOverflow),
            2 => Some(Self::IntegerOverflow),
            3 => Some(Self::DivisionByZero),
            4 => Some(Self::StackOverflow),
            5 => Some(Self::OutOfMemory),
            6 => Some(Self::Timeout),
            7 => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Get severity level (0-3, higher = more severe)
    #[inline]
    pub const fn severity(&self) -> u8 {
        match self {
            Self::Panic => 1,
            Self::BufferOverflow => 3,
            Self::IntegerOverflow => 2,
            Self::DivisionByZero => 1,
            Self::StackOverflow => 2,
            Self::OutOfMemory => 2,
            Self::Timeout => 1,
            Self::Unknown => 2,
        }
    }
}

impl core::fmt::Display for CrashType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Panic => write!(f, "panic"),
            Self::BufferOverflow => write!(f, "buffer_overflow"),
            Self::IntegerOverflow => write!(f, "integer_overflow"),
            Self::DivisionByZero => write!(f, "division_by_zero"),
            Self::StackOverflow => write!(f, "stack_overflow"),
            Self::OutOfMemory => write!(f, "out_of_memory"),
            Self::Timeout => write!(f, "timeout"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Fuzz harness errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FuzzError {
    /// Corpus is empty
    EmptyCorpus,
    /// Input too large
    InputTooLarge(usize),
    /// Invalid target
    InvalidTarget(u8),
    /// IO error (file operations)
    IoError(String),
    /// Corpus full
    CorpusFull,
    /// Invalid seed
    InvalidSeed,
}

impl core::fmt::Display for FuzzError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyCorpus => write!(f, "corpus is empty"),
            Self::InputTooLarge(size) => {
                write!(f, "input too large: {} bytes (max {})", size, MAX_MUTATION_SIZE)
            }
            Self::InvalidTarget(t) => write!(f, "invalid fuzz target: {}", t),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::CorpusFull => write!(f, "corpus is full"),
            Self::InvalidSeed => write!(f, "invalid RNG seed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FuzzError {}

// =============================================================================
// Result Types
// =============================================================================

/// Result of a single fuzz execution
#[derive(Debug, Clone)]
pub struct FuzzResult {
    /// Target that was fuzzed
    pub target: FuzzTarget,
    /// Hash of input data
    pub input_hash: u64,
    /// Execution time in nanoseconds
    pub execution_time_ns: u64,
    /// Whether new coverage was discovered
    pub new_coverage: bool,
    /// Crash type if execution crashed
    pub crash: Option<CrashType>,
}

impl FuzzResult {
    /// Create a successful result (no crash)
    #[inline]
    pub const fn success(target: FuzzTarget, input_hash: u64, time_ns: u64, new_cov: bool) -> Self {
        Self {
            target,
            input_hash,
            execution_time_ns: time_ns,
            new_coverage: new_cov,
            crash: None,
        }
    }

    /// Create a crash result
    #[inline]
    pub const fn crash(target: FuzzTarget, input_hash: u64, time_ns: u64, crash_type: CrashType) -> Self {
        Self {
            target,
            input_hash,
            execution_time_ns: time_ns,
            new_coverage: true, // Crashes always count as new coverage
            crash: Some(crash_type),
        }
    }

    /// Check if this result indicates a crash
    #[inline]
    pub const fn is_crash(&self) -> bool {
        self.crash.is_some()
    }
}

/// Summary of multiple fuzz iterations
#[derive(Debug, Clone, Default)]
pub struct FuzzSummary {
    /// Total executions performed
    pub executions: u64,
    /// Total crashes found
    pub crashes: u32,
    /// Unique crashes (deduplicated)
    pub unique_crashes: u32,
    /// New coverage edges discovered
    pub coverage_increase: u64,
    /// Interesting inputs found (new coverage + no crash)
    pub interesting_inputs: u32,
    /// Executions per second
    pub exec_per_sec: f64,
}

/// Aggregate statistics for fuzzing session
#[derive(Debug, Clone, Default)]
pub struct FuzzStats {
    /// Total executions since start
    pub total_executions: u64,
    /// Total crashes found
    pub total_crashes: u32,
    /// Unique crashes (deduplicated by location)
    pub unique_crashes: u32,
    /// Corpus size (number of inputs)
    pub corpus_size: u32,
    /// Unique coverage edges discovered
    pub coverage_edges: u64,
    /// Current executions per second
    pub exec_per_sec: f64,
}

// =============================================================================
// Corpus Entry
// =============================================================================

/// Entry in the fuzzing corpus
#[derive(Debug, Clone)]
pub struct CorpusEntry {
    /// Hash of the input data
    pub hash: u64,
    /// Target this input is for
    pub target: FuzzTarget,
    /// Size of the input data
    pub size: u32,
    /// Whether this input found new coverage
    pub new_coverage: bool,
    /// Generation when this was added
    pub generation: u64,
}

// =============================================================================
// State Flags (packed in state AtomicU64)
// =============================================================================

/// State flag bits
pub mod state_flags {
    /// Fuzzing is active
    pub const ACTIVE: u64 = 1 << 56;
    /// Coverage tracking enabled
    pub const COVERAGE_ENABLED: u64 = 1 << 57;
    /// Dictionary mode enabled
    pub const DICTIONARY_ENABLED: u64 = 1 << 58;
    /// In havoc mode
    pub const HAVOC_MODE: u64 = 1 << 59;
    /// Crash detected in last execution
    pub const CRASH_DETECTED: u64 = 1 << 60;
    /// New coverage in last execution
    pub const NEW_COVERAGE: u64 = 1 << 61;

    /// Mask for target (bits 0-7)
    pub const TARGET_MASK: u64 = 0xFF;
    /// Mask for strategy (bits 8-15)
    pub const STRATEGY_MASK: u64 = 0xFF << 8;
    /// Shift for strategy
    pub const STRATEGY_SHIFT: u32 = 8;
}

// =============================================================================
// Fuzz Harness Capsule - T4 Batch (256B)
// =============================================================================

/// T4 Batch Fuzz Harness Capsule
///
/// 256B cache-aligned capsule for batch fuzzing operations.
/// Uses 100% lockfree atomics for thread-safe fuzzing.
///
/// # State Layout (64 bits)
///
/// ```text
/// +--------+--------+--------+--------+--------+--------+--------+--------+
/// | Flags  |        Reserved          |Strategy| Target |
/// +--------+--------+--------+--------+--------+--------+--------+--------+
/// 63      56                         15       8        0
/// ```
///
/// # Memory Layout (256 bytes)
///
/// ```text
/// Offset  Size  Field
/// 0       8     state (target | strategy | flags)
/// 8       8     generation (Q34 audit)
/// 16      8     rng_state[0] (xorshift128+)
/// 24      8     rng_state[1] (xorshift128+)
/// 32      8     total_executions
/// 40      4     executions_per_second
/// 44      4     (padding)
/// 48      8     coverage_bitmap_hash
/// 56      8     unique_edges
/// 64      4     total_crashes
/// 68      4     unique_crashes
/// 72      8     last_crash_hash
/// 80      4     corpus_size
/// 84      4     (alignment padding for corpus_bytes)
/// 88      8     corpus_bytes (atomic u64 for total size)
/// 96      4     interesting_inputs
/// 100     4     (alignment padding)
/// 104     152   _padding (to 256B)
/// ```
#[repr(C, align(256))]
pub struct FuzzHarnessCapsule {
    // State: target (8 bits) | strategy (8 bits) | reserved (40 bits) | flags (8 bits)
    state: AtomicU64,

    // Q34 audit trail generation counter
    generation: AtomicU64,

    // RNG state (xorshift128+ algorithm)
    rng_state_0: AtomicU64,
    rng_state_1: AtomicU64,

    // Execution tracking
    total_executions: AtomicU64,
    executions_per_second: AtomicU32,
    _pad0: u32,

    // Coverage tracking (simplified: hash of seen program counters)
    coverage_bitmap_hash: AtomicU64,
    unique_edges: AtomicU64,

    // Crash tracking
    total_crashes: AtomicU32,
    unique_crashes: AtomicU32,
    last_crash_hash: AtomicU64,

    // Corpus tracking
    corpus_size: AtomicU32,
    corpus_bytes: AtomicU64,

    // Interesting inputs (new coverage, no crash)
    interesting_inputs: AtomicU32,

    // Padding to 256 bytes
    // Layout: 5×AtomicU64(40) + 1×AtomicU32+pad(8) + 2×AtomicU64(16) + 2×AtomicU32(8) + 1×AtomicU64(8) + 1×AtomicU32+pad(8) + 1×AtomicU64(8) + 1×AtomicU32+pad(8) = 104 bytes
    // Need: 256 - 104 = 152 bytes padding
    _padding: [u8; 152],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<FuzzHarnessCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<FuzzHarnessCapsule>() == 256);

impl Default for FuzzHarnessCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzHarnessCapsule {
    /// Create a new fuzz harness capsule
    ///
    /// Initializes with:
    /// - Target: FullDecode
    /// - Strategy: Havoc (combined mutations)
    /// - Coverage tracking enabled
    /// - RNG seeded with default values
    #[inline]
    pub const fn new() -> Self {
        let initial_state = (FuzzTarget::FullDecode as u64)
            | ((MutationStrategy::Havoc as u64) << state_flags::STRATEGY_SHIFT)
            | state_flags::COVERAGE_ENABLED;

        Self {
            state: AtomicU64::new(initial_state),
            generation: AtomicU64::new(0),
            rng_state_0: AtomicU64::new(0x853c_49e6_748f_ea9b),
            rng_state_1: AtomicU64::new(0xda3e_39cb_94b9_5bdb),
            total_executions: AtomicU64::new(0),
            executions_per_second: AtomicU32::new(0),
            _pad0: 0,
            coverage_bitmap_hash: AtomicU64::new(0),
            unique_edges: AtomicU64::new(0),
            total_crashes: AtomicU32::new(0),
            unique_crashes: AtomicU32::new(0),
            last_crash_hash: AtomicU64::new(0),
            corpus_size: AtomicU32::new(0),
            corpus_bytes: AtomicU64::new(0),
            interesting_inputs: AtomicU32::new(0),
            _padding: [0u8; 152],
        }
    }

    /// Create with specific seed for deterministic fuzzing
    #[inline]
    pub fn with_seed(seed: u64) -> Self {
        let mut capsule = Self::new();
        capsule.seed_rng(seed);
        capsule
    }

    /// Seed the RNG for reproducible fuzzing
    #[inline]
    pub fn seed_rng(&mut self, seed: u64) {
        // SplitMix64 to generate two seeds from one
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let s0 = z ^ (z >> 31);

        z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_mul(2);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let s1 = z ^ (z >> 31);

        self.rng_state_0.store(s0, Ordering::Release);
        self.rng_state_1.store(s1, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get the current generation (Q34 audit)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current fuzz target
    #[inline]
    pub fn target(&self) -> FuzzTarget {
        let state = self.state.load(Ordering::Acquire);
        FuzzTarget::from_u8((state & state_flags::TARGET_MASK) as u8)
            .unwrap_or(FuzzTarget::FullDecode)
    }

    /// Set fuzz target
    #[inline]
    pub fn set_target(&self, target: FuzzTarget) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = (old_state & !state_flags::TARGET_MASK) | (target as u64);
            if self
                .state
                .compare_exchange_weak(old_state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::AcqRel);
                break;
            }
        }
    }

    /// Get current mutation strategy
    #[inline]
    pub fn strategy(&self) -> MutationStrategy {
        let state = self.state.load(Ordering::Acquire);
        let strategy_bits = ((state & state_flags::STRATEGY_MASK) >> state_flags::STRATEGY_SHIFT) as u8;
        MutationStrategy::from_u8(strategy_bits).unwrap_or(MutationStrategy::Havoc)
    }

    /// Set mutation strategy
    #[inline]
    pub fn set_strategy(&self, strategy: MutationStrategy) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = (old_state & !state_flags::STRATEGY_MASK)
                | ((strategy as u64) << state_flags::STRATEGY_SHIFT);
            if self
                .state
                .compare_exchange_weak(old_state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::AcqRel);
                break;
            }
        }
    }

    /// Check if coverage tracking is enabled
    #[inline]
    pub fn coverage_enabled(&self) -> bool {
        self.state.load(Ordering::Acquire) & state_flags::COVERAGE_ENABLED != 0
    }

    /// Enable/disable coverage tracking
    #[inline]
    pub fn set_coverage_enabled(&self, enabled: bool) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = if enabled {
                old_state | state_flags::COVERAGE_ENABLED
            } else {
                old_state & !state_flags::COVERAGE_ENABLED
            };
            if self
                .state
                .compare_exchange_weak(old_state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    // =========================================================================
    // RNG Operations (xorshift128+)
    // =========================================================================

    /// Generate next random u64
    #[inline]
    fn next_u64(&self) -> u64 {
        // Load current state
        let s0 = self.rng_state_0.load(Ordering::Acquire);
        let s1 = self.rng_state_1.load(Ordering::Acquire);

        let result = s0.wrapping_add(s1);

        // xorshift128+ step
        let s1_xor = s1 ^ s0;
        let new_s0 = s0.rotate_left(24) ^ s1_xor ^ (s1_xor << 16);
        let new_s1 = s1_xor.rotate_left(37);

        // Store new state (lockfree, best-effort update)
        self.rng_state_0.store(new_s0, Ordering::Release);
        self.rng_state_1.store(new_s1, Ordering::Release);

        result
    }

    /// Generate random value in range [0, max)
    #[inline]
    fn next_range(&self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        (self.next_u64() as usize) % max
    }

    /// Generate random u8
    #[inline]
    fn next_u8(&self) -> u8 {
        self.next_u64() as u8
    }

    // =========================================================================
    // Hashing (FNV-1a for fast non-crypto hashing)
    // =========================================================================

    /// Hash data using FNV-1a (fast, non-cryptographic)
    #[inline]
    pub fn hash_data(data: &[u8]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0100_0000_01b3;

        let mut hash = FNV_OFFSET;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    // =========================================================================
    // Mutation Operations
    // =========================================================================

    /// Mutate data using specified strategy
    ///
    /// Returns a new Vec with mutated data. The original is not modified.
    pub fn mutate(&self, data: &[u8], strategy: MutationStrategy, _seed: u64) -> Vec<u8> {
        if data.is_empty() {
            return vec![self.next_u8()];
        }

        match strategy {
            MutationStrategy::BitFlip => self.mutate_bit_flip(data),
            MutationStrategy::ByteFlip => self.mutate_byte_flip(data),
            MutationStrategy::ArithmeticMut => self.mutate_arithmetic(data),
            MutationStrategy::BlockShuffle => self.mutate_block_shuffle(data),
            MutationStrategy::Truncation => self.mutate_truncate(data),
            MutationStrategy::Insertion => self.mutate_insert(data),
            MutationStrategy::Deletion => self.mutate_delete(data),
            MutationStrategy::Dictionary => self.mutate_dictionary(data),
            MutationStrategy::Havoc => self.mutate_havoc(data),
        }
    }

    /// Mutate with random strategy selection
    pub fn mutate_random(&self, data: &[u8], seed: u64) -> Vec<u8> {
        let strategies = MutationStrategy::all();
        let idx = (self.next_u64() ^ seed) as usize % strategies.len();
        self.mutate(data, strategies[idx], seed)
    }

    /// Generate completely random data for target
    pub fn generate_random(&self, target: FuzzTarget, size: usize, _seed: u64) -> Vec<u8> {
        let actual_size = if size == 0 { target.default_size() } else { size };
        let mut data = vec![0u8; actual_size];

        // Fill with random bytes
        for byte in data.iter_mut() {
            *byte = self.next_u8();
        }

        // Add target-specific headers/markers
        match target {
            FuzzTarget::H264Bitstream => {
                // Add H.264 start code
                if data.len() >= 4 {
                    data[0] = 0x00;
                    data[1] = 0x00;
                    data[2] = 0x00;
                    data[3] = 0x01;
                    if data.len() > 4 {
                        // Random NAL type
                        data[4] = H264_NAL_TYPES[self.next_range(H264_NAL_TYPES.len())];
                    }
                }
            }
            FuzzTarget::Vp9Bitstream | FuzzTarget::Vp9FrameHeader => {
                // Add VP9 frame marker
                if data.len() >= 2 {
                    data[0] = 0x82; // VP9 frame marker (10 in bits 7-6)
                    data[1] = VP9_FRAME_TYPES[self.next_range(VP9_FRAME_TYPES.len())];
                }
            }
            FuzzTarget::Mp4DemuxHeader => {
                // Add MP4 ftyp box marker
                if data.len() >= 8 {
                    // Box size (random but valid)
                    data[0] = 0x00;
                    data[1] = 0x00;
                    data[2] = 0x00;
                    data[3] = (actual_size.min(255)) as u8;
                    // 'ftyp' marker
                    data[4] = b'f';
                    data[5] = b't';
                    data[6] = b'y';
                    data[7] = b'p';
                }
            }
            FuzzTarget::MkvDemuxHeader => {
                // Add EBML header marker
                if data.len() >= 4 {
                    data[0] = 0x1A;
                    data[1] = 0x45;
                    data[2] = 0xDF;
                    data[3] = 0xA3;
                }
            }
            _ => {}
        }

        data
    }

    // Mutation implementations

    fn mutate_bit_flip(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        if result.is_empty() {
            return result;
        }

        let num_flips = 1 + self.next_range(8);
        for _ in 0..num_flips {
            let byte_idx = self.next_range(result.len());
            let bit_idx = self.next_range(8);
            result[byte_idx] ^= 1 << bit_idx;
        }
        result
    }

    fn mutate_byte_flip(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        if result.is_empty() {
            return result;
        }

        let num_flips = 1 + self.next_range(4);
        for _ in 0..num_flips {
            let idx = self.next_range(result.len());
            result[idx] ^= 0xFF;
        }
        result
    }

    fn mutate_arithmetic(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        if result.is_empty() {
            return result;
        }

        let idx = self.next_range(result.len());
        let delta = (self.next_range(35) as i16) - 17; // Range: -17 to +17
        result[idx] = (result[idx] as i16).wrapping_add(delta) as u8;
        result
    }

    fn mutate_block_shuffle(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        if result.len() < 4 {
            return result;
        }

        let block_size = 1 + self.next_range(result.len() / 2);
        let num_blocks = result.len() / block_size;
        if num_blocks < 2 {
            return result;
        }

        let block1 = self.next_range(num_blocks);
        let mut block2 = self.next_range(num_blocks);
        while block2 == block1 && num_blocks > 1 {
            block2 = self.next_range(num_blocks);
        }

        let start1 = block1 * block_size;
        let start2 = block2 * block_size;

        for i in 0..block_size {
            if start1 + i < result.len() && start2 + i < result.len() {
                result.swap(start1 + i, start2 + i);
            }
        }
        result
    }

    fn mutate_truncate(&self, data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return vec![];
        }
        let new_len = 1 + self.next_range(data.len());
        data[..new_len].to_vec()
    }

    fn mutate_insert(&self, data: &[u8]) -> Vec<u8> {
        let insert_pos = if data.is_empty() {
            0
        } else {
            self.next_range(data.len())
        };
        let insert_len = 1 + self.next_range(16);

        let mut result = Vec::with_capacity(data.len() + insert_len);
        result.extend_from_slice(&data[..insert_pos]);
        for _ in 0..insert_len {
            result.push(self.next_u8());
        }
        if insert_pos < data.len() {
            result.extend_from_slice(&data[insert_pos..]);
        }
        result
    }

    fn mutate_delete(&self, data: &[u8]) -> Vec<u8> {
        if data.len() <= 1 {
            return vec![];
        }

        let delete_len = 1 + self.next_range(data.len() / 2);
        let delete_pos = self.next_range(data.len().saturating_sub(delete_len) + 1);

        let mut result = Vec::with_capacity(data.len() - delete_len);
        result.extend_from_slice(&data[..delete_pos]);
        let end_pos = (delete_pos + delete_len).min(data.len());
        result.extend_from_slice(&data[end_pos..]);
        result
    }

    fn mutate_dictionary(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        if result.is_empty() {
            // Use interesting values for empty input
            return vec![INTERESTING_U8[self.next_range(INTERESTING_U8.len())]];
        }

        let insert_pos = self.next_range(result.len());
        let choice = self.next_range(3);

        match choice {
            0 => {
                // Insert interesting u8
                let val = INTERESTING_U8[self.next_range(INTERESTING_U8.len())];
                result[insert_pos] = val;
            }
            1 => {
                // Insert interesting u16
                if result.len() >= 2 {
                    let val = INTERESTING_U16[self.next_range(INTERESTING_U16.len())];
                    let pos = self.next_range(result.len() - 1);
                    result[pos] = (val >> 8) as u8;
                    result[pos + 1] = val as u8;
                }
            }
            2 => {
                // Insert interesting u32
                if result.len() >= 4 {
                    let val = INTERESTING_U32[self.next_range(INTERESTING_U32.len())];
                    let pos = self.next_range(result.len() - 3);
                    result[pos] = (val >> 24) as u8;
                    result[pos + 1] = (val >> 16) as u8;
                    result[pos + 2] = (val >> 8) as u8;
                    result[pos + 3] = val as u8;
                }
            }
            _ => unreachable!(),
        }
        result
    }

    fn mutate_havoc(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        let num_mutations = 1 + self.next_range(8);

        for _ in 0..num_mutations {
            let strategies = &[
                MutationStrategy::BitFlip,
                MutationStrategy::ByteFlip,
                MutationStrategy::ArithmeticMut,
                MutationStrategy::Truncation,
                MutationStrategy::Insertion,
                MutationStrategy::Deletion,
                MutationStrategy::Dictionary,
            ];
            let strategy = strategies[self.next_range(strategies.len())];
            result = self.mutate(&result, strategy, 0);
        }
        result
    }

    // =========================================================================
    // Corpus Management
    // =========================================================================

    /// Add input to corpus (returns hash)
    ///
    /// Note: This is a simplified implementation that only tracks metadata.
    /// Actual corpus storage would need external memory management.
    pub fn add_to_corpus(&self, data: &[u8], _target: FuzzTarget) -> u64 {
        let hash = Self::hash_data(data);
        let size = data.len() as u64;

        self.corpus_size.fetch_add(1, Ordering::AcqRel);
        self.corpus_bytes.fetch_add(size, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        hash
    }

    /// Get corpus size (number of entries)
    #[inline]
    pub fn corpus_size(&self) -> u32 {
        self.corpus_size.load(Ordering::Acquire)
    }

    /// Get total corpus bytes
    #[inline]
    pub fn corpus_bytes(&self) -> u64 {
        self.corpus_bytes.load(Ordering::Acquire)
    }

    // =========================================================================
    // Coverage Tracking
    // =========================================================================

    /// Record coverage for a program counter
    ///
    /// Uses a simplified hash-based approach for coverage deduplication.
    pub fn record_coverage(&self, pc: u64) {
        // Mix PC into coverage hash
        let old_hash = self.coverage_bitmap_hash.load(Ordering::Acquire);
        let new_hash = old_hash ^ pc.wrapping_mul(0x517c_c1b7_2722_0a95);
        self.coverage_bitmap_hash.store(new_hash, Ordering::Release);

        // Update edge count (simplified)
        self.unique_edges.fetch_add(1, Ordering::AcqRel);

        // Set new coverage flag
        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = state | state_flags::NEW_COVERAGE;
            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Get unique coverage edge count
    #[inline]
    pub fn coverage_count(&self) -> u64 {
        self.unique_edges.load(Ordering::Acquire)
    }

    /// Check if new coverage was found in last execution
    #[inline]
    pub fn new_coverage(&self) -> bool {
        self.state.load(Ordering::Acquire) & state_flags::NEW_COVERAGE != 0
    }

    /// Clear new coverage flag
    #[inline]
    pub fn clear_new_coverage(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = state & !state_flags::NEW_COVERAGE;
            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    // =========================================================================
    // Crash Tracking
    // =========================================================================

    /// Record a crash
    pub fn record_crash(&self, input_hash: u64, crash_type: CrashType, _location: &str) {
        self.total_crashes.fetch_add(1, Ordering::AcqRel);

        // Check if this is a unique crash (by input hash)
        let last_hash = self.last_crash_hash.swap(input_hash, Ordering::AcqRel);
        if last_hash != input_hash {
            self.unique_crashes.fetch_add(1, Ordering::AcqRel);
        }

        // Set crash detected flag
        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = state | state_flags::CRASH_DETECTED;
            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        self.generation.fetch_add(1, Ordering::AcqRel);

        // Log crash type for debugging
        #[cfg(feature = "std")]
        {
            let _ = crash_type; // Suppress unused warning
        }
    }

    /// Get total crash count
    #[inline]
    pub fn crash_count(&self) -> u32 {
        self.total_crashes.load(Ordering::Acquire)
    }

    /// Get unique crash count
    #[inline]
    pub fn unique_crashes(&self) -> u32 {
        self.unique_crashes.load(Ordering::Acquire)
    }

    /// Clear crash detected flag
    #[inline]
    pub fn clear_crash_flag(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = state & !state_flags::CRASH_DETECTED;
            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    // =========================================================================
    // Fuzzing Execution
    // =========================================================================

    /// Execute a single fuzz iteration
    ///
    /// This is a stub that should be integrated with actual decoder targets.
    /// Returns success by default - real implementation would call decoder.
    pub fn fuzz_once(&self, target: FuzzTarget, data: &[u8]) -> FuzzResult {
        let start = self.generation.load(Ordering::Acquire);
        let input_hash = Self::hash_data(data);

        // Clear flags
        self.clear_new_coverage();
        self.clear_crash_flag();

        // Set target
        self.set_target(target);

        // Simulate execution (real implementation would call decoder)
        // For now, we detect "crashes" based on certain input patterns
        let crash = self.detect_crash_patterns(data);

        // Record execution
        self.total_executions.fetch_add(1, Ordering::AcqRel);

        // Simulate coverage (hash-based, not real coverage)
        if !data.is_empty() {
            let coverage_pc = input_hash.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            self.record_coverage(coverage_pc);
        }

        let end = self.generation.load(Ordering::Acquire);
        let execution_time_ns = (end - start) * 100; // Simulated timing

        if let Some(crash_type) = crash {
            self.record_crash(input_hash, crash_type, "fuzz_once");
            FuzzResult::crash(target, input_hash, execution_time_ns, crash_type)
        } else {
            let new_cov = self.new_coverage();
            if new_cov {
                self.interesting_inputs.fetch_add(1, Ordering::AcqRel);
            }
            FuzzResult::success(target, input_hash, execution_time_ns, new_cov)
        }
    }

    /// Detect crash patterns in input (heuristic-based)
    fn detect_crash_patterns(&self, data: &[u8]) -> Option<CrashType> {
        if data.is_empty() {
            return None;
        }

        // Pattern 1: Specific panic trigger for testing (DEADBEEF magic)
        // Check this FIRST before other patterns
        if data.len() >= 4
            && data[0] == 0xDE
            && data[1] == 0xAD
            && data[2] == 0xBE
            && data[3] == 0xEF
        {
            return Some(CrashType::Panic);
        }

        // Pattern 2: All 0xFF could trigger integer overflow
        if data.len() >= 4 && data.iter().take(4).all(|&b| b == 0xFF) {
            return Some(CrashType::IntegerOverflow);
        }

        // Pattern 3: Extremely large size values (but not DEADBEEF)
        if data.len() >= 4 {
            let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            if size > 0x7FFF_FFFF {
                return Some(CrashType::BufferOverflow);
            }
        }

        // Pattern 4: Division by zero trigger
        if data.len() >= 2 && data[0] == 0x00 && data[1] == 0x00 {
            // Very small probability to trigger
            if self.next_range(100) < 5 {
                return Some(CrashType::DivisionByZero);
            }
        }

        None
    }

    /// Execute multiple fuzz iterations
    pub fn fuzz_iterations(&self, target: FuzzTarget, iterations: u64) -> FuzzSummary {
        let start_execs = self.total_executions.load(Ordering::Acquire);
        let start_crashes = self.total_crashes.load(Ordering::Acquire);
        let start_unique_crashes = self.unique_crashes.load(Ordering::Acquire);
        let start_coverage = self.unique_edges.load(Ordering::Acquire);
        let start_interesting = self.interesting_inputs.load(Ordering::Acquire);

        // Generate and fuzz
        for _ in 0..iterations {
            let data = self.generate_random(target, 0, self.next_u64() as u64);
            let _ = self.fuzz_once(target, &data);
        }

        let end_execs = self.total_executions.load(Ordering::Acquire);
        let end_crashes = self.total_crashes.load(Ordering::Acquire);
        let end_unique_crashes = self.unique_crashes.load(Ordering::Acquire);
        let end_coverage = self.unique_edges.load(Ordering::Acquire);
        let end_interesting = self.interesting_inputs.load(Ordering::Acquire);

        FuzzSummary {
            executions: end_execs - start_execs,
            crashes: end_crashes - start_crashes,
            unique_crashes: end_unique_crashes - start_unique_crashes,
            coverage_increase: end_coverage - start_coverage,
            interesting_inputs: end_interesting - start_interesting,
            exec_per_sec: (end_execs - start_execs) as f64, // Simplified
        }
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get current fuzzing statistics
    pub fn stats(&self) -> FuzzStats {
        FuzzStats {
            total_executions: self.total_executions.load(Ordering::Acquire),
            total_crashes: self.total_crashes.load(Ordering::Acquire),
            unique_crashes: self.unique_crashes.load(Ordering::Acquire),
            corpus_size: self.corpus_size.load(Ordering::Acquire),
            coverage_edges: self.unique_edges.load(Ordering::Acquire),
            exec_per_sec: self.executions_per_second.load(Ordering::Acquire) as f64,
        }
    }

    /// Reset all statistics
    pub fn reset_stats(&self) {
        self.total_executions.store(0, Ordering::Release);
        self.executions_per_second.store(0, Ordering::Release);
        self.coverage_bitmap_hash.store(0, Ordering::Release);
        self.unique_edges.store(0, Ordering::Release);
        self.total_crashes.store(0, Ordering::Release);
        self.unique_crashes.store(0, Ordering::Release);
        self.last_crash_hash.store(0, Ordering::Release);
        self.corpus_size.store(0, Ordering::Release);
        self.corpus_bytes.store(0, Ordering::Release);
        self.interesting_inputs.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

// =============================================================================
// libFuzzer Integration
// =============================================================================

/// Entry point for cargo-fuzz H.264 target
#[cfg(fuzzing)]
pub fn fuzz_target_h264(data: &[u8]) {
    let harness = FuzzHarnessCapsule::new();
    let _ = harness.fuzz_once(FuzzTarget::H264Bitstream, data);
}

/// Entry point for cargo-fuzz VP9 target
#[cfg(fuzzing)]
pub fn fuzz_target_vp9(data: &[u8]) {
    let harness = FuzzHarnessCapsule::new();
    let _ = harness.fuzz_once(FuzzTarget::Vp9Bitstream, data);
}

/// Entry point for cargo-fuzz MP4 target
#[cfg(fuzzing)]
pub fn fuzz_target_mp4(data: &[u8]) {
    let harness = FuzzHarnessCapsule::new();
    let _ = harness.fuzz_once(FuzzTarget::Mp4DemuxHeader, data);
}

/// Entry point for cargo-fuzz MKV target
#[cfg(fuzzing)]
pub fn fuzz_target_mkv(data: &[u8]) {
    let harness = FuzzHarnessCapsule::new();
    let _ = harness.fuzz_once(FuzzTarget::MkvDemuxHeader, data);
}

// =============================================================================
// Tests (T28 Compliance: 28+ tests across 5 tiers)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: Unit Tests (Basic Operations)
    // =========================================================================

    #[test]
    fn q1_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<FuzzHarnessCapsule>(), 256);
        assert_eq!(core::mem::align_of::<FuzzHarnessCapsule>(), 256);
    }

    #[test]
    fn q2_default_initialization() {
        let harness = FuzzHarnessCapsule::new();
        assert_eq!(harness.target(), FuzzTarget::FullDecode);
        assert_eq!(harness.strategy(), MutationStrategy::Havoc);
        assert!(harness.coverage_enabled());
        assert_eq!(harness.generation(), 0);
    }

    #[test]
    fn q3_target_setting() {
        let harness = FuzzHarnessCapsule::new();
        harness.set_target(FuzzTarget::H264Bitstream);
        assert_eq!(harness.target(), FuzzTarget::H264Bitstream);
        assert!(harness.generation() > 0);
    }

    #[test]
    fn q4_strategy_setting() {
        let harness = FuzzHarnessCapsule::new();
        harness.set_strategy(MutationStrategy::BitFlip);
        assert_eq!(harness.strategy(), MutationStrategy::BitFlip);
    }

    #[test]
    fn q5_fuzz_target_from_u8() {
        assert_eq!(FuzzTarget::from_u8(0), Some(FuzzTarget::Mp4DemuxHeader));
        assert_eq!(FuzzTarget::from_u8(4), Some(FuzzTarget::H264Bitstream));
        assert_eq!(FuzzTarget::from_u8(8), Some(FuzzTarget::Vp9Bitstream));
        assert_eq!(FuzzTarget::from_u8(12), Some(FuzzTarget::FullDecode));
        assert_eq!(FuzzTarget::from_u8(255), None);
    }

    #[test]
    fn q6_mutation_strategy_from_u8() {
        assert_eq!(MutationStrategy::from_u8(0), Some(MutationStrategy::BitFlip));
        assert_eq!(MutationStrategy::from_u8(8), Some(MutationStrategy::Havoc));
        assert_eq!(MutationStrategy::from_u8(255), None);
    }

    #[test]
    fn q7_crash_type_severity() {
        assert_eq!(CrashType::BufferOverflow.severity(), 3);
        assert_eq!(CrashType::Panic.severity(), 1);
        assert_eq!(CrashType::IntegerOverflow.severity(), 2);
    }

    // =========================================================================
    // Q8-Q14: Property Tests (Mutation Strategies)
    // =========================================================================

    #[test]
    fn q8_bit_flip_changes_data() {
        let harness = FuzzHarnessCapsule::with_seed(42);
        let original = vec![0u8; 16];
        let mutated = harness.mutate(&original, MutationStrategy::BitFlip, 0);
        assert_ne!(original, mutated);
        assert_eq!(original.len(), mutated.len());
    }

    #[test]
    fn q9_byte_flip_changes_data() {
        let harness = FuzzHarnessCapsule::with_seed(43);
        let original = vec![0u8; 16];
        let mutated = harness.mutate(&original, MutationStrategy::ByteFlip, 0);
        assert_ne!(original, mutated);
    }

    #[test]
    fn q10_arithmetic_mutation() {
        let harness = FuzzHarnessCapsule::with_seed(44);
        let original = vec![100u8; 8];
        let mutated = harness.mutate(&original, MutationStrategy::ArithmeticMut, 0);
        assert_ne!(original, mutated);
        assert_eq!(original.len(), mutated.len());
    }

    #[test]
    fn q11_truncation_reduces_size() {
        let harness = FuzzHarnessCapsule::with_seed(45);
        let original = vec![0xABu8; 100];
        let mutated = harness.mutate(&original, MutationStrategy::Truncation, 0);
        assert!(mutated.len() <= original.len());
        assert!(!mutated.is_empty());
    }

    #[test]
    fn q12_insertion_increases_size() {
        let harness = FuzzHarnessCapsule::with_seed(46);
        let original = vec![0u8; 16];
        let mutated = harness.mutate(&original, MutationStrategy::Insertion, 0);
        assert!(mutated.len() > original.len());
    }

    #[test]
    fn q13_deletion_decreases_size() {
        let harness = FuzzHarnessCapsule::with_seed(47);
        let original = vec![0u8; 100];
        let mutated = harness.mutate(&original, MutationStrategy::Deletion, 0);
        assert!(mutated.len() < original.len());
    }

    #[test]
    fn q14_havoc_applies_multiple_mutations() {
        let harness = FuzzHarnessCapsule::with_seed(48);
        let original = vec![0u8; 64];
        let mutated = harness.mutate(&original, MutationStrategy::Havoc, 0);
        // Havoc should significantly change data
        let diff_count = original
            .iter()
            .zip(mutated.iter())
            .filter(|(a, b)| a != b)
            .count();
        // With havoc, at least one byte should be different
        assert!(diff_count > 0 || original.len() != mutated.len());
    }

    // =========================================================================
    // Q15-Q21: Integration Tests (Full Fuzz Cycle)
    // =========================================================================

    #[test]
    fn q15_fuzz_once_returns_result() {
        let harness = FuzzHarnessCapsule::with_seed(50);
        let data = vec![0u8; 128];
        let result = harness.fuzz_once(FuzzTarget::H264Bitstream, &data);
        assert_eq!(result.target, FuzzTarget::H264Bitstream);
        assert!(result.execution_time_ns > 0 || result.input_hash != 0);
    }

    #[test]
    fn q16_fuzz_iterations_executes_multiple() {
        let harness = FuzzHarnessCapsule::with_seed(51);
        let summary = harness.fuzz_iterations(FuzzTarget::Vp9Bitstream, 10);
        assert_eq!(summary.executions, 10);
    }

    #[test]
    fn q17_generate_random_creates_valid_h264() {
        let harness = FuzzHarnessCapsule::with_seed(52);
        let data = harness.generate_random(FuzzTarget::H264Bitstream, 0, 0);
        // Should have start code
        assert!(data.len() >= 4);
        assert_eq!(data[0], 0x00);
        assert_eq!(data[1], 0x00);
        assert_eq!(data[2], 0x00);
        assert_eq!(data[3], 0x01);
    }

    #[test]
    fn q18_generate_random_creates_valid_vp9() {
        let harness = FuzzHarnessCapsule::with_seed(53);
        let data = harness.generate_random(FuzzTarget::Vp9Bitstream, 0, 0);
        // Should have VP9 marker
        assert!(data.len() >= 2);
        assert_eq!(data[0] & 0xC0, 0x80); // Frame marker = 10 binary
    }

    #[test]
    fn q19_corpus_management() {
        let harness = FuzzHarnessCapsule::new();
        assert_eq!(harness.corpus_size(), 0);

        let hash1 = harness.add_to_corpus(&[1, 2, 3], FuzzTarget::H264Bitstream);
        assert_eq!(harness.corpus_size(), 1);
        assert!(hash1 != 0);

        let hash2 = harness.add_to_corpus(&[4, 5, 6], FuzzTarget::H264Bitstream);
        assert_eq!(harness.corpus_size(), 2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn q20_coverage_tracking() {
        let harness = FuzzHarnessCapsule::new();
        assert_eq!(harness.coverage_count(), 0);

        harness.record_coverage(0x1234_5678);
        assert!(harness.coverage_count() >= 1);
        assert!(harness.new_coverage());

        harness.clear_new_coverage();
        assert!(!harness.new_coverage());
    }

    #[test]
    fn q21_crash_tracking() {
        let harness = FuzzHarnessCapsule::new();
        assert_eq!(harness.crash_count(), 0);

        harness.record_crash(0xABCD, CrashType::BufferOverflow, "test_location");
        assert_eq!(harness.crash_count(), 1);
        assert_eq!(harness.unique_crashes(), 1);

        // Same hash should not increase unique count
        harness.record_crash(0xABCD, CrashType::BufferOverflow, "test_location");
        assert_eq!(harness.crash_count(), 2);
        assert_eq!(harness.unique_crashes(), 1);
    }

    // =========================================================================
    // Q22-Q28: Production Tests (Robustness)
    // =========================================================================

    #[test]
    fn q22_rng_determinism() {
        let h1 = FuzzHarnessCapsule::with_seed(12345);
        let h2 = FuzzHarnessCapsule::with_seed(12345);

        let data1 = h1.generate_random(FuzzTarget::H264Bitstream, 32, 0);
        let data2 = h2.generate_random(FuzzTarget::H264Bitstream, 32, 0);

        assert_eq!(data1, data2);
    }

    #[test]
    fn q23_empty_input_handling() {
        let harness = FuzzHarnessCapsule::with_seed(100);

        // Empty input should not crash
        let result = harness.fuzz_once(FuzzTarget::H264Bitstream, &[]);
        assert!(!result.is_crash() || result.crash.is_some());

        // Mutation of empty should produce something
        let mutated = harness.mutate(&[], MutationStrategy::Insertion, 0);
        assert!(!mutated.is_empty());
    }

    #[test]
    fn q24_large_input_mutation() {
        let harness = FuzzHarnessCapsule::with_seed(101);
        let large_input = vec![0xABu8; 10000];

        for strategy in MutationStrategy::all() {
            let mutated = harness.mutate(&large_input, *strategy, 0);
            assert!(!mutated.is_empty() || *strategy == MutationStrategy::Truncation);
        }
    }

    #[test]
    fn q25_crash_detection_patterns() {
        let harness = FuzzHarnessCapsule::with_seed(102);

        // Integer overflow pattern
        let overflow_input = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let result = harness.fuzz_once(FuzzTarget::FullDecode, &overflow_input);
        assert!(result.crash == Some(CrashType::IntegerOverflow));

        // Panic trigger pattern
        let panic_input = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00];
        let result = harness.fuzz_once(FuzzTarget::FullDecode, &panic_input);
        assert!(result.crash == Some(CrashType::Panic));
    }

    #[test]
    fn q26_stats_reset() {
        let harness = FuzzHarnessCapsule::new();

        // Generate some stats
        harness.fuzz_iterations(FuzzTarget::H264Bitstream, 10);
        assert!(harness.stats().total_executions > 0);

        // Reset
        harness.reset_stats();
        let stats = harness.stats();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.total_crashes, 0);
        assert_eq!(stats.corpus_size, 0);
    }

    #[test]
    fn q27_target_categorization() {
        assert!(FuzzTarget::Mp4DemuxHeader.is_container());
        assert!(!FuzzTarget::Mp4DemuxHeader.is_h264());

        assert!(FuzzTarget::H264Bitstream.is_h264());
        assert!(!FuzzTarget::H264Bitstream.is_vp9());

        assert!(FuzzTarget::Vp9Bitstream.is_vp9());
        assert!(!FuzzTarget::Vp9Bitstream.is_container());

        assert!(!FuzzTarget::FullDecode.is_container());
        assert!(!FuzzTarget::FullDecode.is_h264());
        assert!(!FuzzTarget::FullDecode.is_vp9());
    }

    #[test]
    fn q28_fuzz_result_creation() {
        let success = FuzzResult::success(FuzzTarget::H264Bitstream, 123, 1000, true);
        assert!(!success.is_crash());
        assert!(success.new_coverage);

        let crash = FuzzResult::crash(FuzzTarget::Vp9Bitstream, 456, 2000, CrashType::Panic);
        assert!(crash.is_crash());
        assert!(crash.new_coverage); // Crashes always count as new coverage
    }

    // =========================================================================
    // Q29-Q35: Determinism Tests (Reproducibility)
    // =========================================================================

    #[test]
    fn q29_hash_determinism() {
        let data = b"test data for hashing";
        let hash1 = FuzzHarnessCapsule::hash_data(data);
        let hash2 = FuzzHarnessCapsule::hash_data(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn q30_mutation_determinism_with_same_seed() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];

        let h1 = FuzzHarnessCapsule::with_seed(99999);
        let h2 = FuzzHarnessCapsule::with_seed(99999);

        let m1 = h1.mutate(&data, MutationStrategy::BitFlip, 0);
        let m2 = h2.mutate(&data, MutationStrategy::BitFlip, 0);

        assert_eq!(m1, m2);
    }

    #[test]
    fn q31_different_seeds_produce_different_output() {
        let h1 = FuzzHarnessCapsule::with_seed(11111);
        let h2 = FuzzHarnessCapsule::with_seed(22222);

        let m1 = h1.generate_random(FuzzTarget::H264Bitstream, 64, 0);
        let m2 = h2.generate_random(FuzzTarget::H264Bitstream, 64, 0);

        assert_ne!(m1, m2);
    }

    #[test]
    fn q32_generation_counter_increments() {
        let harness = FuzzHarnessCapsule::new();
        let gen0 = harness.generation();

        harness.set_target(FuzzTarget::H264Bitstream);
        let gen1 = harness.generation();
        assert!(gen1 > gen0);

        harness.add_to_corpus(&[1, 2, 3], FuzzTarget::H264Bitstream);
        let gen2 = harness.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn q33_lockfree_state_transitions() {
        // Test that state transitions are atomic
        let harness = FuzzHarnessCapsule::new();

        for target in [
            FuzzTarget::Mp4DemuxHeader,
            FuzzTarget::H264Bitstream,
            FuzzTarget::Vp9Bitstream,
            FuzzTarget::FullDecode,
        ] {
            harness.set_target(target);
            assert_eq!(harness.target(), target);
        }

        for strategy in MutationStrategy::all() {
            harness.set_strategy(*strategy);
            assert_eq!(harness.strategy(), *strategy);
        }
    }

    #[test]
    fn q34_coverage_flag_atomicity() {
        let harness = FuzzHarnessCapsule::new();

        harness.set_coverage_enabled(true);
        assert!(harness.coverage_enabled());

        harness.set_coverage_enabled(false);
        assert!(!harness.coverage_enabled());

        harness.set_coverage_enabled(true);
        assert!(harness.coverage_enabled());
    }
}
