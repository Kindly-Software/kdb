use crate::estimator::Route;
use crate::layout::ActSnapshot;

use serde::Serialize;
use std::collections::HashMap;

/// Key for telemetry aggregation buckets.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub struct TelemetryKey {
    pub symbol: String,
    pub route: Route,
    pub size_bucket: u8,
}

impl TelemetryKey {
    pub fn new(symbol: impl Into<String>, route: Route, size_bucket: u8) -> Self {
        Self {
            symbol: symbol.into(),
            route,
            size_bucket,
        }
    }
}

/// Aggregated stats for ACT snapshots in basis points.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SnapshotStats {
    pub samples: u64,
    pub ok_samples: u64,
    pub gross_bp_sum: f64,
    pub fees_bp_sum: f64,
    pub slip_bp_sum: f64,
    pub net_bp_sum: f64,
}

impl SnapshotStats {
    pub fn record(&mut self, snapshot: &ActSnapshot) {
        self.samples += 1;
        if snapshot.flags.contains(crate::layout::ActFlags::OK) {
            self.ok_samples += 1;
        }
        self.gross_bp_sum += snapshot.gross.to_bp();
        self.fees_bp_sum += snapshot.fees.to_bp();
        self.slip_bp_sum += snapshot.slip.to_bp();
        self.net_bp_sum += snapshot.net.to_bp();
    }

    pub fn ok_rate(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.ok_samples as f64 / self.samples as f64
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples += other.samples;
        self.ok_samples += other.ok_samples;
        self.gross_bp_sum += other.gross_bp_sum;
        self.fees_bp_sum += other.fees_bp_sum;
        self.slip_bp_sum += other.slip_bp_sum;
        self.net_bp_sum += other.net_bp_sum;
    }
}

/// Aggregated stats for realized fills vs expected slip.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FillStats {
    pub samples: u64,
    pub predicted_slip_bp_sum: f64,
    pub realized_slip_bp_sum: f64,
    pub abs_error_bp_sum: f64,
}

impl FillStats {
    pub fn record(&mut self, predicted_slip_bp: f64, realized_slip_bp: f64) {
        self.samples += 1;
        self.predicted_slip_bp_sum += predicted_slip_bp;
        self.realized_slip_bp_sum += realized_slip_bp;
        self.abs_error_bp_sum += (realized_slip_bp - predicted_slip_bp).abs();
    }

    pub fn mean_abs_error(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.abs_error_bp_sum / self.samples as f64
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples += other.samples;
        self.predicted_slip_bp_sum += other.predicted_slip_bp_sum;
        self.realized_slip_bp_sum += other.realized_slip_bp_sum;
        self.abs_error_bp_sum += other.abs_error_bp_sum;
    }
}

/// Telemetry recorder tracking snapshot and fill statistics.
#[derive(Clone, Debug, Default)]
pub struct ActTelemetry {
    snapshot_stats: HashMap<TelemetryKey, SnapshotStats>,
    fill_stats: HashMap<TelemetryKey, FillStats>,
}

impl ActTelemetry {
    pub fn record_snapshot(&mut self, key: TelemetryKey, snapshot: &ActSnapshot) {
        self.snapshot_stats.entry(key).or_default().record(snapshot);
    }

    pub fn record_fill(
        &mut self,
        key: TelemetryKey,
        predicted_slip_bp: f64,
        realized_slip_bp: f64,
    ) {
        self.fill_stats
            .entry(key)
            .or_default()
            .record(predicted_slip_bp, realized_slip_bp);
    }

    pub fn merge(&mut self, other: &ActTelemetry) {
        for (key, stats) in &other.snapshot_stats {
            self.snapshot_stats
                .entry(key.clone())
                .or_default()
                .merge(stats);
        }
        for (key, stats) in &other.fill_stats {
            self.fill_stats.entry(key.clone()).or_default().merge(stats);
        }
    }

    pub fn snapshot_stats(&self) -> &HashMap<TelemetryKey, SnapshotStats> {
        &self.snapshot_stats
    }

    pub fn fill_stats(&self) -> &HashMap<TelemetryKey, FillStats> {
        &self.fill_stats
    }

    pub fn to_report(&self) -> TelemetryReport {
        let snapshots = self
            .snapshot_stats
            .iter()
            .map(|(key, stats)| TelemetrySnapshotEntry {
                key: key.clone(),
                stats: stats.clone(),
            })
            .collect();
        let fills = self
            .fill_stats
            .iter()
            .map(|(key, stats)| TelemetryFillEntry {
                key: key.clone(),
                stats: stats.clone(),
            })
            .collect();
        TelemetryReport { snapshots, fills }
    }

    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.to_report())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TelemetrySnapshotEntry {
    pub key: TelemetryKey,
    pub stats: SnapshotStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct TelemetryFillEntry {
    pub key: TelemetryKey,
    pub stats: FillStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct TelemetryReport {
    pub snapshots: Vec<TelemetrySnapshotEntry>,
    pub fills: Vec<TelemetryFillEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ActFlags, ActSnapshot, FixedQ8_8};

    fn sample_snapshot(ok: bool) -> ActSnapshot {
        ActSnapshot {
            gross: FixedQ8_8::saturating_from_bp(3.0),
            fees: FixedQ8_8::saturating_from_bp(1.0),
            slip: FixedQ8_8::saturating_from_bp(0.5),
            net: FixedQ8_8::saturating_from_bp(if ok { 2.0 } else { 0.5 }),
            min_required: FixedQ8_8::saturating_from_bp(1.5),
            sigma: FixedQ8_8::saturating_from_bp(0.2),
            flags: if ok { ActFlags::OK } else { ActFlags::empty() },
            version: 1,
            seq: 1,
            age_ms_bucket: 0,
        }
    }

    #[test]
    fn records_snapshot_stats() {
        let mut telemetry = ActTelemetry::default();
        let key = TelemetryKey::new("MES", Route::Maker, 0);
        telemetry.record_snapshot(key.clone(), &sample_snapshot(true));
        telemetry.record_snapshot(key.clone(), &sample_snapshot(false));

        let stats = telemetry.snapshot_stats().get(&key).unwrap();
        assert_eq!(stats.samples, 2);
        assert_eq!(stats.ok_samples, 1);
        assert!((stats.ok_rate() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn records_fill_stats() {
        let mut telemetry = ActTelemetry::default();
        let key = TelemetryKey::new("MES", Route::Taker, 1);
        telemetry.record_fill(key.clone(), 0.6, 1.2);
        telemetry.record_fill(key.clone(), 0.4, 0.2);

        let stats = telemetry.fill_stats().get(&key).unwrap();
        assert_eq!(stats.samples, 2);
        assert!((stats.mean_abs_error() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn exports_json() {
        let mut telemetry = ActTelemetry::default();
        let key = TelemetryKey::new("MES", Route::Maker, 0);
        telemetry.record_snapshot(key.clone(), &sample_snapshot(true));
        let json = telemetry.to_json_pretty().unwrap();
        assert!(json.contains("\"symbol\": \"MES\""));
    }
}
