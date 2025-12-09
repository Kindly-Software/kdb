#![no_std]

//! ALT-128 (Atomic Latency Ticket) exposes a single atomic load path for
//! connectivity and execution health. Writers apply one release-store to publish
//! a freshly packed `u128`, and readers inspect the snapshot with one relaxed
//! load to gate breaker logic, routing, or strategy posture adjustments.

use core::{cmp, sync::atomic::Ordering};

use portable_atomic::AtomicU128;

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy)]
struct Field {
    shift: u8,
    bits: u8,
}

impl Field {
    const fn value_mask(self) -> u128 {
        if self.bits == 0 {
            0
        } else if self.bits as u32 >= 128 {
            u128::MAX
        } else {
            (1u128 << self.bits) - 1
        }
    }

    const fn mask(self) -> u128 {
        self.value_mask() << self.shift
    }
}

#[inline]
fn set_field(word: u128, field: Field, value: u128) -> u128 {
    debug_assert_eq!(value & !field.value_mask(), 0, "field overflow");
    let cleared = word & !field.mask();
    cleared | ((value & field.value_mask()) << field.shift)
}

#[inline]
fn get_field(word: u128, field: Field) -> u32 {
    ((word >> field.shift) & field.value_mask()) as u32
}

const F_FEED_TO_DECISION: Field = Field { shift: 0, bits: 12 };
const F_DECISION_TO_ACK: Field = Field {
    shift: 12,
    bits: 12,
};
const F_ACK_TO_FILL: Field = Field {
    shift: 24,
    bits: 12,
};
const F_REJECT_RATE: Field = Field {
    shift: 36,
    bits: 10,
};
const F_CANCEL_RATE: Field = Field {
    shift: 46,
    bits: 10,
};
const F_LOSS_RATE: Field = Field {
    shift: 56,
    bits: 10,
};
const F_JITTER: Field = Field {
    shift: 66,
    bits: 12,
};
const F_QUEUE_POSITION: Field = Field {
    shift: 78,
    bits: 12,
};
const F_FLAGS: Field = Field { shift: 90, bits: 8 };
const F_VERSION: Field = Field { shift: 98, bits: 8 };
const F_SEQUENCE: Field = Field {
    shift: 106,
    bits: 10,
};
const F_AGE: Field = Field {
    shift: 116,
    bits: 12,
};

/// Flags set by the latency sidecar when budgets are breached.
pub const FLAG_SLOW_ACK: u8 = 0b0000_0001;
/// Flag indicating fills are materially slower than the configured budget.
pub const FLAG_SLOW_FILL: u8 = 0b0000_0010;
/// Flag indicating latency dispersion (jitter) has exceeded the high-water mark.
pub const FLAG_HIGH_JITTER: u8 = 0b0000_0100;
/// Flag signalling packet loss is above acceptable limits.
pub const FLAG_HIGH_LOSS: u8 = 0b0000_1000;
/// Flag set when venue or infra applies rate limiting to the session.
pub const FLAG_RATE_LIMIT: u8 = 0b0001_0000;
/// Flag denoting exchange backpressure (e.g., long cancel ACKs, queue churn).
pub const FLAG_BACKPRESSURE: u8 = 0b0010_0000;
/// Flag instructing router to gate cancels to protect priority.
pub const FLAG_GATE_CANCEL: u8 = 0b0100_0000;
/// Spare flag available for custom wiring.
pub const FLAG_SPARE: u8 = 0b1000_0000;

const MAX_LAT_TICKS: u32 = (1 << F_FEED_TO_DECISION.bits) - 1;
const MAX_RATE_BPS: u32 = (1 << F_REJECT_RATE.bits) - 1;
const MAX_QUEUE_Q012: u32 = (1 << F_QUEUE_POSITION.bits) - 1;
const MAX_SEQ: u32 = (1 << F_SEQUENCE.bits) - 1;
const MAX_AGE_TICKS: u32 = (1 << F_AGE.bits) - 1;
const LAT_DIVISOR_US: u32 = 2;
const AGE_DIVISOR_MS: u32 = 8;

/// Saturating quantized latency units (µs / 2), rounded to nearest.
#[inline]
const fn quantize_latency_us_q2(value_us: u32) -> u16 {
    let rounded = (value_us + (LAT_DIVISOR_US / 2)) / LAT_DIVISOR_US;
    let capped = if rounded > MAX_LAT_TICKS {
        MAX_LAT_TICKS
    } else {
        rounded
    };
    capped as u16
}

/// Saturating rate quantizer for basis points.
#[inline]
const fn quantize_rate_bps(value_bps: u32) -> u16 {
    let capped = if value_bps > MAX_RATE_BPS {
        MAX_RATE_BPS
    } else {
        value_bps
    };
    capped as u16
}

/// Saturating queue percentile quantizer (Q0.12).
#[inline]
fn quantize_queue_position(percentile: f32) -> u16 {
    if percentile.is_nan() {
        return 0;
    }
    let clamped = percentile.clamp(0.0, 1.0);
    let scaled = clamped * (MAX_QUEUE_Q012 as f32);
    let rounded = (scaled + 0.5).min(MAX_QUEUE_Q012 as f32);
    rounded as u16
}

/// Saturating age quantizer (ms / 8).
#[inline]
const fn quantize_age_ms(value_ms: u32) -> u16 {
    let rounded = (value_ms + (AGE_DIVISOR_MS / 2)) / AGE_DIVISOR_MS;
    let capped = if rounded > MAX_AGE_TICKS {
        MAX_AGE_TICKS
    } else {
        rounded
    };
    capped as u16
}

/// Atomic container for the ALT-128 ticket.
#[repr(transparent)]
pub struct AltAtomic {
    inner: AtomicU128,
}

impl AltAtomic {
    /// Construct a zeroed ticket slot.
    pub const fn new() -> Self {
        Self {
            inner: AtomicU128::new(0),
        }
    }

    /// Store a packed ticket with the provided memory ordering.
    #[inline]
    pub fn store(&self, ticket: AltPacked, order: Ordering) {
        self.inner.store(ticket.0, order);
    }

    /// Publish a ticket with `Release` ordering.
    #[inline]
    pub fn store_release(&self, ticket: AltPacked) {
        self.store(ticket, Ordering::Release);
    }

    /// Publish from real-world samples with one `store(Release)`.
    #[inline]
    pub fn publish_sample(&self, sample: AltSample) {
        let quantized = AltQuantized::from_sample(sample);
        self.store_release(quantized.pack());
    }

    /// Load the packed ticket with the supplied ordering.
    #[inline]
    pub fn load(&self, order: Ordering) -> AltPacked {
        AltPacked(self.inner.load(order))
    }

    /// Hot-path helper: relaxed load of the packed ticket.
    #[inline]
    pub fn load_relaxed(&self) -> AltPacked {
        self.load(Ordering::Relaxed)
    }
}

impl Default for AltAtomic {
    fn default() -> Self {
        Self::new()
    }
}

/// Align the ticket on a cache line to avoid false sharing.
#[repr(C, align(64))]
pub struct AltSlot {
    ticket: AltAtomic,
}

impl AltSlot {
    /// Construct a cache-line aligned slot.
    pub const fn new() -> Self {
        Self {
            ticket: AltAtomic::new(),
        }
    }

    /// Borrow the inner atomic ticket.
    #[inline]
    pub fn ticket(&self) -> &AltAtomic {
        &self.ticket
    }
}

impl Default for AltSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Quantized representation of the ALT-128 layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AltQuantized {
    pub feed_to_decision_us2: u16,
    pub decision_to_ack_us2: u16,
    pub ack_to_fill_us2: u16,
    pub reject_rate_bps: u16,
    pub cancel_rate_bps: u16,
    pub loss_rate_bps: u16,
    pub jitter_us2: u16,
    pub queue_position_q012: u16,
    pub flags: u8,
    pub version: u8,
    pub sequence: u16,
    pub age_8ms: u16,
}

impl AltQuantized {
    /// Pack the quantized fields into the 128-bit ticket.
    #[inline]
    pub fn pack(self) -> AltPacked {
        let mut raw = 0u128;
        raw = set_field(raw, F_FEED_TO_DECISION, self.feed_to_decision_us2 as u128);
        raw = set_field(raw, F_DECISION_TO_ACK, self.decision_to_ack_us2 as u128);
        raw = set_field(raw, F_ACK_TO_FILL, self.ack_to_fill_us2 as u128);
        raw = set_field(raw, F_REJECT_RATE, self.reject_rate_bps as u128);
        raw = set_field(raw, F_CANCEL_RATE, self.cancel_rate_bps as u128);
        raw = set_field(raw, F_LOSS_RATE, self.loss_rate_bps as u128);
        raw = set_field(raw, F_JITTER, self.jitter_us2 as u128);
        raw = set_field(raw, F_QUEUE_POSITION, self.queue_position_q012 as u128);
        raw = set_field(raw, F_FLAGS, self.flags as u128);
        raw = set_field(raw, F_VERSION, self.version as u128);
        raw = set_field(raw, F_SEQUENCE, (self.sequence & MAX_SEQ as u16) as u128);
        raw = set_field(raw, F_AGE, self.age_8ms as u128);
        AltPacked(raw)
    }

    /// Saturating conversion from real-world samples.
    #[inline]
    pub fn from_sample(sample: AltSample) -> Self {
        Self {
            feed_to_decision_us2: quantize_latency_us_q2(sample.feed_to_decision_us),
            decision_to_ack_us2: quantize_latency_us_q2(sample.decision_to_ack_us),
            ack_to_fill_us2: quantize_latency_us_q2(sample.ack_to_first_fill_us),
            reject_rate_bps: quantize_rate_bps(sample.reject_rate_bps as u32),
            cancel_rate_bps: quantize_rate_bps(sample.cancel_rate_bps as u32),
            loss_rate_bps: quantize_rate_bps(sample.loss_rate_bps as u32),
            jitter_us2: quantize_latency_us_q2(sample.jitter_us),
            queue_position_q012: quantize_queue_position(sample.queue_position),
            flags: sample.flags,
            version: sample.version,
            sequence: cmp::min(sample.sequence, MAX_SEQ as u16),
            age_8ms: quantize_age_ms(sample.age_ms),
        }
    }

    /// Convert back into real units.
    #[inline]
    pub fn into_snapshot(self) -> AltSnapshot {
        AltSnapshot {
            feed_to_decision_us: (self.feed_to_decision_us2 as u32) * LAT_DIVISOR_US,
            decision_to_ack_us: (self.decision_to_ack_us2 as u32) * LAT_DIVISOR_US,
            ack_to_first_fill_us: (self.ack_to_fill_us2 as u32) * LAT_DIVISOR_US,
            reject_rate_bps: self.reject_rate_bps,
            cancel_rate_bps: self.cancel_rate_bps,
            loss_rate_bps: self.loss_rate_bps,
            jitter_us: (self.jitter_us2 as u32) * LAT_DIVISOR_US,
            queue_position: (self.queue_position_q012 as f32) / (MAX_QUEUE_Q012 as f32),
            flags: self.flags,
            version: self.version,
            sequence: (self.sequence & MAX_SEQ as u16),
            age_ms: (self.age_8ms as u32) * AGE_DIVISOR_MS,
        }
    }
}

impl From<AltSample> for AltQuantized {
    fn from(value: AltSample) -> Self {
        Self::from_sample(value)
    }
}

impl From<AltQuantized> for AltSample {
    fn from(value: AltQuantized) -> Self {
        value.into_snapshot()
    }
}

/// Physical-units view of the ticket.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AltSample {
    pub feed_to_decision_us: u32,
    pub decision_to_ack_us: u32,
    pub ack_to_first_fill_us: u32,
    pub reject_rate_bps: u16,
    pub cancel_rate_bps: u16,
    pub loss_rate_bps: u16,
    pub jitter_us: u32,
    pub queue_position: f32,
    pub flags: u8,
    pub version: u8,
    pub sequence: u16,
    pub age_ms: u32,
}

impl AltSample {
    /// Helper to evaluate snapshot staleness relative to a budget in ms.
    #[inline]
    pub fn is_stale(&self, max_age_ms: u32) -> bool {
        self.age_ms > max_age_ms
    }
}

/// Packed 128-bit ALT ticket.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AltPacked(u128);

impl AltPacked {
    /// Construct from the raw `u128` word.
    pub const fn from_raw(raw: u128) -> Self {
        Self(raw)
    }

    /// Obtain the packed raw word.
    pub const fn raw(self) -> u128 {
        self.0
    }

    /// Recover the quantized representation.
    #[inline]
    pub fn quantized(self) -> AltQuantized {
        AltQuantized {
            feed_to_decision_us2: get_field(self.0, F_FEED_TO_DECISION) as u16,
            decision_to_ack_us2: get_field(self.0, F_DECISION_TO_ACK) as u16,
            ack_to_fill_us2: get_field(self.0, F_ACK_TO_FILL) as u16,
            reject_rate_bps: get_field(self.0, F_REJECT_RATE) as u16,
            cancel_rate_bps: get_field(self.0, F_CANCEL_RATE) as u16,
            loss_rate_bps: get_field(self.0, F_LOSS_RATE) as u16,
            jitter_us2: get_field(self.0, F_JITTER) as u16,
            queue_position_q012: get_field(self.0, F_QUEUE_POSITION) as u16,
            flags: get_field(self.0, F_FLAGS) as u16 as u8,
            version: get_field(self.0, F_VERSION) as u16 as u8,
            sequence: get_field(self.0, F_SEQUENCE) as u16,
            age_8ms: get_field(self.0, F_AGE) as u16,
        }
    }

    /// Decode into physical units (`AltSnapshot`).
    #[inline]
    pub fn snapshot(self) -> AltSnapshot {
        self.quantized().into_snapshot()
    }

    /// Convenience getter for flags.
    #[inline]
    pub fn flags(self) -> u8 {
        get_field(self.0, F_FLAGS) as u8
    }
}

/// Decoded ALT ticket with physical units.
pub type AltSnapshot = AltSample;

const RATE_WINDOW: usize = 256;

fn round_to_u32(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else {
        let clamped = value.min(u32::MAX as f32);
        (clamped + 0.5) as u32
    }
}

fn round_to_u16(value: f32) -> u16 {
    if value <= 0.0 {
        0
    } else {
        let clamped = value.min(u16::MAX as f32);
        (clamped + 0.5) as u16
    }
}

#[derive(Clone, Copy)]
struct Ewma {
    value: f32,
    alpha: f32,
    initialized: bool,
}

impl Ewma {
    const fn new(alpha: f32) -> Self {
        Self {
            value: 0.0,
            alpha,
            initialized: false,
        }
    }

    fn reset(&mut self, value: f32) {
        self.value = value;
        self.initialized = true;
    }

    fn update(&mut self, sample: f32) -> f32 {
        if !self.initialized {
            self.reset(sample);
        } else {
            self.value += self.alpha * (sample - self.value);
        }
        self.value
    }

    fn value(&self) -> Option<f32> {
        if self.initialized {
            Some(self.value)
        } else {
            None
        }
    }
}

struct BudgetTracker {
    baseline: Ewma,
    deviation: Ewma,
    warmup_samples: u32,
    min_deviation: f32,
    sigma: f32,
    sample_count: u32,
    threshold: f32,
}

struct BudgetState {
    breached: bool,
    baseline: f32,
}

impl BudgetTracker {
    const fn new(alpha: f32, sigma: f32, warmup_samples: u32, min_deviation: f32) -> Self {
        Self {
            baseline: Ewma::new(alpha),
            deviation: Ewma::new(alpha * 0.5),
            warmup_samples,
            min_deviation,
            sigma,
            sample_count: 0,
            threshold: 0.0,
        }
    }

    fn observe(&mut self, sample: f32) -> BudgetState {
        self.sample_count = self.sample_count.saturating_add(1);
        let mean = self.baseline.update(sample);
        let delta = (sample - mean).abs();
        let dev = self.deviation.update(delta).max(self.min_deviation);
        let threshold = mean + self.sigma * dev;
        self.threshold = threshold;
        let calibrated = self.sample_count >= self.warmup_samples.max(1);
        let breached = calibrated && sample > threshold;
        BudgetState {
            breached,
            baseline: mean,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OrderOutcome {
    Acknowledged,
    Rejected,
    Cancelled,
}

struct RateWindow {
    outcomes: [u8; RATE_WINDOW],
    len: usize,
    index: usize,
    reject_count: u16,
    cancel_count: u16,
}

impl RateWindow {
    const fn new() -> Self {
        Self {
            outcomes: [0; RATE_WINDOW],
            len: 0,
            index: 0,
            reject_count: 0,
            cancel_count: 0,
        }
    }

    fn push(&mut self, outcome: OrderOutcome) {
        let code = match outcome {
            OrderOutcome::Acknowledged => 0,
            OrderOutcome::Rejected => 1,
            OrderOutcome::Cancelled => 2,
        };
        if self.len == RATE_WINDOW {
            let old = self.outcomes[self.index];
            match old {
                1 => self.reject_count = self.reject_count.saturating_sub(1),
                2 => self.cancel_count = self.cancel_count.saturating_sub(1),
                _ => {}
            }
        } else {
            self.len += 1;
        }
        self.outcomes[self.index] = code;
        match code {
            1 => self.reject_count = self.reject_count.saturating_add(1),
            2 => self.cancel_count = self.cancel_count.saturating_add(1),
            _ => {}
        }
        self.index = (self.index + 1) % RATE_WINDOW;
    }

    fn totals(&self) -> (u16, u16, u16) {
        let total = self.len as u16;
        (self.reject_count, self.cancel_count, total)
    }
}

/// Configuration for the ALT writer sidecar.
#[derive(Clone, Copy, Debug)]
pub struct AltWriterConfig {
    pub version: u8,
    pub latency_alpha: f32,
    pub jitter_alpha: f32,
    pub queue_alpha: f32,
    pub loss_alpha: f32,
    pub ack_sigma: f32,
    pub fill_sigma: f32,
    pub min_deviation_us: f32,
    pub warmup_samples: u32,
    pub jitter_flag_us: u32,
    pub loss_flag_bps: u32,
    pub cancel_backpressure_bps: u32,
}

impl Default for AltWriterConfig {
    fn default() -> Self {
        Self {
            version: 1,
            latency_alpha: 0.2,
            jitter_alpha: 0.2,
            queue_alpha: 0.15,
            loss_alpha: 0.1,
            ack_sigma: 3.0,
            fill_sigma: 3.0,
            min_deviation_us: 50.0,
            warmup_samples: 32,
            jitter_flag_us: 5_000,
            loss_flag_bps: 50,
            cancel_backpressure_bps: 300,
        }
    }
}

/// Writer-side helper that measures order path metrics and publishes ALT updates.
pub struct AltWriter<'a> {
    ticket: &'a AltAtomic,
    config: AltWriterConfig,
    feed_ewma: Ewma,
    ack_ewma: Ewma,
    fill_ewma: Ewma,
    jitter_ewma: Ewma,
    queue_ewma: Ewma,
    loss_ewma: Ewma,
    ack_budget: BudgetTracker,
    fill_budget: BudgetTracker,
    rates: RateWindow,
    slow_ack: bool,
    slow_fill: bool,
    rate_limited: bool,
    gate_cancel: bool,
    backpressure_override: bool,
    spare_flag: bool,
    seq: u16,
    last_publish_ns: Option<u64>,
    last_send_ns: Option<u64>,
    last_ack_ns: Option<u64>,
    pending_fill_latency: bool,
    last_d2a_us: u32,
    last_a2f_us: u32,
    last_f2d_us: u32,
}

impl<'a> AltWriter<'a> {
    /// Create a new writer bound to an `AltAtomic`.
    pub fn new(ticket: &'a AltAtomic, config: AltWriterConfig) -> Self {
        Self {
            ticket,
            config,
            feed_ewma: Ewma::new(config.latency_alpha),
            ack_ewma: Ewma::new(config.latency_alpha),
            fill_ewma: Ewma::new(config.latency_alpha),
            jitter_ewma: Ewma::new(config.jitter_alpha),
            queue_ewma: Ewma::new(config.queue_alpha),
            loss_ewma: Ewma::new(config.loss_alpha),
            ack_budget: BudgetTracker::new(
                config.latency_alpha,
                config.ack_sigma,
                config.warmup_samples,
                config.min_deviation_us,
            ),
            fill_budget: BudgetTracker::new(
                config.latency_alpha,
                config.fill_sigma,
                config.warmup_samples,
                config.min_deviation_us,
            ),
            rates: RateWindow::new(),
            slow_ack: false,
            slow_fill: false,
            rate_limited: false,
            gate_cancel: false,
            backpressure_override: false,
            spare_flag: false,
            seq: 0,
            last_publish_ns: None,
            last_send_ns: None,
            last_ack_ns: None,
            pending_fill_latency: false,
            last_d2a_us: 0,
            last_a2f_us: 0,
            last_f2d_us: 0,
        }
    }

    /// Convenience constructor from an `AltSlot`.
    pub fn from_slot(slot: &'a AltSlot, config: AltWriterConfig) -> Self {
        Self::new(slot.ticket(), config)
    }

    /// Record the time from feed event to decision phase (in microseconds).
    pub fn record_feed_to_decision(&mut self, latency_us: u32) {
        let sample = latency_us as f32;
        let value = self.feed_ewma.update(sample).max(0.0);
        self.last_f2d_us = round_to_u32(value);
    }

    /// Record that an order was sent at `now_ns`.
    pub fn on_order_send(&mut self, now_ns: u64) {
        self.last_send_ns = Some(now_ns);
    }

    /// Record that an order was acknowledged at `now_ns`.
    pub fn on_order_ack(&mut self, now_ns: u64) {
        if let Some(sent) = self.last_send_ns {
            let latency_us = ((now_ns.saturating_sub(sent)) / 1_000) as u32;
            let sample = latency_us as f32;
            let state = self.ack_budget.observe(sample);
            self.slow_ack = state.breached;
            self.ack_ewma.update(sample);
            let jitter_sample = (sample - state.baseline).abs();
            self.jitter_ewma.update(jitter_sample);
            self.last_d2a_us = latency_us;
        }
        self.last_ack_ns = Some(now_ns);
        self.pending_fill_latency = true;
        self.record_outcome(OrderOutcome::Acknowledged);
    }

    /// Record a rejected order outcome for the sliding window.
    pub fn on_order_reject(&mut self) {
        self.record_outcome(OrderOutcome::Rejected);
    }

    /// Record a cancelled order outcome for the sliding window.
    pub fn on_order_cancel(&mut self) {
        self.record_outcome(OrderOutcome::Cancelled);
    }

    /// Record the first fill latency at `now_ns`.
    pub fn on_first_fill(&mut self, now_ns: u64) {
        if self.pending_fill_latency {
            if let Some(ack_ts) = self.last_ack_ns {
                let latency_us = ((now_ns.saturating_sub(ack_ts)) / 1_000) as u32;
                let sample = latency_us as f32;
                let state = self.fill_budget.observe(sample);
                self.slow_fill = state.breached;
                self.fill_ewma.update(sample);
                self.last_a2f_us = latency_us;
            }
            self.pending_fill_latency = false;
        }
    }

    /// Record the observed outcome for an order (ack, reject, cancel).
    pub fn record_outcome(&mut self, outcome: OrderOutcome) {
        self.rates.push(outcome);
    }

    /// Update the loss estimate (basis points) from connectivity probes.
    pub fn record_loss_bps(&mut self, loss_bps: u32) {
        let sample = loss_bps as f32;
        self.loss_ewma.update(sample);
    }

    /// Update the queue percentile estimate (0.0 – 1.0).
    pub fn update_queue_position(&mut self, percentile: f32) {
        let clamped = if percentile.is_nan() {
            0.0
        } else {
            percentile.clamp(0.0, 1.0)
        };
        self.queue_ewma.update(clamped);
    }

    /// Set or clear the rate-limit flag.
    pub fn set_rate_limited(&mut self, enabled: bool) {
        self.rate_limited = enabled;
    }

    /// Set or clear the cancel gating flag.
    pub fn set_gate_cancel(&mut self, enabled: bool) {
        self.gate_cancel = enabled;
    }

    /// Set or clear the backpressure flag supplied externally.
    pub fn set_backpressure_override(&mut self, enabled: bool) {
        self.backpressure_override = enabled;
    }

    /// Set the spare flag bit.
    pub fn set_spare_flag(&mut self, enabled: bool) {
        self.spare_flag = enabled;
    }

    fn current_flags(&self, cancel_rate_bps: u16, jitter_us: u32, loss_bps: u16) -> u8 {
        let backpressure = self.backpressure_override
            || u32::from(cancel_rate_bps) > self.config.cancel_backpressure_bps;
        let mut flags = 0u8;
        if self.slow_ack {
            flags |= FLAG_SLOW_ACK;
        }
        if self.slow_fill {
            flags |= FLAG_SLOW_FILL;
        }
        if jitter_us > self.config.jitter_flag_us {
            flags |= FLAG_HIGH_JITTER;
        }
        if u32::from(loss_bps) > self.config.loss_flag_bps {
            flags |= FLAG_HIGH_LOSS;
        }
        if self.rate_limited {
            flags |= FLAG_RATE_LIMIT;
        }
        if backpressure {
            flags |= FLAG_BACKPRESSURE;
        }
        if self.gate_cancel {
            flags |= FLAG_GATE_CANCEL;
        }
        if self.spare_flag {
            flags |= FLAG_SPARE;
        }
        flags
    }

    fn ewma_value_or_zero(ewma: &Ewma) -> u32 {
        ewma.value().map(|v| round_to_u32(v.max(0.0))).unwrap_or(0)
    }

    fn jitter_value(&self) -> u32 {
        self.jitter_ewma
            .value()
            .map(|v| round_to_u32(v.max(0.0)))
            .unwrap_or(0)
    }

    fn queue_value(&self) -> f32 {
        self.queue_ewma
            .value()
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or(0.0)
    }

    fn loss_value(&self) -> u16 {
        self.loss_ewma
            .value()
            .map(|v| round_to_u16(v.max(0.0)))
            .unwrap_or(0)
    }

    /// Publish the current telemetry view and reset the age counter.
    pub fn publish(&mut self, now_ns: u64) -> AltSnapshot {
        let prev_ns = self.last_publish_ns;
        let age_ms = prev_ns
            .map(|last| ((now_ns.saturating_sub(last)) / 1_000_000) as u32)
            .unwrap_or(0);

        let (reject_count, cancel_count, total) = self.rates.totals();
        let total = total.max(1); // avoid division by zero
        let reject_rate_bps = ((reject_count as u32) * 10_000 / total as u32) as u16;
        let cancel_rate_bps = ((cancel_count as u32) * 10_000 / total as u32) as u16;
        let feed_us = Self::ewma_value_or_zero(&self.feed_ewma).max(self.last_f2d_us);
        let d2a_us = Self::ewma_value_or_zero(&self.ack_ewma).max(self.last_d2a_us);
        let a2f_us = Self::ewma_value_or_zero(&self.fill_ewma).max(self.last_a2f_us);
        let jitter_us = self.jitter_value();
        let queue = self.queue_value();
        let loss_bps = self.loss_value();
        let flags = self.current_flags(cancel_rate_bps, jitter_us, loss_bps);

        let sample = AltSample {
            feed_to_decision_us: feed_us,
            decision_to_ack_us: d2a_us,
            ack_to_first_fill_us: a2f_us,
            reject_rate_bps,
            cancel_rate_bps,
            loss_rate_bps: loss_bps,
            jitter_us,
            queue_position: queue,
            flags,
            version: self.config.version,
            sequence: self.seq,
            age_ms,
        };

        self.ticket.publish_sample(sample);
        self.last_publish_ns = Some(now_ns);
        self.seq = (self.seq + 1) & (MAX_SEQ as u16);
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn quantized_round_trip_extremes() {
        let max = AltQuantized {
            feed_to_decision_us2: MAX_LAT_TICKS as u16,
            decision_to_ack_us2: MAX_LAT_TICKS as u16,
            ack_to_fill_us2: MAX_LAT_TICKS as u16,
            reject_rate_bps: MAX_RATE_BPS as u16,
            cancel_rate_bps: MAX_RATE_BPS as u16,
            loss_rate_bps: MAX_RATE_BPS as u16,
            jitter_us2: MAX_LAT_TICKS as u16,
            queue_position_q012: MAX_QUEUE_Q012 as u16,
            flags: 0xAA,
            version: 0x42,
            sequence: MAX_SEQ as u16,
            age_8ms: MAX_AGE_TICKS as u16,
        };
        let packed = max.pack();
        assert_eq!(packed.quantized(), max);

        let zero = AltQuantized::default();
        let packed_zero = zero.pack();
        assert_eq!(packed_zero.quantized(), zero);
    }

    proptest! {
        #[test]
        fn prop_pack_unpack(q in any::<u16>(),
                             q2 in any::<u16>(),
                             q3 in any::<u16>(),
                             rate1 in any::<u16>(),
                             rate2 in any::<u16>(),
                             rate3 in any::<u16>(),
                             jitter in any::<u16>(),
                             queue in any::<u16>(),
                             flags in any::<u8>(),
                             version in any::<u8>(),
                             seq in any::<u16>(),
                             age in any::<u16>()) {
            let quantized = AltQuantized {
                feed_to_decision_us2: (q % (MAX_LAT_TICKS as u16 + 1)),
                decision_to_ack_us2: (q2 % (MAX_LAT_TICKS as u16 + 1)),
                ack_to_fill_us2: (q3 % (MAX_LAT_TICKS as u16 + 1)),
                reject_rate_bps: (rate1 % (MAX_RATE_BPS as u16 + 1)),
                cancel_rate_bps: (rate2 % (MAX_RATE_BPS as u16 + 1)),
                loss_rate_bps: (rate3 % (MAX_RATE_BPS as u16 + 1)),
                jitter_us2: (jitter % (MAX_LAT_TICKS as u16 + 1)),
                queue_position_q012: (queue % (MAX_QUEUE_Q012 as u16 + 1)),
                flags,
                version,
                sequence: (seq % (MAX_SEQ as u16 + 1)),
                age_8ms: (age % (MAX_AGE_TICKS as u16 + 1)),
            };
            let packed = quantized.pack();
            prop_assert_eq!(packed.quantized(), quantized);
        }
    }

    #[test]
    fn sample_quantization_saturates() {
        let sample = AltSample {
            feed_to_decision_us: 20_000,
            decision_to_ack_us: 20_000,
            ack_to_first_fill_us: 20_000,
            reject_rate_bps: 4_000,
            cancel_rate_bps: 4_000,
            loss_rate_bps: 4_000,
            jitter_us: 20_000,
            queue_position: 1.2,
            flags: 0xFF,
            version: 1,
            sequence: 9_999,
            age_ms: 100_000,
        };
        let q = AltQuantized::from_sample(sample);
        assert_eq!(q.feed_to_decision_us2, MAX_LAT_TICKS as u16);
        assert_eq!(q.reject_rate_bps, MAX_RATE_BPS as u16);
        assert_eq!(q.queue_position_q012, MAX_QUEUE_Q012 as u16);
        assert_eq!(q.sequence, MAX_SEQ as u16);
        assert_eq!(q.age_8ms, MAX_AGE_TICKS as u16);
    }

    #[test]
    fn snapshot_units_match() {
        let sample = AltSample {
            feed_to_decision_us: 750,
            decision_to_ack_us: 3_250,
            ack_to_first_fill_us: 10_001,
            reject_rate_bps: 123,
            cancel_rate_bps: 200,
            loss_rate_bps: 3,
            jitter_us: 1_501,
            queue_position: 0.333,
            flags: FLAG_HIGH_LOSS,
            version: 7,
            sequence: 321,
            age_ms: 612,
        };
        let packed = AltQuantized::from_sample(sample).pack();
        let snapshot = packed.snapshot();
        assert!(snapshot.feed_to_decision_us >= 748 && snapshot.feed_to_decision_us <= 752);
        assert_eq!(snapshot.reject_rate_bps, 123);
        assert_eq!(snapshot.flags, FLAG_HIGH_LOSS);
        assert_eq!(snapshot.version, 7);
        assert_eq!(snapshot.sequence, 321);
        assert!((snapshot.queue_position - sample.queue_position).abs() < 0.0015);
        assert!(snapshot.age_ms >= 608 && snapshot.age_ms <= 616);
    }

    #[test]
    fn atomic_store_and_load() {
        let slot = AltSlot::new();
        let sample = AltSample {
            feed_to_decision_us: 500,
            decision_to_ack_us: 1_500,
            ack_to_first_fill_us: 2_500,
            reject_rate_bps: 10,
            cancel_rate_bps: 20,
            loss_rate_bps: 5,
            jitter_us: 800,
            queue_position: 0.4,
            flags: FLAG_SLOW_ACK,
            version: 3,
            sequence: 17,
            age_ms: 128,
        };
        slot.ticket().publish_sample(sample);
        let snapshot = slot.ticket().load_relaxed().snapshot();
        assert_eq!(snapshot.version, 3);
        assert_eq!(snapshot.flags, FLAG_SLOW_ACK);
        assert!(!snapshot.is_stale(200));
    }

    #[test]
    fn writer_basic_flow_publishes_snapshot() {
        let slot = AltSlot::new();
        let mut config = AltWriterConfig::default();
        config.warmup_samples = 3;
        config.jitter_flag_us = u32::MAX;
        config.loss_flag_bps = u32::MAX;
        config.cancel_backpressure_bps = u32::MAX;
        let mut writer = AltWriter::from_slot(&slot, config);

        writer.record_feed_to_decision(600);
        writer.on_order_send(1_000_000);
        writer.on_order_ack(4_000_000);
        writer.on_first_fill(6_500_000);
        writer.on_order_reject();
        writer.on_order_cancel();
        writer.update_queue_position(0.55);
        writer.record_loss_bps(10);

        let snapshot = writer.publish(10_000_000);
        assert!(snapshot.decision_to_ack_us > 0);
        assert_eq!(snapshot.version, config.version);
        assert!(snapshot.reject_rate_bps > 0);
        assert!(snapshot.cancel_rate_bps > 0);
        assert_eq!(
            snapshot.flags & (FLAG_SLOW_ACK | FLAG_SLOW_FILL | FLAG_HIGH_LOSS),
            0
        );
    }

    #[test]
    fn writer_flags_slow_ack_after_warmup() {
        let slot = AltSlot::new();
        let mut config = AltWriterConfig::default();
        config.warmup_samples = 2;
        config.jitter_flag_us = u32::MAX;
        config.loss_flag_bps = u32::MAX;
        config.cancel_backpressure_bps = u32::MAX;
        let mut writer = AltWriter::from_slot(&slot, config);

        writer.on_order_send(0);
        writer.on_order_ack(2_000_000);
        writer.publish(2_000_000);

        writer.on_order_send(3_000_000);
        writer.on_order_ack(5_000_000);
        writer.publish(5_000_000);

        writer.on_order_send(6_000_000);
        writer.on_order_ack(16_000_000);
        let snapshot = writer.publish(16_000_000);
        assert!(snapshot.decision_to_ack_us >= 10_000);
        assert_ne!(snapshot.flags & FLAG_SLOW_ACK, 0);
    }

    #[test]
    fn writer_sets_backpressure_flag_on_high_cancel_rate() {
        let slot = AltSlot::new();
        let mut config = AltWriterConfig::default();
        config.cancel_backpressure_bps = 2_000; // 20%
        config.loss_flag_bps = u32::MAX;
        config.jitter_flag_us = u32::MAX;
        let mut writer = AltWriter::from_slot(&slot, config);

        for _ in 0..10 {
            writer.on_order_cancel();
        }
        let snapshot = writer.publish(1_000_000);
        assert_ne!(snapshot.flags & FLAG_BACKPRESSURE, 0);
    }

    #[test]
    fn writer_sets_loss_flag_when_loss_exceeds_threshold() {
        let slot = AltSlot::new();
        let mut config = AltWriterConfig::default();
        config.loss_flag_bps = 25;
        config.cancel_backpressure_bps = u32::MAX;
        config.jitter_flag_us = u32::MAX;
        let mut writer = AltWriter::from_slot(&slot, config);

        writer.record_loss_bps(100);
        let snapshot = writer.publish(1_000_000);
        assert_ne!(snapshot.flags & FLAG_HIGH_LOSS, 0);
    }
}
