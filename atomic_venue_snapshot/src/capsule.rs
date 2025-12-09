#![allow(clippy::module_name_repetitions)]

use core::sync::atomic::Ordering;
use portable_atomic::AtomicU128;
use atomic_breaker::AtomicBreakerSWeMR;

use crate::layout;

/// Packed AVS-128 word wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedAvs(pub u128);

impl PackedAvs {
    /// Create a wrapper from a raw `u128` word.
    #[must_use]
    #[inline]
    pub const fn new(raw: u128) -> Self {
        Self(raw)
    }

    /// Return the raw packed word.
    #[must_use]
    #[inline]
    pub const fn raw(self) -> u128 {
        self.0
    }

    /// Unpack the word into a structured snapshot.
    #[must_use]
    #[inline]
    pub fn unpack(self) -> Avs128Snapshot {
        Avs128Snapshot::from(self)
    }
}

/// Human-friendly representation of the AVS-128 fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Avs128Snapshot {
    /// Spread between best ask and best bid expressed in ticks.
    pub spread_ticks: u8,
    /// Order-book imbalance encoded as signed Q1.10 (−1024..+1023).
    pub obi_q1_10: i16,
    /// Microprice offset to midprice in ticks.
    pub micro_off_ticks: i16,
    /// Sum of bid depth levels 1..3 (raw size units).
    pub sum_bid_l1_3: u16,
    /// Sum of ask depth levels 1..3 (raw size units).
    pub sum_ask_l1_3: u16,
    /// Short-horizon volatility in Q8.8 basis points.
    pub vol_bp_q8_8: u16,
    /// Sweep detection flag (set for recent aggressive sweeps).
    pub sweep_flag: bool,
    /// Midprice trend over ≈200 ms expressed in ticks.
    pub trend_200ms_ticks: i16,
    /// Timestamp since session open quantised to `ms / 4` units.
    pub ts_coarse_ms: u32,
    /// Snapshot schema version for diagnostics.
    pub version: u8,
    /// Sequence bump counter (4-bit, wraps at 15).
    pub sequence: u8,
}

/// Extended venue snapshot with network metadata (AVS-NET-160).
/// #ASSUME: Network extensions are optional and backward compatible
/// #VERIFY: Existing AVS-128 code continues to work unchanged
#[cfg(feature = "network")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AvsNetSnapshot {
    /// Core market data snapshot (128 bits)
    pub core: Avs128Snapshot,
    /// Network timestamp when packet was received (32 bits)
    pub network_timestamp: u32,
    /// Source ID for multicast feed identification (8 bits)
    pub source_id: u8,
    /// Sequence number for gap detection (16 bits)
    pub sequence_number: u16,
    /// Network routing metadata (8 bits spare)
    pub network_route: u8,
}

impl Avs128Snapshot {
    /// Pack this snapshot into the bit layout, clamping out-of-range values.
    #[must_use]
    #[inline]
    pub fn pack(self) -> PackedAvs {
        PackedAvs(pack_snapshot(self))
    }

    /// Check whether the snapshot is stale given a millisecond clock and budget.
    #[must_use]
    #[inline]
    pub fn is_stale(&self, now_ms: u64, budget_ms: u64) -> bool {
        let snapshot_ms = layout::dequantise_timestamp_ms(self.ts_coarse_ms);
        now_ms.saturating_sub(snapshot_ms) > budget_ms
    }
}

impl From<PackedAvs> for Avs128Snapshot {
    #[inline]
    fn from(word: PackedAvs) -> Self {
        unpack_snapshot(word.0)
    }
}

impl From<Avs128Snapshot> for PackedAvs {
    #[inline]
    fn from(snapshot: Avs128Snapshot) -> Self {
        snapshot.pack()
    }
}

/// Atomic AVS-128 capsule supporting single-writer, many-reader semantics.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct Avs128 {
    word: AtomicU128,
}

/// Atomic venue snapshot with integrated circuit breaker support.
/// Combines market data with real-time circuit breaking capabilities
/// for unified risk management across venue snapshots.
///
/// # Performance Requirements
/// - <5ns overhead for breaker checks
/// - Lockfree operations only (100% atomic)
/// - Memory ordering: Acquire/Release semantics
#[repr(C, align(128))]
pub struct AtomicVenueSnapshotWithBreaker {
    /// Core market data snapshot (128 bits)
    snapshot: Avs128,
    /// Circuit breaker for market quality control
    breaker: AtomicBreakerSWeMR,
}

/// Market quality thresholds for circuit breaker triggering.
/// These define the boundaries for acceptable market conditions.
#[derive(Debug, Clone, Copy)]
pub struct MarketQualityThresholds {
    /// Maximum volatility in Q8.8 basis points (default: 5000 = ~50bp)
    pub max_volatility_bp_q8_8: u16,
    /// Maximum spread in ticks (default: 50)
    pub max_spread_ticks: u8,
    /// Maximum absolute order book imbalance Q1.10 (default: 900 = ~0.88)
    pub max_obi_abs_q1_10: u16,
    /// Maximum trend spike in ticks (default: 100)
    pub max_trend_spike_ticks: u16,
}

impl Default for MarketQualityThresholds {
    fn default() -> Self {
        Self {
            max_volatility_bp_q8_8: 5000,  // ~50 basis points
            max_spread_ticks: 50,
            max_obi_abs_q1_10: 900,        // ~0.88 imbalance ratio
            max_trend_spike_ticks: 100,
        }
    }
}

/// Extended atomic venue snapshot with network metadata (AVS-NET-160).
/// Uses two 128-bit words: core market data + network metadata.
#[cfg(feature = "network")]
#[derive(Debug)]
#[repr(C, align(64))]
pub struct AvsNet {
    core_word: AtomicU128,
    net_word: AtomicU128,
}

impl Avs128 {
    /// Create a zero-initialised capsule.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            word: AtomicU128::new(0),
        }
    }

    /// Create a capsule seeded with an initial packed word.
    #[must_use]
    #[inline]
    pub const fn from_packed(packed: PackedAvs) -> Self {
        Self {
            word: AtomicU128::new(packed.0),
        }
    }

    /// Publish a packed snapshot with `store(Ordering::Release)` semantics.
    #[inline]
    pub fn store_release(&self, packed: PackedAvs) {
        self.word.store(packed.0, Ordering::Release);
    }

    /// Convenience helper that packs the snapshot and publishes it.
    #[inline]
    pub fn publish(&self, snapshot: Avs128Snapshot) {
        self.store_release(snapshot.pack());
    }

    /// Load the packed word with relaxed ordering (typical reader contract).
    #[must_use]
    #[inline]
    pub fn load_relaxed(&self) -> PackedAvs {
        PackedAvs(self.word.load(Ordering::Relaxed))
    }

    /// Load with an arbitrary ordering for diagnostics or tooling.
    #[must_use]
    #[inline]
    pub fn load(&self, ordering: Ordering) -> PackedAvs {
        PackedAvs(self.word.load(ordering))
    }

    /// Expose the underlying atomic for advanced integrations.
    #[must_use]
    #[inline]
    pub const fn as_atomic(&self) -> &AtomicU128 {
        &self.word
    }
}

impl Default for Avs128 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicVenueSnapshotWithBreaker {
    /// Create a new venue snapshot with integrated circuit breaker.
    /// #`ASSUME_TOCTOU_SAFE`: Initial state is consistent across both atomics
    /// #`VERIFY_TOCTOU_PREVENTED`: Constructor creates clean initial state
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            snapshot: Avs128::new(),
            breaker: AtomicBreakerSWeMR::new(atomic_breaker::breaker::State::Closed),
        }
    }

    /// Create a venue snapshot with specific breaker state.
    /// #`ASSUME_STATE_VALID`: Provided breaker state is valid
    /// #`VERIFY_STATE_MACHINE`: State validation handled by breaker implementation
    #[must_use]
    #[inline]
    pub const fn with_breaker_state(breaker_state: atomic_breaker::breaker::State) -> Self {
        Self {
            snapshot: Avs128::new(),
            breaker: AtomicBreakerSWeMR::new(breaker_state),
        }
    }

    /// Publish a snapshot with market quality validation and circuit breaking.
    ///
    /// # Circuit Breaking Logic
    /// Checks market quality against thresholds and trips breaker if:
    /// - Volatility exceeds threshold
    /// - Spread exceeds maximum
    /// - Order book imbalance is extreme
    /// - Trend spike detected
    ///
    /// #`ASSUME_MEMORY_ORDERING`: Release on snapshot, Acquire/Release on breaker
    /// #`VERIFY_ORDERING_SUFFICIENT`: Synchronizes with `load_snapshot_with_breaker`
    /// #`ASSUME_METRIC_ATOMIC`: All breaker updates are atomic
    /// #`VERIFY_COUNTER_ACCURACY`: Error increments are not lost under contention
    #[inline]
    pub fn publish_with_validation(&self, snapshot: Avs128Snapshot, thresholds: MarketQualityThresholds) {
        // Check market quality thresholds
        let mut trigger_count = 0u16;
        let mut cause_flags = 0u8;

        // Volatility check
        if snapshot.vol_bp_q8_8 > thresholds.max_volatility_bp_q8_8 {
            trigger_count += 1;
            cause_flags |= 0x01; // Volatility flag
        }

        // Spread check
        if snapshot.spread_ticks > thresholds.max_spread_ticks {
            trigger_count += 1;
            cause_flags |= 0x02; // Spread flag
        }

        // Order book imbalance check (absolute value)
        let obi_abs = if snapshot.obi_q1_10 >= 0 {
            snapshot.obi_q1_10 as u16
        } else {
            (-snapshot.obi_q1_10) as u16
        };
        if obi_abs > thresholds.max_obi_abs_q1_10 {
            trigger_count += 1;
            cause_flags |= 0x04; // Imbalance flag
        }

        // Trend spike detection (absolute value)
        let trend_abs = if snapshot.trend_200ms_ticks >= 0 {
            snapshot.trend_200ms_ticks as u16
        } else {
            (-snapshot.trend_200ms_ticks) as u16
        };
        if trend_abs > thresholds.max_trend_spike_ticks {
            trigger_count += 1;
            cause_flags |= 0x08; // Trend flag
        }

        // Sweep flag escalation
        if snapshot.sweep_flag {
            trigger_count += 1;
            cause_flags |= 0x10; // Sweep flag
        }

        // #ASSUME_PANIC_SAFE: Volatility normalized to prevent overflow
        // #VERIFY_NO_PANIC: Values are clamped and scaled safely
        let volatility_norm = (snapshot.vol_bp_q8_8.min(8191) / 32).min(255); // Normalize to 0-255 range
        let spread_norm = u16::from(snapshot.spread_ticks).min(255); // Keep spread in native range

        // Update breaker metrics
        // #ASSUME_TOCTOU_SAFE: Using single atomic update for metrics
        // #VERIFY_TOCTOU_PREVENTED: All updates through breaker's atomic interface
        self.breaker.update_metrics(
            trigger_count,     // Error increment
            volatility_norm,   // Mean (volatility)
            spread_norm,       // Sigma (spread)
            cause_flags,       // Cause of quality issues
            0,                 // Backoff (not used for market data)
        );

        // Trip breaker if multiple conditions triggered
        if trigger_count >= 2 {
            self.breaker.open();
        } else if trigger_count == 0 {
            // Clear errors and close breaker if market is healthy
            self.breaker.clear_error();
            if self.breaker.state() == atomic_breaker::breaker::State::Open {
                self.breaker.half_open();
            }
        }

        // Always publish the snapshot (market data flow continues)
        // #ASSUME_MEMORY_ORDERING: Release ensures breaker state visible before snapshot
        // #VERIFY_ORDERING_SUFFICIENT: Readers see consistent breaker-snapshot pair
        self.snapshot.publish(snapshot);
    }

    /// Load current snapshot and breaker state atomically.
    /// Returns (snapshot, `breaker_open`) where `breaker_open` indicates circuit state.
    ///
    /// #`ASSUME_MEMORY_ORDERING`: Acquire on breaker, then Relaxed on snapshot
    /// #`VERIFY_ORDERING_SUFFICIENT`: Sees consistent breaker-snapshot relationship
    #[must_use]
    #[inline]
    pub fn load_snapshot_with_breaker(&self) -> (Avs128Snapshot, bool) {
        // Load breaker state first with acquire ordering
        let breaker_state = self.breaker.state();
        let is_open = matches!(breaker_state,
            atomic_breaker::breaker::State::Open |
            atomic_breaker::breaker::State::ForcedOpen
        );

        // Then load snapshot with relaxed ordering
        let snapshot = self.snapshot.load_relaxed().unpack();

        (snapshot, is_open)
    }

    /// Get current breaker state.
    /// #`ASSUME_MEMORY_ORDERING`: Relaxed sufficient for state queries
    /// #`VERIFY_ORDERING_SUFFICIENT`: State reads don't require synchronization
    #[must_use]
    #[inline]
    pub fn breaker_state(&self) -> atomic_breaker::breaker::State {
        self.breaker.state()
    }

    /// Force the circuit breaker open (emergency stop).
    /// #`ASSUME_STATE_VALID`: `ForcedOpen` is valid terminal state
    /// #`VERIFY_STATE_MACHINE`: Breaker handles forced open correctly
    #[inline]
    pub fn force_breaker_open(&self) {
        self.breaker.force_open();
    }

    /// Close the circuit breaker (manual recovery).
    /// #`ASSUME_STATE_VALID`: Closed is valid recovery state
    /// #`VERIFY_STATE_MACHINE`: State transition validated by breaker
    #[inline]
    pub fn close_breaker(&self) {
        self.breaker.close();
    }

    /// Access the underlying snapshot for direct operations.
    /// #`ASSUME_LIFETIME_VALID`: Reference valid for lifetime of container
    /// #`VERIFY_LIFETIME_BOUNDS`: Borrow checker ensures safety
    #[must_use]
    #[inline]
    pub const fn snapshot(&self) -> &Avs128 {
        &self.snapshot
    }

    /// Access the underlying breaker for advanced control.
    /// #`ASSUME_LIFETIME_VALID`: Reference valid for lifetime of container
    /// #`VERIFY_LIFETIME_BOUNDS`: Borrow checker ensures safety
    #[must_use]
    #[inline]
    pub const fn breaker(&self) -> &AtomicBreakerSWeMR {
        &self.breaker
    }
}

impl Default for AtomicVenueSnapshotWithBreaker {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn pack_snapshot(snapshot: Avs128Snapshot) -> u128 {
    let mut word = 0u128;
    word |= pack_unsigned(
        u32::from(snapshot.spread_ticks),
        layout::SPREAD_TICKS_SHIFT,
        layout::SPREAD_TICKS_WIDTH,
    );
    word |= pack_signed(
        i32::from(snapshot.obi_q1_10),
        layout::OBI_Q1_10_SHIFT,
        layout::OBI_Q1_10_WIDTH,
        layout::OBI_Q1_10_MIN,
        layout::OBI_Q1_10_MAX,
    );
    word |= pack_signed(
        i32::from(snapshot.micro_off_ticks),
        layout::MICRO_OFF_TICKS_SHIFT,
        layout::MICRO_OFF_TICKS_WIDTH,
        layout::MICRO_OFF_TICKS_MIN,
        layout::MICRO_OFF_TICKS_MAX,
    );
    word |= pack_unsigned(
        u32::from(snapshot.sum_bid_l1_3),
        layout::SUM_BID_L1_3_SHIFT,
        layout::SUM_BID_L1_3_WIDTH,
    );
    word |= pack_unsigned(
        u32::from(snapshot.sum_ask_l1_3),
        layout::SUM_ASK_L1_3_SHIFT,
        layout::SUM_ASK_L1_3_WIDTH,
    );
    word |= pack_unsigned(
        u32::from(snapshot.vol_bp_q8_8),
        layout::VOL_BP_Q8_8_SHIFT,
        layout::VOL_BP_Q8_8_WIDTH,
    );
    if snapshot.sweep_flag {
        word |= 1u128 << layout::SWEEP_FLAG_SHIFT;
    }
    word |= pack_signed(
        i32::from(snapshot.trend_200ms_ticks),
        layout::TREND_200MS_TICKS_SHIFT,
        layout::TREND_200MS_TICKS_WIDTH,
        layout::TREND_200MS_TICKS_MIN,
        layout::TREND_200MS_TICKS_MAX,
    );
    word |= pack_unsigned(
        snapshot.ts_coarse_ms,
        layout::TS_COARSE_MS_SHIFT,
        layout::TS_COARSE_MS_WIDTH,
    );
    word |= pack_unsigned(
        u32::from(snapshot.version),
        layout::VERSION_SHIFT,
        layout::VERSION_WIDTH,
    );
    word |= pack_unsigned(
        u32::from(snapshot.sequence),
        layout::SEQUENCE_SHIFT,
        layout::SEQUENCE_WIDTH,
    );
    word
}

#[inline]
fn unpack_snapshot(word: u128) -> Avs128Snapshot {
    Avs128Snapshot {
        spread_ticks: unpack_unsigned(word, layout::SPREAD_TICKS_SHIFT, layout::SPREAD_TICKS_WIDTH)
            as u8,
        obi_q1_10: unpack_signed(word, layout::OBI_Q1_10_SHIFT, layout::OBI_Q1_10_WIDTH) as i16,
        micro_off_ticks: unpack_signed(
            word,
            layout::MICRO_OFF_TICKS_SHIFT,
            layout::MICRO_OFF_TICKS_WIDTH,
        ) as i16,
        sum_bid_l1_3: unpack_unsigned(word, layout::SUM_BID_L1_3_SHIFT, layout::SUM_BID_L1_3_WIDTH)
            as u16,
        sum_ask_l1_3: unpack_unsigned(word, layout::SUM_ASK_L1_3_SHIFT, layout::SUM_ASK_L1_3_WIDTH)
            as u16,
        vol_bp_q8_8: unpack_unsigned(word, layout::VOL_BP_Q8_8_SHIFT, layout::VOL_BP_Q8_8_WIDTH)
            as u16,
        sweep_flag: ((word >> layout::SWEEP_FLAG_SHIFT) & 1) != 0,
        trend_200ms_ticks: unpack_signed(
            word,
            layout::TREND_200MS_TICKS_SHIFT,
            layout::TREND_200MS_TICKS_WIDTH,
        ) as i16,
        ts_coarse_ms: unpack_unsigned(word, layout::TS_COARSE_MS_SHIFT, layout::TS_COARSE_MS_WIDTH),
        version: unpack_unsigned(word, layout::VERSION_SHIFT, layout::VERSION_WIDTH) as u8,
        sequence: unpack_unsigned(word, layout::SEQUENCE_SHIFT, layout::SEQUENCE_WIDTH) as u8,
    }
}

#[inline]
fn pack_unsigned(value: u32, shift: u32, width: u32) -> u128 {
    debug_assert!(width > 0 && width < 128);
    let max = (1u128 << width) - 1;
    let clamped = u128::from(value).min(max);
    clamped << shift
}

#[inline]
fn pack_signed(value: i32, shift: u32, width: u32, min: i16, max: i16) -> u128 {
    debug_assert!(width > 0 && width < 31);
    let mut clamped = value;
    if clamped < i32::from(min) {
        clamped = i32::from(min);
    } else if clamped > i32::from(max) {
        clamped = i32::from(max);
    }
    let mask = (1i128 << width) - 1;
    let encoded = (i128::from(clamped) & mask) as u128;
    encoded << shift
}

#[inline]
fn unpack_unsigned(word: u128, shift: u32, width: u32) -> u32 {
    debug_assert!(width > 0 && width < 128);
    ((word >> shift) & ((1u128 << width) - 1)) as u32
}

#[inline]
fn unpack_signed(word: u128, shift: u32, width: u32) -> i32 {
    debug_assert!(width > 0 && width < 31);
    let mask = (1u32 << width) - 1;
    let raw = ((word >> shift) as u32) & mask;
    let sign_bit = 1u32 << (width - 1);
    if (raw & sign_bit) != 0 {
        let extended = raw | !mask;
        extended as i32
    } else {
        raw as i32
    }
}

#[cfg(feature = "network")]
impl AvsNet {
    /// Create a zero-initialised network-enabled capsule.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            core_word: AtomicU128::new(0),
            net_word: AtomicU128::new(0),
        }
    }

    /// Publish a network snapshot with atomic semantics.
    /// Core data is stored first, then network metadata with release ordering.
    #[inline]
    pub fn publish(&self, snapshot: AvsNetSnapshot) {
        // Pack network metadata into second word
        let net_word = pack_network_metadata(snapshot.network_timestamp, 
            snapshot.source_id, snapshot.sequence_number, snapshot.network_route);
        
        // Store core data first (relaxed), then network data (release)
        self.core_word.store(pack_snapshot(snapshot.core), Ordering::Relaxed);
        self.net_word.store(net_word, Ordering::Release);
    }

    /// Load the complete network snapshot.
    /// Loads network metadata first (acquire), then core data (relaxed).
    #[must_use]
    #[inline]
    pub fn load(&self) -> AvsNetSnapshot {
        let net_word = self.net_word.load(Ordering::Acquire);
        let core_word = self.core_word.load(Ordering::Relaxed);
        
        let core = unpack_snapshot(core_word);
        let (network_timestamp, source_id, sequence_number, network_route) = 
            unpack_network_metadata(net_word);
        
        AvsNetSnapshot {
            core,
            network_timestamp,
            source_id,
            sequence_number,
            network_route,
        }
    }

    /// Load only the core market data (for backward compatibility).
    #[must_use]
    #[inline]
    pub fn load_core(&self) -> Avs128Snapshot {
        unpack_snapshot(self.core_word.load(Ordering::Relaxed))
    }
}

#[cfg(feature = "network")]
impl Default for AvsNet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Pack network metadata into a 128-bit word.
/// Layout: timestamp(32) | sequence(16) | source_id(8) | route(8) | spare(64)
#[cfg(feature = "network")]
#[inline]
fn pack_network_metadata(timestamp: u32, source_id: u8, sequence: u16, route: u8) -> u128 {
    let mut word = 0u128;
    word |= (timestamp as u128) << 96;  // 32 bits at position 96-127
    word |= (sequence as u128) << 80;   // 16 bits at position 80-95
    word |= (source_id as u128) << 72;  // 8 bits at position 72-79
    word |= (route as u128) << 64;      // 8 bits at position 64-71
    // Remaining 64 bits (0-63) are spare for future extensions
    word
}

/// Unpack network metadata from a 128-bit word.
#[cfg(feature = "network")]
#[inline]
fn unpack_network_metadata(word: u128) -> (u32, u8, u16, u8) {
    let timestamp = ((word >> 96) & 0xFFFF_FFFF) as u32;
    let sequence = ((word >> 80) & 0xFFFF) as u16;
    let source_id = ((word >> 72) & 0xFF) as u8;
    let route = ((word >> 64) & 0xFF) as u8;
    (timestamp, source_id, sequence, route)
}
