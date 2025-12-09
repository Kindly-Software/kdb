use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU64, Ordering};

/// Telemetry capsule for combat events and counters.
///
/// - Alignment: 128B, size 256B (expanded for AI counters).
/// - Purpose: keep hot counters lockfree and snapshot-friendly.
#[repr(C, align(128))]
pub struct TelemetryCapsule {
    events: AtomicU64,
    casualties: AtomicU64,
    shock_weight_q16: AtomicU64,
    ammo_spent: AtomicU64,
    tick_last_flush: AtomicU64,
    retreats: AtomicU64,
    musket_shots: AtomicU64,
    artillery_shots: AtomicU64,
    formation_breaks: AtomicU64,
    morale_shocks: AtomicU64,
    supply_pressure_accum_q16: AtomicU64,
    supply_fatigue_accum_q16: AtomicU64,
    supply_samples: AtomicU64,
    ai_orders: AtomicU64,
    charge_orders: AtomicU64,
    charge_commits: AtomicU64,
    brace_orders: AtomicU64,
    _padding: [u8; 0],
}

impl TelemetryCapsule {
    pub const fn new() -> Self {
        Self {
            events: AtomicU64::new(0),
            casualties: AtomicU64::new(0),
            ammo_spent: AtomicU64::new(0),
            tick_last_flush: AtomicU64::new(0),
            retreats: AtomicU64::new(0),
            musket_shots: AtomicU64::new(0),
            artillery_shots: AtomicU64::new(0),
            formation_breaks: AtomicU64::new(0),
            morale_shocks: AtomicU64::new(0),
            supply_pressure_accum_q16: AtomicU64::new(0),
            supply_fatigue_accum_q16: AtomicU64::new(0),
            supply_samples: AtomicU64::new(0),
            ai_orders: AtomicU64::new(0),
            shock_weight_q16: AtomicU64::new(0),
            charge_orders: AtomicU64::new(0),
            charge_commits: AtomicU64::new(0),
            brace_orders: AtomicU64::new(0),
            _padding: [],
        }
    }

    pub fn log_event(&self) -> u64 {
        self.events.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn add_casualties(&self, count: u32) {
        self.casualties.fetch_add(count as u64, Ordering::AcqRel);
    }

    fn add_shock_weight(&self, weight_q16: u32) {
        if weight_q16 > 0 {
            self.shock_weight_q16
                .fetch_add(weight_q16 as u64, Ordering::AcqRel);
        }
    }

    pub fn add_ammo_spent(&self, amount: u32) {
        self.ammo_spent.fetch_add(amount as u64, Ordering::AcqRel);
    }

    pub fn mark_flush(&self, tick: u64) {
        self.tick_last_flush.store(tick, Ordering::Release);
    }

    pub fn record_retreat(&self) {
        self.retreats.fetch_add(1, Ordering::AcqRel);
    }

    pub fn log_musket_fire(&self, count: u32) {
        self.musket_shots.fetch_add(count as u64, Ordering::AcqRel);
    }

    pub fn log_musket_shock(&self, casualties: u32) {
        self.log_casualty_shock(casualties);
        // Musket shocks: lighter than artillery; keep a modest additive weight.
        let musket_weight_q16 = (casualties as u64)
            .saturating_mul(65_536)
            .min(u32::MAX as u64) as u32;
        self.add_shock_weight(musket_weight_q16);
    }

    pub fn log_artillery_fire(&self, count: u32) {
        self.artillery_shots
            .fetch_add(count as u64, Ordering::AcqRel);
    }

    pub fn log_artillery_shock(&self, casualties: u32, volley: u16) {
        self.log_casualty_shock(casualties);
        // Artillery shocks: heavier casualty weight plus a per-volley fear spike.
        let casualty_weight_q16 = (casualties as u64)
            .saturating_mul(3 * 65_536)
            .min(u32::MAX as u64);
        let volley_bonus_q16 = ((volley as u64 + 1) * 24_576).min(180_000);
        let total_q16 = casualty_weight_q16
            .saturating_add(volley_bonus_q16)
            .min(u32::MAX as u64) as u32;
        self.add_shock_weight(total_q16);
    }

    /// Fire-control telemetry: account volley count and note dispersion in ammo_spent as proxy.
    pub fn log_fire_control(&self, volley: u16, dispersion_mils: u16) {
        self.artillery_shots
            .fetch_add(volley as u64, Ordering::AcqRel);
        self.ammo_spent.fetch_add(volley as u64, Ordering::AcqRel);
        // Track peak dispersion; useful for post-battle diagnostics.
        self.tick_last_flush
            .fetch_max(dispersion_mils as u64, Ordering::Relaxed);
    }

    /// Casualty batch logging plus morale shock notification (used by artillery impacts).
    pub fn log_casualty_shock(&self, casualties: u32) {
        if casualties == 0 {
            return;
        }
        self.add_casualties(casualties);
        self.log_morale_shock();
        let base_weight_q16 = (casualties as u64)
            .saturating_mul(2 * 65_536)
            .min(u32::MAX as u64) as u32;
        self.add_shock_weight(base_weight_q16);
    }

    /// Charge (bayonet/cavalry) shock logging: combine casualties and impulse weight.
    pub fn log_charge_shock(&self, casualties: u32, impulse_q16: u32) {
        self.log_casualty_shock(casualties);
        self.add_shock_weight(impulse_q16);
    }

    /// Count charge orders for replay determinism; commits are tracked separately.
    pub fn log_charge_order(&self, commit: bool) {
        self.charge_orders.fetch_add(1, Ordering::AcqRel);
        if commit {
            self.charge_commits.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Count brace orders for replay determinism.
    pub fn log_brace_order(&self) {
        self.brace_orders.fetch_add(1, Ordering::AcqRel);
    }

    pub fn log_morale_shock(&self) {
        self.morale_shocks.fetch_add(1, Ordering::AcqRel);
    }

    pub fn log_formation_break(&self) {
        self.formation_breaks.fetch_add(1, Ordering::AcqRel);
    }

    /// Record per-tick supply averages for replay/analytics.
    pub fn log_supply_stats(&self, pressure_avg_q16: u32, fatigue_avg_q16: u32, samples: u32) {
        if samples == 0 {
            return;
        }
        self.supply_pressure_accum_q16
            .fetch_add(pressure_avg_q16 as u64, Ordering::AcqRel);
        self.supply_fatigue_accum_q16
            .fetch_add(fatigue_avg_q16 as u64, Ordering::AcqRel);
        self.supply_samples
            .fetch_add(samples as u64, Ordering::AcqRel);
    }

    /// Count AI-issued orders for analytics/guardrails.
    pub fn log_ai_orders(&self, count: u32) {
        if count == 0 {
            return;
        }
        self.ai_orders.fetch_add(count as u64, Ordering::AcqRel);
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            events: self.events.load(Ordering::Relaxed),
            casualties: self.casualties.load(Ordering::Relaxed),
            ammo_spent: self.ammo_spent.load(Ordering::Relaxed),
            tick_last_flush: self.tick_last_flush.load(Ordering::Relaxed),
            retreats: self.retreats.load(Ordering::Relaxed),
            musket_shots: self.musket_shots.load(Ordering::Relaxed),
            artillery_shots: self.artillery_shots.load(Ordering::Relaxed),
            formation_breaks: self.formation_breaks.load(Ordering::Relaxed),
            morale_shocks: self.morale_shocks.load(Ordering::Relaxed),
            shock_weight_q16: self.shock_weight_q16.load(Ordering::Relaxed),
            supply_pressure_accum_q16: self.supply_pressure_accum_q16.load(Ordering::Relaxed),
            supply_fatigue_accum_q16: self.supply_fatigue_accum_q16.load(Ordering::Relaxed),
            supply_samples: self.supply_samples.load(Ordering::Relaxed),
            ai_orders: self.ai_orders.load(Ordering::Relaxed),
            charge_orders: self.charge_orders.load(Ordering::Relaxed),
            charge_commits: self.charge_commits.load(Ordering::Relaxed),
            brace_orders: self.brace_orders.load(Ordering::Relaxed),
        }
    }
}

verify_capsule_properties!(TelemetryCapsule, 128, 256);

#[derive(Debug, Clone, Copy)]
pub struct TelemetrySnapshot {
    pub events: u64,
    pub casualties: u64,
    pub ammo_spent: u64,
    pub tick_last_flush: u64,
    pub retreats: u64,
    pub musket_shots: u64,
    pub artillery_shots: u64,
    pub formation_breaks: u64,
    pub morale_shocks: u64,
    pub shock_weight_q16: u64,
    pub supply_pressure_accum_q16: u64,
    pub supply_fatigue_accum_q16: u64,
    pub supply_samples: u64,
    pub ai_orders: u64,
    pub charge_orders: u64,
    pub charge_commits: u64,
    pub brace_orders: u64,
}

/// Per-formation break counter capsule (fixed capacity).
#[repr(C, align(128))]
pub struct FormationBreakTelemetryCapsule<const N: usize> {
    breaks: [AtomicU64; N],
}

impl<const N: usize> FormationBreakTelemetryCapsule<N> {
    pub fn new() -> Self {
        Self {
            breaks: core::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    /// Increment break count for a formation index (if in range).
    pub fn record(&self, formation_idx: usize) {
        if let Some(slot) = self.breaks.get(formation_idx) {
            slot.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn snapshot(&self) -> [u64; N] {
        let mut out = [0u64; N];
        for (i, slot) in self.breaks.iter().enumerate() {
            out[i] = slot.load(Ordering::Relaxed);
        }
        out
    }
}

verify_alignment_only!(FormationBreakTelemetryCapsule<4>, 128);

/// Wider-capacity aliases for large formations.
pub type FormationBreaks256 = FormationBreakTelemetryCapsule<256>;
pub type FormationBreaks1024 = FormationBreakTelemetryCapsule<1024>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_counts() {
        let t = TelemetryCapsule::new();
        assert_eq!(t.log_event(), 1);
        t.add_casualties(5);
        t.add_ammo_spent(30);
        t.mark_flush(10);
        t.record_retreat();
        t.log_musket_fire(12);
        t.log_artillery_fire(3);
        t.log_formation_break();
        t.log_morale_shock();
        t.log_musket_shock(4);
        t.log_artillery_shock(5, 2);
        t.log_charge_order(true);
        t.log_charge_order(false);
        t.log_brace_order();
        t.log_ai_orders(3);
        let snap = t.snapshot();
        assert_eq!(snap.events, 1);
        assert_eq!(snap.casualties, 14);
        assert_eq!(snap.ammo_spent, 30);
        assert_eq!(snap.tick_last_flush, 10);
        assert_eq!(snap.retreats, 1);
        assert_eq!(snap.musket_shots, 12);
        assert_eq!(snap.artillery_shots, 3);
        assert_eq!(snap.formation_breaks, 1);
        assert!(snap.morale_shocks >= 3); // base + musket + artillery shocks
        assert!(snap.shock_weight_q16 > 0);
        assert_eq!(snap.charge_orders, 2);
        assert_eq!(snap.charge_commits, 1);
        assert_eq!(snap.brace_orders, 1);
        assert_eq!(snap.ai_orders, 3);
    }

    #[test]
    fn formation_break_counters() {
        let breaks: FormationBreakTelemetryCapsule<3> = FormationBreakTelemetryCapsule::new();
        breaks.record(0);
        breaks.record(2);
        breaks.record(2);
        let snap = breaks.snapshot();
        assert_eq!(snap, [1, 0, 2]);
    }
}
