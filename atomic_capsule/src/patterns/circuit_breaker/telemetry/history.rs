//! Sliding-window telemetry history for adaptive tuning.
//!
//! This module is gated behind the `auto_tune` feature to keep the hot path slim when
//! adaptive controllers are not required.

use crate::patterns::circuit_breaker::breaker::{MetricsSnapshot, State};
use crate::patterns::circuit_breaker::telemetry::{ActionOutcome, TelemetrySample};
use std::io::{self, Write};

/// Recorded outcome for a breaker evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistoryEntry {
    /// Wall-clock timestamp associated with the observation (milliseconds).
    pub timestamp_ms: u32,
    /// State prior to evaluation.
    pub prev_state: State,
    /// State after evaluation.
    pub next_state: State,
    /// Level prior to evaluation.
    pub prev_level: u8,
    /// Level after evaluation.
    pub next_level: u8,
    /// Time spent in the previous state before this evaluation.
    pub dwell_ms: u32,
    /// Whether the breaker recovered (e.g., closed cleanly) after this evaluation.
    pub success: bool,
    /// Snapshot of metrics before evaluation.
    pub before: MetricsSnapshot,
    /// Snapshot of metrics after evaluation.
    pub after: MetricsSnapshot,
    /// Telemetry sample that drove this evaluation.
    pub sample: TelemetrySample,
    /// Optional workload feedback describing recovery effectiveness.
    pub action_outcome: Option<ActionOutcome>,
}

impl HistoryEntry {
    /// Construct an empty entry used for buffer initialisation.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            timestamp_ms: 0,
            prev_state: State::Closed,
            next_state: State::Closed,
            prev_level: 0,
            next_level: 0,
            dwell_ms: 0,
            success: false,
            before: MetricsSnapshot {
                state: State::Closed,
                level: 0,
                err: 0,
                mu_norm: 0.0,
                sg_norm: 0.0,
                cause: 0,
                backoff: 0,
            },
            after: MetricsSnapshot {
                state: State::Closed,
                level: 0,
                err: 0,
                mu_norm: 0.0,
                sg_norm: 0.0,
                cause: 0,
                backoff: 0,
            },
            sample: TelemetrySample::zero(),
            action_outcome: None,
        }
    }
}

/// Circular buffer retaining the most recent breaker evaluations.
#[derive(Clone, Debug)]
pub struct HistoryBuffer {
    entries: Vec<HistoryEntry>,
    cursor: usize,
    len: usize,
}

impl HistoryBuffer {
    /// Create a history buffer with the requested capacity (defaults to 512 if zero).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = if capacity == 0 { 512 } else { capacity };
        let entries = vec![HistoryEntry::empty(); capacity];
        Self {
            entries,
            cursor: 0,
            len: 0,
        }
    }

    /// Return the number of stored entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return the configured capacity of the buffer.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the buffer contains any entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append a new history entry, evicting the oldest one when capacity is reached.
    pub fn record(&mut self, entry: HistoryEntry) {
        if self.entries.is_empty() {
            return;
        }
        self.entries[self.cursor] = entry;
        self.cursor = (self.cursor + 1) % self.entries.len();
        if self.len < self.entries.len() {
            self.len += 1;
        }
    }

    /// Reset the buffer, dropping all retained entries.
    pub fn clear(&mut self) {
        self.cursor = 0;
        self.len = 0;
    }

    /// Iterate over entries in chronological order (oldest to newest).
    #[must_use]
    pub fn iter(&self) -> HistoryIter<'_> {
        let start = if self.len == self.entries.len() {
            self.cursor
        } else {
            0
        };
        HistoryIter {
            buffer: self,
            index: start,
            yielded: 0,
        }
    }

    /// Export the accumulated history as CSV to the provided writer.
    pub fn export_csv<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writeln!(
            writer,
            "timestamp_ms,prev_state,next_state,prev_level,next_level,dwell_ms,success,mu_before,sg_before,mu_after,sg_after,err_after,cause_after,backoff_after,mu_sample,sg_sample,err_inc,cause_sample,backoff_sample,recovered_within_target,recovery_ms"
        )?;
        for entry in self.iter() {
            writeln!(
                writer,
                "{},{:?},{:?},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{},{},{},{:.4},{:.4},{},{},{},{},{}",
                entry.timestamp_ms,
                entry.prev_state,
                entry.next_state,
                entry.prev_level,
                entry.next_level,
                entry.dwell_ms,
                entry.success,
                entry.before.mu_norm,
                entry.before.sg_norm,
                entry.after.mu_norm,
                entry.after.sg_norm,
                entry.after.err,
                entry.after.cause,
                entry.after.backoff,
                entry.sample.mu_norm,
                entry.sample.sg_norm,
                entry.sample.err_inc,
                entry.sample.cause,
                entry.sample.backoff_hint.unwrap_or(0),
                entry
                    .action_outcome
                    .is_some_and(|outcome| outcome.recovered_within_target),
                entry
                    .action_outcome
                    .and_then(|outcome| outcome.observed_recovery_ms)
                    .unwrap_or(0),
            )?;
        }
        Ok(())
    }
}

impl Default for HistoryBuffer {
    fn default() -> Self {
        Self::new(512)
    }
}

/// Iterator over history entries.
pub struct HistoryIter<'a> {
    buffer: &'a HistoryBuffer,
    index: usize,
    yielded: usize,
}

impl<'a> Iterator for HistoryIter<'a> {
    type Item = &'a HistoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.yielded >= self.buffer.len {
            return None;
        }
        let idx = if self.buffer.entries.is_empty() {
            0
        } else {
            (self.index + self.yielded) % self.buffer.entries.len()
        };
        self.yielded += 1;
        self.buffer.entries.get(idx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.len.saturating_sub(self.yielded);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for HistoryIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_buffer_wraps_correctly() {
        let mut history = HistoryBuffer::new(3);
        for idx in 0..5u32 {
            let mut entry = HistoryEntry::empty();
            entry.timestamp_ms = idx;
            entry.sample.mu_norm = idx as f32;
            history.record(entry);
        }
        let collected: Vec<_> = history.iter().map(|e| e.timestamp_ms).collect();
        assert_eq!(collected, vec![2, 3, 4]);
    }

    #[test]
    fn export_csv_emits_header_and_rows() {
        let mut history = HistoryBuffer::new(2);
        let mut entry = HistoryEntry::empty();
        entry.timestamp_ms = 42;
        entry.sample.mu_norm = 1.25;
        entry.after.mu_norm = 0.75;
        history.record(entry);

        let mut output = Vec::new();
        history.export_csv(&mut output).unwrap();
        let csv = String::from_utf8(output).unwrap();
        assert!(csv.contains("timestamp_ms"));
        assert!(csv.contains("42"));
    }
}
