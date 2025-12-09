use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::inflection::InflectionEvent;
use crate::motion::MotionBlockCapsule;
use crate::presets::{PresetLibraryCapsule, PresetMeta};

#[derive(Debug, Clone, Copy)]
pub struct StretchHandles {
    pub left_ms: u64,
    pub right_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapGrid {
    Milliseconds(u32),
    Bpm(u32), // beats per minute
}

#[derive(Debug, Clone, Copy)]
pub struct TimelineUiState {
    pub zoom: f32,
    pub snap: Option<SnapGrid>,
    pub snap_tolerance_ms: u32,
    pub show_stretch_handles: bool,
    pub last_shown_duration_ms: u64,
    pub last_shown_tempo_bpm: Option<u32>,
    pub grid_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TimelineUiHint {
    pub zoom: f32,
    pub snap: Option<SnapGrid>,
    pub grid_ms: Option<u32>,
    pub snap_tolerance_ms: u32,
    pub show_stretch_handles: bool,
    pub stretch_handles: Option<StretchHandles>,
    pub duration_ms: u64,
    pub tempo_bpm: Option<u32>,
    pub grid_lines_ms: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub start_ms: u64,
    pub duration_ms: u64,
    pub stretch_ppm: u32,
    pub preset_name: Option<String>,
    pub preset_meta: Option<PresetMeta>,
    pub block: MotionBlockCapsule,
}

impl TimelineEntry {
    pub fn new(start_ms: u64, duration_ms: u64, block: MotionBlockCapsule) -> Self {
        Self {
            start_ms,
            duration_ms,
            stretch_ppm: 1_000_000,
            preset_name: None,
            preset_meta: None,
            block,
        }
    }

    pub fn with_stretch_ppm(mut self, stretch_ppm: u32) -> Self {
        self.stretch_ppm = stretch_ppm.max(1); // avoid zero
        self
    }

    pub fn with_preset_name(mut self, name: impl Into<String>) -> Self {
        self.preset_name = Some(name.into());
        self
    }

    pub fn with_preset_meta(mut self, meta: PresetMeta) -> Self {
        self.preset_meta = Some(meta);
        self
    }

    pub fn effective_duration_ms(&self) -> u64 {
        ((self.duration_ms as u128 * self.stretch_ppm as u128) / 1_000_000) as u64
    }

    pub fn effective_tempo_bpm(&self) -> u32 {
        let base = self.block.tempo().base_bpm() as f32;
        let stretch = self.stretch_ppm as f32 / 1_000_000.0;
        ((base / stretch).round() as u32).max(1)
    }

    pub fn end_ms(&self) -> u64 {
        self.start_ms.saturating_add(self.effective_duration_ms())
    }

    pub fn stretch_handles(&self) -> StretchHandles {
        StretchHandles {
            left_ms: self.start_ms,
            right_ms: self.end_ms(),
        }
    }

    pub fn duplicated(&self, offset_ms: u64) -> TimelineEntry {
        let mut clone = self.clone();
        clone.start_ms = self.start_ms.saturating_add(offset_ms);
        clone
    }

    pub fn inverted(&self) -> TimelineEntry {
        let mut inverted = self.clone();
        inverted.block = self.block.inverted_range();
        inverted
    }

    pub fn looped(&self, times: u32, gap_ms: u64) -> Vec<TimelineEntry> {
        let mut out = Vec::new();
        let base_duration = self.effective_duration_ms();
        for i in 0..times.max(1) {
            let mut clone = self.clone();
            let offset = i as u64 * (base_duration + gap_ms);
            clone.start_ms = self.start_ms.saturating_add(offset);
            out.push(clone);
        }
        out
    }
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct TimelineCapsule {
    generation: AtomicU64,
    entries: Vec<TimelineEntry>,
    ui: TimelineUiState,
}

impl TimelineCapsule {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            entries: Vec::new(),
            ui: TimelineUiState {
                zoom: 1.0,
                snap: None,
                snap_tolerance_ms: 25,
                show_stretch_handles: true,
                last_shown_duration_ms: 0,
                last_shown_tempo_bpm: None,
                grid_ms: None,
            },
        }
    }

    pub fn push(&mut self, entry: TimelineEntry) {
        self.entries.push(entry);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn push_from_preset(
        &mut self,
        preset: &PresetLibraryCapsule,
        name: &str,
        start_ms: u64,
        duration_ms: u64,
        stretch_ppm: u32,
    ) -> bool {
        if let Some(block) = preset.get(name) {
            let mut entry = TimelineEntry::new(start_ms, duration_ms, block)
                .with_stretch_ppm(stretch_ppm)
                .with_preset_name(name.to_string());
            if let Some(meta) = preset.meta(name).cloned() {
                entry = entry.with_preset_meta(meta);
            }
            self.push(entry);
            true
        } else {
            false
        }
    }

    pub fn entries(&self) -> &[TimelineEntry] {
        &self.entries
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn ui_state(&self) -> &TimelineUiState {
        &self.ui
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.ui.zoom = zoom.clamp(0.1, 10.0);
    }

    pub fn set_snap(&mut self, snap: Option<SnapGrid>) {
        self.ui.snap = snap;
        self.ui.grid_ms = match snap {
            Some(SnapGrid::Milliseconds(ms)) => Some(ms),
            Some(SnapGrid::Bpm(bpm)) if bpm > 0 => Some(60_000u32.saturating_div(bpm.max(1))),
            _ => None,
        };
    }

    pub fn set_snap_tolerance_ms(&mut self, tolerance_ms: u32) {
        self.ui.snap_tolerance_ms = tolerance_ms;
    }

    pub fn set_show_stretch_handles(&mut self, show: bool) {
        self.ui.show_stretch_handles = show;
    }

    pub fn update_hint_duration_tempo(&mut self, duration_ms: u64, tempo_bpm: Option<u32>) {
        self.ui.last_shown_duration_ms = duration_ms;
        self.ui.last_shown_tempo_bpm = tempo_bpm;
    }

    pub fn ui_hint(&self, selected: Option<usize>) -> TimelineUiHint {
        self.ui_hint_with_viewport(selected, None)
    }

    pub fn ui_hint_with_viewport(
        &self,
        selected: Option<usize>,
        viewport_ms: Option<(u64, u64)>,
    ) -> TimelineUiHint {
        let stretch_handles = selected
            .and_then(|idx| self.entries.get(idx))
            .map(|e| e.stretch_handles());
        let grid_lines_ms = viewport_ms
            .or_else(|| stretch_handles.map(|h| (h.left_ms, h.right_ms)))
            .map(|(start, end)| self.grid_lines_for_view(start, end, 256))
            .unwrap_or_default();
        TimelineUiHint {
            zoom: self.ui.zoom,
            snap: self.ui.snap,
            grid_ms: self.ui.grid_ms,
            snap_tolerance_ms: self.ui.snap_tolerance_ms,
            show_stretch_handles: self.ui.show_stretch_handles,
            stretch_handles,
            duration_ms: self.ui.last_shown_duration_ms,
            tempo_bpm: self.ui.last_shown_tempo_bpm,
            grid_lines_ms,
        }
    }

    pub fn refresh_hint_for(&mut self, entry: &TimelineEntry) {
        self.update_hint_duration_tempo(entry.effective_duration_ms(), Some(entry.effective_tempo_bpm()));
    }

    pub fn tempo_for_entry(&self, index: usize) -> Option<u32> {
        self.entries.get(index).map(|e| e.effective_tempo_bpm())
    }

    pub fn snap_time(&self, time_ms: u64) -> u64 {
        match self.ui.snap {
            None => time_ms,
            Some(SnapGrid::Milliseconds(step)) if step > 0 => {
                let step64 = step as u64;
                ((time_ms + step64 / 2) / step64) * step64
            }
            Some(SnapGrid::Bpm(bpm)) if bpm > 0 => {
                let beat_ms = 60_000u64.saturating_div(bpm as u64).max(1);
                ((time_ms + beat_ms / 2) / beat_ms) * beat_ms
            }
            _ => time_ms,
        }
    }

    pub fn snap_entry(&self, entry: &TimelineEntry) -> TimelineEntry {
        let mut snapped = entry.clone();
        snapped.start_ms = self.snap_time(entry.start_ms);
        snapped.duration_ms = self.snap_time(entry.duration_ms.max(1));
        snapped
    }

    pub fn grid_lines_for_view(&self, start_ms: u64, end_ms: u64, max_lines: usize) -> Vec<u64> {
        let step = match self.grid_step_ms() {
            Some(step) if step > 0 => step as u64,
            _ => return Vec::new(),
        };
        if max_lines == 0 {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let first = (start_ms / step).saturating_mul(step);
        let mut t = first;
        while t <= end_ms.saturating_add(step) && lines.len() < max_lines {
            if t >= start_ms.saturating_sub(step) {
                lines.push(t);
            }
            t = t.saturating_add(step);
        }
        lines
    }

    fn grid_step_ms(&self) -> Option<u32> {
        if let Some(ms) = self.ui.grid_ms {
            if ms > 0 {
                return Some(ms);
            }
        }
        // Fallback grid derived from zoom level (zoom>1 → denser grid).
        let base = (100.0 / self.ui.zoom.max(0.1)).round() as u32;
        Some(base.max(10))
    }

    pub fn drag_entry(&mut self, index: usize, new_start_ms: u64) -> Option<TimelineEntry> {
        let snapped = self.snap_time(new_start_ms);
        let entry = self.entries.get_mut(index)?;
        entry.start_ms = snapped;
        let entry_clone = entry.clone();
        self.refresh_hint_for(&entry_clone);
        self.generation.fetch_add(1, Ordering::Relaxed);
        Some(entry_clone)
    }

    pub fn stretch_entry(&mut self, index: usize, new_duration_ms: u64) -> Option<TimelineEntry> {
        let snapped = self.snap_time(new_duration_ms.max(1));
        let entry = self.entries.get_mut(index)?;
        entry.duration_ms = snapped;
        let entry_clone = entry.clone();
        self.refresh_hint_for(&entry_clone);
        self.generation.fetch_add(1, Ordering::Relaxed);
        Some(entry_clone)
    }

    pub fn magnetize_to_inflections(
        &self,
        entry: &TimelineEntry,
        events: &[InflectionEvent],
    ) -> TimelineEntry {
        let mut adjusted = entry.clone();
        let tolerance = self.ui.snap_tolerance_ms as u64;

        if let Some(target) = nearest_event(entry.start_ms, events, tolerance) {
            adjusted.start_ms = target;
        }
        if let Some(target) = nearest_event(entry.end_ms(), events, tolerance) {
            let new_duration = target.saturating_sub(adjusted.start_ms);
            if new_duration > 0 {
                adjusted.duration_ms = new_duration;
            }
        }
        adjusted
    }

    pub fn apply_shortcut_duplicate(&mut self, index: usize, offset_ms: u64) -> Option<usize> {
        let entry = self.entries.get(index)?.duplicated(offset_ms);
        let new_index = self.entries.len();
        self.push(entry);
        Some(new_index)
    }

    pub fn apply_shortcut_invert(&mut self, index: usize) -> Option<()> {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = entry.inverted();
            self.generation.fetch_add(1, Ordering::Relaxed);
            return Some(());
        }
        None
    }

    pub fn apply_shortcut_loop(&mut self, index: usize, times: u32, gap_ms: u64) -> Option<()> {
        let entry = self.entries.get(index)?.clone();
        for clone in entry.looped(times, gap_ms).into_iter().skip(1) {
            self.push(clone);
        }
        Some(())
    }
}

impl Default for TimelineCapsule {
    fn default() -> Self {
        Self::new()
    }
}

fn nearest_event(target_time: u64, events: &[InflectionEvent], tolerance_ms: u64) -> Option<u64> {
    let mut best: Option<(u64, u64)> = None; // (delta, time)
    for e in events {
        let delta = if e.time_ms > target_time {
            e.time_ms - target_time
        } else {
            target_time - e.time_ms
        };
        if delta <= tolerance_ms && best.map(|(d, _)| delta < d).unwrap_or(true) {
            best = Some((delta, e.time_ms));
        }
    }
    best.map(|(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{MotionPattern, MotionTempo};

    #[test]
    fn push_increments_generation() {
        let mut timeline = TimelineCapsule::new();
        assert_eq!(timeline.generation(), 0);
        assert_eq!(timeline.len(), 0);

        let block =
            MotionBlockCapsule::new(1, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 2000);
        timeline.push(TimelineEntry::new(0, 2000, block));

        assert_eq!(timeline.generation(), 1);
        assert_eq!(timeline.len(), 1);
        let entry = &timeline.entries()[0];
        assert_eq!(entry.start_ms, 0);
        assert_eq!(entry.duration_ms, 2000);
        assert_eq!(entry.effective_duration_ms(), 2000);
        assert_eq!(entry.block.id(), 1);

        let stretched = entry.clone().with_stretch_ppm(1_500_000);
        assert_eq!(stretched.effective_duration_ms(), 3000);
        assert_eq!(stretched.stretch_ppm, 1_500_000);

        timeline.set_zoom(2.0);
        timeline.set_snap(Some(SnapGrid::Milliseconds(50)));
        timeline.set_show_stretch_handles(false);
        timeline.update_hint_duration_tempo(3000, Some(120));
        let ui = timeline.ui_state();
        assert_eq!(ui.zoom, 2.0);
        assert!(matches!(ui.snap, Some(SnapGrid::Milliseconds(50))));
        assert!(!ui.show_stretch_handles);
        assert_eq!(ui.last_shown_duration_ms, 3000);
        assert_eq!(ui.last_shown_tempo_bpm, Some(120));
        assert_eq!(ui.grid_ms, Some(50));

        timeline.refresh_hint_for(&stretched);
        assert_eq!(timeline.ui_state().last_shown_duration_ms, stretched.effective_duration_ms());

        let hint = timeline.ui_hint(Some(0));
        assert_eq!(hint.grid_ms, Some(50));
        assert!(hint.stretch_handles.is_some());
        assert!(!hint.grid_lines_ms.is_empty());
    }

    #[test]
    fn snapping_and_magnetize() {
        let mut timeline = TimelineCapsule::new();
        timeline.set_snap(Some(SnapGrid::Milliseconds(25)));
        timeline.set_snap_tolerance_ms(30);

        let block =
            MotionBlockCapsule::new(3, MotionPattern::Linear, 0, 100, MotionTempo::Lent, 1200);
        let entry = TimelineEntry::new(40, 103, block);
        let snapped = timeline.snap_entry(&entry);
        assert_eq!(snapped.start_ms, 50);
        assert_eq!(snapped.duration_ms, 100);

        let events = vec![InflectionEvent {
            time_ms: 50,
            position_pct: 100.0,
            kind: crate::inflection::InflectionKind::ImpactHigh,
        }];
        let magnetized = timeline.magnetize_to_inflections(&snapped, &events);
        assert_eq!(magnetized.start_ms, 50);

        let grid = timeline.grid_lines_for_view(0, 120, 32);
        assert!(!grid.is_empty());
        assert!(grid.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn shortcuts_duplicate_invert_loop() {
        let mut timeline = TimelineCapsule::new();
        let block =
            MotionBlockCapsule::new(4, MotionPattern::Linear, 10, 90, MotionTempo::Moyen, 800);
        timeline.push(TimelineEntry::new(0, 800, block));
        let dup_idx = timeline.apply_shortcut_duplicate(0, 1000).unwrap();
        assert_eq!(dup_idx, 1);
        assert_eq!(timeline.entries()[dup_idx].start_ms, 1000);

        timeline.apply_shortcut_invert(0).unwrap();
        assert_eq!(timeline.entries()[0].block.range(), (90, 10));

        timeline.apply_shortcut_loop(1, 2, 200).unwrap();
        assert!(timeline.len() >= 3);
    }

    #[test]
    fn snap_and_magnet_keep_sampler_deterministic() {
        use crate::sampler::sample_motion_block;

        let mut timeline = TimelineCapsule::new();
        timeline.set_snap(Some(SnapGrid::Milliseconds(20)));
        timeline.set_snap_tolerance_ms(15);

        let block =
            MotionBlockCapsule::new(6, MotionPattern::Linear, 0, 100, MotionTempo::Moyen, 600);
        let entry = TimelineEntry::new(43, 610, block);
        let events = vec![
            InflectionEvent {
                time_ms: 40,
                position_pct: 0.0,
                kind: crate::inflection::InflectionKind::ImpactLow,
            },
            InflectionEvent {
                time_ms: 340,
                position_pct: 100.0,
                kind: crate::inflection::InflectionKind::ImpactHigh,
            },
        ];

        let snapped = timeline.snap_entry(&entry);
        let magnetized = timeline.magnetize_to_inflections(&snapped, &events);
        let remagnetized = timeline.magnetize_to_inflections(&magnetized, &events);

        let samples_a = sample_motion_block(&magnetized, 90);
        let samples_b = sample_motion_block(&remagnetized, 90);

        assert_eq!(magnetized.start_ms, remagnetized.start_ms);
        assert_eq!(magnetized.duration_ms, remagnetized.duration_ms);
        assert_eq!(samples_a.len(), samples_b.len());
        for (a, b) in samples_a.iter().zip(samples_b.iter()) {
            assert!((a.position_pct - b.position_pct).abs() < 0.0001);
        }
    }
}
