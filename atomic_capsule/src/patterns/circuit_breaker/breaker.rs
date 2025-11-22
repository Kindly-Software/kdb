//! Atomic breaker primitives (single-writer and MPMC variants).

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "circuit-breaker-compact48")]
use super::layout::compact48;
use super::layout::{standard64, DefaultLayout, Layout, LayoutRaw, Standard64};
#[cfg(feature = "std")]
use super::telemetry::TelemetrySample;

#[cfg(all(
    feature = "circuit-breaker-auto-tune",
    feature = "circuit-breaker-compact48"
))]
use super::layout::unpack_q6_10;
#[cfg(feature = "circuit-breaker-auto-tune")]
use super::layout::unpack_q8_8;
#[cfg(feature = "circuit-breaker-compact48")]
use super::layout::Compact48;

#[cfg(feature = "diagnostics")]
#[inline]
fn assert_state_level_invariants(state: State, level: u8) {
    debug_assert!(level <= 3, "level {} exceeds encoding", level);
    debug_assert!(
        state != State::ForcedOpen || level == 3,
        "forced open must publish level 3 (got {})",
        level
    );
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
fn assert_state_level_invariants(_: State, _: u8) {}

#[cfg(feature = "diagnostics")]
#[inline]
fn assert_backoff_range(backoff: u8) {
    debug_assert!(backoff <= 63, "backoff out of range: {}", backoff);
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
fn assert_backoff_range(_: u8) {}

/// Breaker states encoded into the packed word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[derive(Default)]
pub enum State {
    /// Normal operation.
    #[default]
    Closed = 0,
    /// Limited probing for recovery.
    HalfOpen = 1,
    /// Actively rejecting load.
    Open = 2,
    /// Operator-forced open condition.
    ForcedOpen = 3,
}

impl State {
    /// Construct from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x3 {
            0 => Self::Closed,
            1 => Self::HalfOpen,
            2 => Self::Open,
            _ => Self::ForcedOpen,
        }
    }

    /// Convert state to raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self as u8
    }
}

/// Describe which layout is active for a breaker instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// Standard 64-bit layout with causes/backoff.
    Standard64,
    /// Compact 48-bit layout (stored in the lower bits of the word).
    #[cfg(feature = "circuit-breaker-compact48")]
    Compact48,
}

impl LayoutKind {
    /// Return the default layout selected at compile time.
    #[must_use]
    pub const fn default() -> Self {
        #[cfg(feature = "circuit-breaker-standard64")]
        {
            Self::Standard64
        }

        #[cfg(all(
            not(feature = "circuit-breaker-standard64"),
            feature = "circuit-breaker-compact48"
        ))]
        {
            Self::Compact48
        }
    }
}

/// Trait implemented by breaker primitives that can be driven by policies.
pub trait BreakerLike {
    /// Layout carried by the breaker word.
    type Layout: Layout;

    /// Return the layout kind for this breaker.
    fn layout_kind(&self) -> LayoutKind;

    /// Load the packed word using relaxed semantics.
    fn load_relaxed(&self) -> u64;

    /// Load the packed word using acquire semantics (for config-dependent readers).
    fn load_acquire(&self) -> u64;

    /// Store a packed word using release semantics.
    fn store_release(&self, new: u64);

    /// Update metrics and auxiliary fields, preserving state/level.
    fn update_metrics(&self, err_inc: u16, mu_q: u16, sg_q: u16, cause: u8, backoff: u8);

    /// Reset the error counter while preserving other metrics.
    fn clear_error(&self);

    /// Set the breaker level (2-bit field).
    fn set_level(&self, level: u8);

    /// Set both state and level in one store.
    fn set_state_level(&self, state: State, level: u8);

    /// Apply a telemetry sample (no-op by default).
    #[cfg(feature = "std")]
    fn apply_sample(&self, sample: &TelemetrySample) {
        let _ = sample;
    }
}

/// Newtype for the single-writer/many-reader breaker.
#[derive(Default, Debug)]
pub struct AtomicBreakerSWeMR(pub AtomicU64);

impl AtomicBreakerSWeMR {
    /// Create a breaker with the given state using the standard 64-bit layout.
    #[must_use]
    pub const fn new_standard64(state: State) -> Self {
        let word = (state.bits() & 0x3) as u64;
        Self(AtomicU64::new(word))
    }

    /// Create a breaker with the given state using the default layout selected by features.
    #[must_use]
    pub const fn new(state: State) -> Self {
        #[cfg(feature = "circuit-breaker-standard64")]
        {
            Self::new_standard64(state)
        }

        #[cfg(all(
            not(feature = "circuit-breaker-standard64"),
            feature = "circuit-breaker-compact48"
        ))]
        {
            Self::new_compact48(state)
        }
    }

    /// Create a breaker from an already packed word.
    #[must_use]
    pub const fn from_packed(packed: u64) -> Self {
        Self(AtomicU64::new(packed))
    }

    /// Return a reference to the inner atomic for low-level integrations.
    #[must_use]
    pub const fn atomic(&self) -> &AtomicU64 {
        &self.0
    }

    /// Load the packed word using relaxed ordering.
    #[must_use]
    pub fn load_relaxed(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Load the packed word using acquire ordering.
    #[must_use]
    pub fn load_acquire(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Store an updated packed word with release ordering.
    pub fn store_release(&self, new: u64) {
        self.0.store(new, Ordering::Release);
    }

    /// Return the layout kind for this breaker instance.
    #[must_use]
    pub const fn layout_kind(&self) -> LayoutKind {
        LayoutKind::default()
    }

    /// Extract the current state using relaxed ordering.
    #[must_use]
    pub fn state(&self) -> State {
        let word = self.load_relaxed();
        let bits = match self.layout_kind() {
            LayoutKind::Standard64 => standard64::state(word),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => compact48::state(word),
        };
        State::from_bits(bits)
    }

    /// Extract the current level using relaxed ordering.
    #[must_use]
    pub fn level(&self) -> u8 {
        let word = self.load_relaxed();
        match self.layout_kind() {
            LayoutKind::Standard64 => standard64::level(word),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => compact48::level(word),
        }
    }

    /// Extract the current cause bits (standard layout).
    #[must_use]
    pub fn cause(&self) -> u8 {
        let word = self.load_relaxed();
        match self.layout_kind() {
            LayoutKind::Standard64 => standard64::cause(word),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => 0,
        }
    }

    /// Extract the backoff index for the current word (standard layout).
    #[must_use]
    pub fn backoff(&self) -> u8 {
        let word = self.load_relaxed();
        match self.layout_kind() {
            LayoutKind::Standard64 => standard64::backoff(word),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => 0,
        }
    }

    /// Transition to the Closed state while preserving metrics.
    pub fn close(&self) {
        self.set_state_bits(State::Closed);
    }

    /// Transition to the `HalfOpen` state while preserving metrics.
    pub fn half_open(&self) {
        self.set_state_bits(State::HalfOpen);
    }

    /// Transition to the Open state while preserving metrics.
    pub fn open(&self) {
        self.set_state_bits(State::Open);
    }

    /// Force the breaker open, preserving metrics.
    pub fn force_open(&self) {
        self.set_state_bits(State::ForcedOpen);
    }

    fn set_state_bits(&self, state: State) {
        let cur = self.load_relaxed();
        let new_word = match self.layout_kind() {
            LayoutKind::Standard64 => standard64::with_state(cur, state.bits()),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => compact48::with_state(cur, state.bits()),
        };
        #[cfg(feature = "diagnostics")]
        {
            let level = match self.layout_kind() {
                LayoutKind::Standard64 => standard64::level(new_word),
                #[cfg(feature = "circuit-breaker-compact48")]
                LayoutKind::Compact48 => compact48::level(new_word),
            };
            assert_state_level_invariants(state, level);
        }
        self.store_release(new_word);
    }

    /// Update metrics for the standard layout.
    pub fn update_metrics_standard64(
        &self,
        err_inc: u16,
        mu_q: u16,
        sg_q: u16,
        cause: u8,
        backoff: u8,
    ) {
        assert_backoff_range(backoff);
        let cur = self.load_relaxed();
        let state_level = cur & standard64::STATE_LEVEL_MASK;
        let old_err = standard64::err(cur);
        let new_err = old_err.saturating_add(err_inc).min(0x3fff);
        let metrics = standard64::pack_metrics(new_err, mu_q, sg_q, cause, backoff);
        let new_word = state_level | metrics;
        self.store_release(new_word);
    }

    /// Reset the error counter to zero while preserving other metrics.
    pub fn clear_error_standard64(&self) {
        let cur = self.load_relaxed();
        let mu = standard64::mu(cur);
        let sigma = standard64::sigma(cur);
        let cause = standard64::cause(cur);
        let backoff = standard64::backoff(cur);
        let metrics = standard64::pack_metrics(0, mu, sigma, cause, backoff);
        let new_word = (cur & standard64::STATE_LEVEL_MASK) | metrics;
        self.store_release(new_word);
    }

    /// Adjust the breaker level while preserving other fields.
    pub fn set_level_standard64(&self, level: u8) {
        let cur = self.load_relaxed();
        let word = standard64::with_level(cur, level & 0x3);
        assert_state_level_invariants(State::from_bits(standard64::state(word)), level & 0x3);
        self.store_release(word);
    }

    /// Set state and level atomically during the single-writer phase.
    pub fn set_state_level_standard64(&self, state: State, level: u8) {
        let cur = self.load_relaxed();
        let mut word = standard64::with_state(cur, state.bits());
        word = standard64::with_level(word, level & 0x3);
        assert_state_level_invariants(state, level & 0x3);
        self.store_release(word);
    }

    /// Apply a backoff index.
    #[cfg(feature = "circuit-breaker-standard64")]
    pub fn set_backoff_standard64(&self, backoff: u8) {
        assert_backoff_range(backoff);
        let cur = self.load_relaxed();
        let metrics = standard64::pack_metrics(
            standard64::err(cur),
            standard64::mu(cur),
            standard64::sigma(cur),
            standard64::cause(cur),
            backoff,
        );
        let new_word = (cur & standard64::STATE_LEVEL_MASK) | metrics;
        self.store_release(new_word);
    }

    /// Update metrics for the default layout compiled into the crate.
    pub fn update_metrics(&self, err_inc: u16, mu_q: u16, sg_q: u16, cause: u8, backoff: u8) {
        match self.layout_kind() {
            LayoutKind::Standard64 => {
                self.update_metrics_standard64(err_inc, mu_q, sg_q, cause, backoff);
            }
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => self.update_metrics_compact48(err_inc, mu_q, sg_q),
        }
    }

    /// Apply a pre-normalised telemetry sample to the breaker.
    #[cfg(feature = "std")]
    pub fn apply_sample(&self, sample: &TelemetrySample) {
        let clamped = sample.clamped();
        match self.layout_kind() {
            LayoutKind::Standard64 => {
                let word = self.load_relaxed();
                let mu_q = super::layout::pack_q8_8(clamped.mu_norm);
                let sg_q = super::layout::pack_q8_8(clamped.sg_norm);
                let backoff = clamped
                    .backoff_hint
                    .unwrap_or_else(|| standard64::backoff(word));
                self.update_metrics_standard64(clamped.err_inc, mu_q, sg_q, clamped.cause, backoff);
            }
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => {
                let mu_q = super::layout::pack_q6_10(clamped.mu_norm);
                let sg_q = super::layout::pack_q6_10(clamped.sg_norm);
                self.update_metrics_compact48(clamped.err_inc, mu_q, sg_q);
            }
        }
    }

    /// Clear the error counter for the default layout.
    pub fn clear_error(&self) {
        match self.layout_kind() {
            LayoutKind::Standard64 => self.clear_error_standard64(),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => self.clear_error_compact48(),
        }
    }

    /// Set breaker level respecting the default layout.
    pub fn set_level(&self, level: u8) {
        match self.layout_kind() {
            LayoutKind::Standard64 => self.set_level_standard64(level),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => self.set_level_compact48(level),
        }
    }

    /// Set state and level for the default layout.
    pub fn set_state_level(&self, state: State, level: u8) {
        match self.layout_kind() {
            LayoutKind::Standard64 => self.set_state_level_standard64(state, level),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => self.set_state_level_compact48(state, level),
        }
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    /// Create a breaker with the compact layout.
    pub const fn new_compact48(state: State) -> Self {
        let word = (state.bits() & 0x3) as u64;
        Self(AtomicU64::new(word))
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    fn update_metrics_compact48(&self, err_inc: u16, mu_q: u16, sg_q: u16) {
        let cur = self.load_relaxed();
        let state_level = cur & compact48::STATE_LEVEL_MASK;
        let old_err = compact48::err(cur);
        let new_err = old_err.saturating_add(err_inc).min(0x0fff);
        let metrics = compact48::pack_metrics(new_err, mu_q, sg_q);
        let new_word = state_level | metrics;
        self.store_release(new_word);
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    fn clear_error_compact48(&self) {
        let cur = self.load_relaxed();
        let metrics = compact48::pack_metrics(0, compact48::mu(cur), compact48::sigma(cur));
        let new_word = (cur & compact48::STATE_LEVEL_MASK) | metrics;
        self.store_release(new_word);
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    fn set_level_compact48(&self, level: u8) {
        let cur = self.load_relaxed();
        let new_word = compact48::with_level(cur, level & 0x3);
        self.store_release(new_word);
    }

    #[cfg(feature = "circuit-breaker-compact48")]
    fn set_state_level_compact48(&self, state: State, level: u8) {
        let cur = self.load_relaxed();
        let word = compact48::with_level(compact48::with_state(cur, state.bits()), level & 0x3);
        self.store_release(word);
    }
}

impl BreakerLike for AtomicBreakerSWeMR {
    type Layout = DefaultLayout;

    fn layout_kind(&self) -> LayoutKind {
        AtomicBreakerSWeMR::layout_kind(self)
    }

    fn load_relaxed(&self) -> u64 {
        AtomicBreakerSWeMR::load_relaxed(self)
    }

    fn load_acquire(&self) -> u64 {
        AtomicBreakerSWeMR::load_acquire(self)
    }

    fn store_release(&self, new: u64) {
        AtomicBreakerSWeMR::store_release(self, new);
    }

    fn update_metrics(&self, err_inc: u16, mu_q: u16, sg_q: u16, cause: u8, backoff: u8) {
        AtomicBreakerSWeMR::update_metrics(self, err_inc, mu_q, sg_q, cause, backoff);
    }

    fn clear_error(&self) {
        AtomicBreakerSWeMR::clear_error(self);
    }

    fn set_level(&self, level: u8) {
        AtomicBreakerSWeMR::set_level(self, level);
    }

    fn set_state_level(&self, state: State, level: u8) {
        AtomicBreakerSWeMR::set_state_level(self, state, level);
    }

    #[cfg(feature = "std")]
    fn apply_sample(&self, sample: &TelemetrySample) {
        AtomicBreakerSWeMR::apply_sample(self, sample);
    }
}

/// Guard that parses a packed word once and exposes accessor helpers.
#[derive(Clone, Copy, Debug)]
pub struct AtomicBreakerGuard {
    packed: u64,
    raw: LayoutRaw,
    layout: LayoutKind,
}

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Snapshot of the breaker hot-word decoded into floating-point ratios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetricsSnapshot {
    /// Current breaker state.
    pub state: State,
    /// Current breaker level.
    pub level: u8,
    /// Saturating error counter.
    pub err: u16,
    /// Normalised mean metric as a floating-point ratio.
    pub mu_norm: f32,
    /// Normalised jitter metric as a floating-point ratio.
    pub sg_norm: f32,
    /// Cause flags (standard layout only; zero for compact).
    pub cause: u8,
    /// Backoff hint (standard layout only; zero for compact).
    pub backoff: u8,
}

impl AtomicBreakerGuard {
    /// Create a guard by unpacking a standard-layout word.
    #[must_use]
    pub fn new(packed: u64) -> Self {
        Self::from_layout(packed, LayoutKind::default())
    }

    /// Create a guard from a word and explicit layout kind.
    #[must_use]
    pub fn from_layout(packed: u64, layout: LayoutKind) -> Self {
        let raw = match layout {
            LayoutKind::Standard64 => Standard64::unpack(packed),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => Compact48::unpack(packed),
        };
        Self {
            packed,
            raw,
            layout,
        }
    }

    /// Return the raw packed bits.
    #[must_use]
    pub const fn packed(&self) -> u64 {
        self.packed
    }

    /// Access the decoded state.
    #[must_use]
    pub const fn state(&self) -> State {
        State::from_bits(self.raw.state)
    }

    /// Access the unpacked word components.
    #[must_use]
    pub const fn raw(&self) -> LayoutRaw {
        self.raw
    }

    /// Access the decoded level.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.raw.level
    }

    /// Access the decoded error counter.
    #[must_use]
    pub const fn err(&self) -> u16 {
        self.raw.err
    }

    /// Access the decoded mean metric.
    #[must_use]
    pub const fn mu_norm(&self) -> u16 {
        self.raw.mu_norm
    }

    /// Access the decoded jitter metric.
    #[must_use]
    pub const fn sg_norm(&self) -> u16 {
        self.raw.sg_norm
    }

    /// Access the decoded cause flags (standard layout only).
    #[must_use]
    pub const fn cause(&self) -> u8 {
        self.raw.cause
    }

    /// Access the decoded backoff index (standard layout only).
    #[must_use]
    pub const fn backoff(&self) -> u8 {
        self.raw.backoff
    }

    /// Return the layout kind used for decoding.
    #[must_use]
    pub const fn layout(&self) -> LayoutKind {
        self.layout
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    /// Return the mean metric as a floating-point ratio.
    #[must_use]
    pub fn mu_ratio(&self) -> f32 {
        match self.layout {
            LayoutKind::Standard64 => unpack_q8_8(self.raw.mu_norm),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => unpack_q6_10(self.raw.mu_norm),
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    /// Return the jitter metric as a floating-point ratio.
    #[must_use]
    pub fn sg_ratio(&self) -> f32 {
        match self.layout {
            LayoutKind::Standard64 => unpack_q8_8(self.raw.sg_norm),
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => unpack_q6_10(self.raw.sg_norm),
        }
    }

    #[cfg(feature = "circuit-breaker-auto-tune")]
    /// Package the decoded fields into a metrics snapshot.
    #[must_use]
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            state: self.state(),
            level: self.level(),
            err: self.err(),
            mu_norm: self.mu_ratio(),
            sg_norm: self.sg_ratio(),
            cause: self.cause(),
            backoff: self.backoff(),
        }
    }
}

#[cfg(feature = "circuit-breaker-mpmc")]
/// Multi-producer/multi-consumer breaker variant using CAS for updates.
pub struct AtomicBreakerMPMC(pub AtomicU64);

#[cfg(feature = "circuit-breaker-mpmc")]
impl AtomicBreakerMPMC {
    /// Create a breaker with the given state using the default layout.
    pub fn new(state: State) -> Self {
        #[cfg(feature = "circuit-breaker-standard64")]
        {
            let word = AtomicBreakerSWeMR::new_standard64(state).load_relaxed();
            Self(AtomicU64::new(word))
        }

        #[cfg(all(
            not(feature = "circuit-breaker-standard64"),
            feature = "circuit-breaker-compact48"
        ))]
        {
            let word = AtomicBreakerSWeMR::new_compact48(state).load_relaxed();
            Self(AtomicU64::new(word))
        }
    }

    /// Load with relaxed ordering.
    #[must_use]
    pub fn load_relaxed(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Load with acquire ordering.
    #[must_use]
    pub fn load_acquire(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Store with release ordering.
    pub fn store_release(&self, new: u64) {
        self.0.store(new, Ordering::Release);
    }

    /// Return the layout kind selected at compile time.
    #[must_use]
    pub const fn layout_kind(&self) -> LayoutKind {
        LayoutKind::default()
    }

    /// Update metrics using a bounded CAS loop.
    pub fn update_metrics_cas(
        &self,
        err_inc: u16,
        mu_q: u16,
        sg_q: u16,
        cause: u8,
        backoff: u8,
        max_retries: usize,
    ) -> Result<(), usize> {
        let mut retries = 0usize;
        let layout = self.layout_kind();
        let result = self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                if retries >= max_retries {
                    return None;
                }
                retries += 1;
                let state_level = match layout {
                    LayoutKind::Standard64 => cur & standard64::STATE_LEVEL_MASK,
                    #[cfg(feature = "circuit-breaker-compact48")]
                    LayoutKind::Compact48 => cur & compact48::STATE_LEVEL_MASK,
                };
                let new_word = match layout {
                    LayoutKind::Standard64 => {
                        let old_err = standard64::err(cur);
                        let new_err = old_err.saturating_add(err_inc).min(0x3fff);
                        let metrics = standard64::pack_metrics(new_err, mu_q, sg_q, cause, backoff);
                        state_level | metrics
                    }
                    #[cfg(feature = "circuit-breaker-compact48")]
                    LayoutKind::Compact48 => {
                        let old_err = compact48::err(cur);
                        let new_err = old_err.saturating_add(err_inc).min(0x0fff);
                        let metrics = compact48::pack_metrics(new_err, mu_q, sg_q);
                        state_level | metrics
                    }
                };
                Some(new_word)
            });

        if result.is_err() {
            Err(retries)
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "circuit-breaker-mpmc")]
impl BreakerLike for AtomicBreakerMPMC {
    type Layout = DefaultLayout;

    fn layout_kind(&self) -> LayoutKind {
        AtomicBreakerMPMC::layout_kind(self)
    }

    fn load_relaxed(&self) -> u64 {
        AtomicBreakerMPMC::load_relaxed(self)
    }

    fn load_acquire(&self) -> u64 {
        AtomicBreakerMPMC::load_acquire(self)
    }

    fn store_release(&self, new: u64) {
        AtomicBreakerMPMC::store_release(self, new);
    }

    fn update_metrics(&self, err_inc: u16, mu_q: u16, sg_q: u16, cause: u8, backoff: u8) {
        let _ = self.update_metrics_cas(err_inc, mu_q, sg_q, cause, backoff, 8);
    }

    fn clear_error(&self) {
        let mut current = self.load_relaxed();
        loop {
            let new_word = match self.layout_kind() {
                LayoutKind::Standard64 => {
                    let metrics = standard64::pack_metrics(
                        0,
                        standard64::mu(current),
                        standard64::sigma(current),
                        standard64::cause(current),
                        standard64::backoff(current),
                    );
                    (current & standard64::STATE_LEVEL_MASK) | metrics
                }
                #[cfg(feature = "circuit-breaker-compact48")]
                LayoutKind::Compact48 => {
                    let metrics = compact48::pack_metrics(
                        0,
                        compact48::mu(current),
                        compact48::sigma(current),
                    );
                    (current & compact48::STATE_LEVEL_MASK) | metrics
                }
            };
            match self
                .0
                .compare_exchange(current, new_word, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    fn set_level(&self, level: u8) {
        let mut current = self.load_relaxed();
        loop {
            let new_word = match self.layout_kind() {
                LayoutKind::Standard64 => standard64::with_level(current, level & 0x3),
                #[cfg(feature = "circuit-breaker-compact48")]
                LayoutKind::Compact48 => {
                    let cleared = current & !compact48::LEVEL_MASK;
                    cleared | (u64::from(level & 0x3) << 2)
                }
            };
            match self
                .0
                .compare_exchange(current, new_word, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    fn set_state_level(&self, state: State, level: u8) {
        let mut current = self.load_relaxed();
        loop {
            let new_word = match self.layout_kind() {
                LayoutKind::Standard64 => {
                    let mut word = standard64::with_state(current, state.bits());
                    word = standard64::with_level(word, level & 0x3);
                    word
                }
                #[cfg(feature = "circuit-breaker-compact48")]
                LayoutKind::Compact48 => {
                    let cleared_state = current & !(compact48::STATE_MASK | compact48::LEVEL_MASK);
                    cleared_state | u64::from(state.bits()) | (u64::from(level & 0x3) << 2)
                }
            };
            match self
                .0
                .compare_exchange(current, new_word, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    #[cfg(feature = "std")]
    fn apply_sample(&self, sample: &TelemetrySample) {
        let clamped = sample.clamped();
        match self.layout_kind() {
            LayoutKind::Standard64 => {
                let mu_q = super::layout::pack_q8_8(clamped.mu_norm);
                let sg_q = super::layout::pack_q8_8(clamped.sg_norm);
                let _ = self.update_metrics_cas(
                    clamped.err_inc,
                    mu_q,
                    sg_q,
                    clamped.cause,
                    clamped.backoff_hint.unwrap_or(0),
                    8,
                );
            }
            #[cfg(feature = "circuit-breaker-compact48")]
            LayoutKind::Compact48 => {
                let mu_q = super::layout::pack_q6_10(clamped.mu_norm);
                let sg_q = super::layout::pack_q6_10(clamped.sg_norm);
                let _ = self.update_metrics_cas(clamped.err_inc, mu_q, sg_q, 0, 0, 8);
            }
        }
    }
}

#[cfg(feature = "circuit-breaker-serde")]
mod serde_impls {
    use super::{AtomicBreakerGuard, AtomicBreakerSWeMR, LayoutKind};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for AtomicBreakerSWeMR {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_u64(self.load_relaxed())
        }
    }

    impl<'de> Deserialize<'de> for AtomicBreakerSWeMR {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let packed = u64::deserialize(deserializer)?;
            Ok(Self::from_packed(packed))
        }
    }

    impl Serialize for AtomicBreakerGuard {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_u64(self.packed())
        }
    }

    impl<'de> Deserialize<'de> for AtomicBreakerGuard {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let packed = u64::deserialize(deserializer)?;
            Ok(Self::from_layout(packed, LayoutKind::default()))
        }
    }
}

#[cfg(all(test, feature = "std", feature = "circuit-breaker-standard64"))]
mod tests {
    use super::*;
    use crate::cause;
    use std::sync::{atomic::AtomicUsize, atomic::Ordering as StdOrdering, Arc, Barrier};

    fn exercise_breaker_like<B: BreakerLike>(breaker: &B) {
        let layout = breaker.layout_kind();
        let snapshot = breaker.load_relaxed();
        let _ = layout;
        let _ = breaker.load_acquire();
        breaker.store_release(snapshot);
        breaker.update_metrics(0, 0, 0, 0, 0);
        breaker.clear_error();
        breaker.set_level(0);
        breaker.set_state_level(State::Closed, 0);
    }

    #[test]
    fn metrics_update_saturates_error() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        breaker.update_metrics_standard64(0x3ffe, 100, 200, 0, 0);
        breaker.update_metrics_standard64(8, 120, 220, 0, 0);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 0x3fff);
        assert_eq!(guard.mu_norm(), 120);
        assert_eq!(guard.sg_norm(), 220);
    }

    #[test]
    fn state_transition_preserves_metrics() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        breaker.update_metrics_standard64(4, 3000, 4000, cause::LAT, 3);
        breaker.open();
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Open);
        assert_eq!(guard.err(), 4);
        assert_eq!(guard.mu_norm(), 3000);
        assert_eq!(guard.sg_norm(), 4000);
        assert_eq!(guard.cause() & cause::LAT, cause::LAT);
        assert_eq!(guard.backoff(), 3);
    }

    #[test]
    fn release_acquire_orders_payload() {
        let breaker = Arc::new(AtomicBreakerSWeMR::new_standard64(State::Closed));
        let payload = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(2));

        let reader_breaker = Arc::clone(&breaker);
        let reader_payload = Arc::clone(&payload);
        let reader_barrier = Arc::clone(&barrier);

        let handle = std::thread::spawn(move || {
            reader_barrier.wait();
            loop {
                let packed = reader_breaker.load_acquire();
                let guard = AtomicBreakerGuard::new(packed);
                if guard.state() == State::Open {
                    return reader_payload.load(StdOrdering::Acquire);
                }
                std::thread::yield_now();
            }
        });

        barrier.wait();
        payload.store(0xDEAD_BEEFu64 as usize, StdOrdering::Release);
        breaker.open();

        let observed = handle.join().expect("reader thread panicked");
        assert_eq!(observed, 0xDEAD_BEEFu64 as usize);
    }

    #[test]
    fn backoff_increments_on_open() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        breaker.update_metrics_standard64(0, 0, 0, 0, 1);
        breaker.open();
        let first = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(first.backoff(), 1);

        breaker.update_metrics_standard64(0, 0, 0, 0, first.backoff());
        breaker.open();
        let second = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert!(second.backoff() >= first.backoff());
    }

    #[test]
    fn clear_error_resets_counter() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        breaker.update_metrics_standard64(12, 100, 200, 0, 0);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 12);

        breaker.clear_error();
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 0);
        assert_eq!(guard.mu_norm(), 100);
        assert_eq!(guard.sg_norm(), 200);

        breaker.update_metrics_standard64(2, 150, 250, 0, 0);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 2);
    }

    #[cfg(feature = "circuit-breaker-standard64")]
    #[test]
    fn swe_mr_full_api_surface() {
        let breaker = AtomicBreakerSWeMR::new(State::HalfOpen);
        assert_eq!(breaker.layout_kind(), LayoutKind::Standard64);
        assert_eq!(breaker.state(), State::HalfOpen);
        assert_eq!(breaker.level(), 0);

        breaker.set_level(2);
        assert_eq!(breaker.level(), 2);

        breaker.close();
        breaker.open();
        breaker.half_open();
        breaker.force_open();
        assert_eq!(breaker.state(), State::ForcedOpen);

        breaker.set_state_level(State::Open, 1);
        assert_eq!(breaker.state(), State::Open);
        assert_eq!(breaker.level(), 1);

        breaker.set_backoff_standard64(9);
        assert_eq!(breaker.backoff(), 9);

        breaker.update_metrics(3, 111, 222, cause::CPU, 4);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.cause() & cause::CPU, cause::CPU);
        assert_eq!(breaker.cause() & cause::CPU, cause::CPU);
        assert_eq!(breaker.backoff(), 4);

        let packed = breaker.load_relaxed();
        let copy = AtomicBreakerSWeMR::from_packed(packed);
        assert_eq!(copy.state(), breaker.state());
        assert_eq!(copy.level(), breaker.level());

        let _atomic_ref = breaker.atomic();

        let guard = AtomicBreakerGuard::new(packed);
        assert_eq!(guard.raw().state, guard.state().bits());
        assert_eq!(guard.raw().level, guard.level());

        exercise_breaker_like(&breaker);
    }

    #[test]
    fn apply_sample_updates_metrics() {
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let sample = TelemetrySample {
            mu_norm: 1.5,
            sg_norm: 0.75,
            err_inc: 2,
            cause: cause::LAT,
            backoff_hint: Some(7),
        };
        breaker.apply_sample(&sample);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 2);
        assert!(guard.mu_norm() >= super::layout::pack_q8_8(1.5));
        assert!(guard.sg_norm() >= super::layout::pack_q8_8(0.75));
        assert_eq!(guard.cause() & cause::LAT, cause::LAT);
        assert_eq!(guard.backoff(), 7);
    }

    #[cfg(all(
        feature = "circuit-breaker-mpmc",
        feature = "circuit-breaker-standard64"
    ))]
    #[test]
    fn mpmc_variants_cover_paths() {
        let mpmc = AtomicBreakerMPMC::new(State::Closed);
        assert_eq!(mpmc.layout_kind(), LayoutKind::Standard64);

        let current = mpmc.load_relaxed();
        mpmc.store_release(current);

        mpmc.update_metrics_cas(1, 200, 300, cause::LAT, 2, 8)
            .expect("CAS update succeeds");
        let err = mpmc
            .update_metrics_cas(1, 200, 300, cause::LAT, 2, 0)
            .expect_err("retry budget exhausted");
        assert_eq!(err, 0);
        mpmc.clear_error();
        mpmc.set_level(3);
        mpmc.set_state_level(State::Open, 2);

        let guard = AtomicBreakerGuard::new(mpmc.load_relaxed());
        assert_eq!(guard.state(), State::Open);
        assert_eq!(guard.level(), 2);

        exercise_breaker_like(&mpmc);
    }
}

#[cfg(all(test, feature = "std", feature = "circuit-breaker-compact48"))]
mod compact_tests {
    use super::*;

    #[test]
    fn compact_layout_saturates_error() {
        let breaker = AtomicBreakerSWeMR::new_compact48(State::Closed);
        breaker.update_metrics(0x0ff0, 100, 200, 0, 0);
        breaker.update_metrics(32, 150, 250, 0, 0);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.err(), 0x0fff);
        assert_eq!(guard.mu_norm(), 150);
        assert_eq!(guard.sg_norm(), 250);
    }

    #[test]
    fn compact_state_transitions() {
        let breaker = AtomicBreakerSWeMR::new_compact48(State::Closed);
        breaker.open();
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Open);
        breaker.set_state_level(State::HalfOpen, 2);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::HalfOpen);
        assert_eq!(guard.level(), 2);
    }
}
