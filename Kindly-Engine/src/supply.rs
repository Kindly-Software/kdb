use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, Ordering};

/// Directed road edge for pressure/flow propagation.
#[derive(Debug, Clone, Copy)]
pub struct SupplyRoad {
    pub from: u32,
    pub to: u32,
    pub capacity_q16: u32,
    pub loss_q16: u32,
    pub distance_tiles: u32,
}

/// Snapshot of supply state for analytics/tests.
#[derive(Debug, Clone)]
pub struct SupplySnapshot {
    pub pressure: Vec<u32>,
    pub fatigue_penalty_q16: Vec<u32>,
    pub ammo: Vec<u32>,
    pub ammo_gain: Vec<u32>,
    pub throughput_q16: Vec<u32>,
    pub avg_pressure_q16: u32,
    pub avg_throughput_q16: u32,
    pub baggage_captured: bool,
    pub road_integrity_q16: Vec<u32>,
    pub road_disrupted: Vec<u8>,
    pub disruption_events: u32,
    pub attrition_events: u32,
    pub command_delay_penalty_ticks: Vec<u16>,
    pub command_delay_penalty_avg_ticks: u16,
}

/// Pressure/flow capsule for supply and logistics.
///
/// - Roads propagate pressure with capacity/loss/distance scaling.
/// - Weather (mud/wind) and decay clamp delivery.
/// - Fatigue/starvation hooks: low pressure raises fatigue penalty.
/// - Ammo resupply: pressure converts to ammo trickle per node.
#[repr(C, align(128))]
pub struct SupplyCapsule {
    roads: Vec<SupplyRoad>,
    pressure: Vec<AtomicU32>,
    ammo: Vec<AtomicU32>,
    fatigue_penalty_q16: Vec<AtomicU32>,
    decay_q16: AtomicU32,
    weather_mud_q16: AtomicU32,
    wind_q16: AtomicU32,
    road_integrity_q16: Vec<AtomicU32>,
    road_disrupted: Vec<AtomicU32>,
    _padding: [u8; 12],
}

impl SupplyCapsule {
    /// Create a supply graph with `nodes` entries (all zeroed).
    pub fn new(nodes: usize) -> Self {
        let mut pressure = Vec::with_capacity(nodes);
        let mut ammo = Vec::with_capacity(nodes);
        let mut fatigue = Vec::with_capacity(nodes);
        for _ in 0..nodes {
            pressure.push(AtomicU32::new(0));
            ammo.push(AtomicU32::new(0));
            fatigue.push(AtomicU32::new(0));
        }
        Self {
            roads: Vec::new(),
            pressure,
            ammo,
            fatigue_penalty_q16: fatigue,
            decay_q16: AtomicU32::new(2_048), // ~3% decay per tick by default
            weather_mud_q16: AtomicU32::new(0),
            wind_q16: AtomicU32::new(0),
            road_integrity_q16: Vec::new(),
            road_disrupted: Vec::new(),
            _padding: [0; 12],
        }
    }

    /// Add a directed road edge; capacity/loss are Q16.16 scales, distance in tiles (penalizes delivery).
    pub fn add_road(
        &mut self,
        from: u32,
        to: u32,
        capacity_q16: u32,
        loss_q16: u32,
        distance_tiles: u32,
    ) {
        if (from as usize) < self.pressure.len() && (to as usize) < self.pressure.len() {
            self.roads.push(SupplyRoad {
                from,
                to,
                capacity_q16,
                loss_q16,
                distance_tiles,
            });
            self.road_integrity_q16
                .push(AtomicU32::new(65_536));
            self.road_disrupted.push(AtomicU32::new(0));
        }
    }

    /// Inject pressure at a node (e.g., depot/source).
    pub fn inject_pressure(&self, node: u32, pressure_q16: u32) {
        if let Some(slot) = self.pressure.get(node as usize) {
            slot.store(pressure_q16, Ordering::Release);
        }
    }

    /// Set initial ammo at a node (e.g., formation stock).
    pub fn set_ammo(&self, node: u32, ammo_units: u32) {
        if let Some(slot) = self.ammo.get(node as usize) {
            slot.store(ammo_units, Ordering::Release);
        }
    }

    /// Configure decay/weather penalties.
    pub fn set_decay_q16(&self, decay_q16: u32) {
        self.decay_q16
            .store(decay_q16.min(65_536), Ordering::Release);
    }

    pub fn set_weather(&self, mud_q16: u32, wind_q16: u32) {
        self.weather_mud_q16
            .store(mud_q16.min(65_536), Ordering::Release);
        self.wind_q16.store(wind_q16.min(65_536), Ordering::Release);
    }

    /// Step the supply graph deterministically and return a snapshot.
    pub fn step(&self) -> SupplySnapshot {
        let nodes = self.pressure.len();
        let mut available: Vec<u32> = self
            .pressure
            .iter()
            .map(|p| p.load(Ordering::Acquire))
            .collect();
        let weather_scale =
            65_536u32.saturating_sub(self.weather_mud_q16.load(Ordering::Relaxed) / 2);
        let wind_scale = 65_536u32.saturating_sub(self.wind_q16.load(Ordering::Relaxed) / 4);
        let mut throughput: Vec<u32> = vec![0; nodes];
        let mut road_integrity_snapshot: Vec<u32> = Vec::with_capacity(self.roads.len());
        let mut road_disrupted_snapshot: Vec<u8> = Vec::with_capacity(self.roads.len());
        let mut node_disrupted: Vec<bool> = vec![false; nodes];
        let mut disruption_events = 0u32;
        let mut attrition_events = 0u32;

        for (idx, road) in self.roads.iter().enumerate() {
            let integrity = self
                .road_integrity_q16
                .get(idx)
                .map(|v| v.load(Ordering::Acquire))
                .unwrap_or(65_536);
            let was_disrupted = self
                .road_disrupted
                .get(idx)
                .map(|v| v.load(Ordering::Relaxed) != 0)
                .unwrap_or(false);
            let Some(src_avail) = available.get_mut(road.from as usize) else {
                road_integrity_snapshot.push(integrity);
                road_disrupted_snapshot.push(was_disrupted as u8);
                continue;
            };
            let delivered = self.compute_delivery(road, *src_avail, weather_scale, wind_scale);
            let integrity_scale = integrity.min(65_536) as u64;
            let mut delivered = ((delivered as u64 * integrity_scale) / 65_536)
                .min(u32::MAX as u64) as u32;
            if was_disrupted {
                delivered /= 2;
            }
            let attrition = compute_attrition(road, delivered, integrity);
            let repair = compute_repair(delivered, integrity);
            let mut next_integrity = integrity
                .saturating_sub(attrition)
                .saturating_add(repair)
                .min(65_536);
            let mut is_disrupted = was_disrupted;
            if delivered < 2_000 || next_integrity < 24_000 {
                is_disrupted = true;
            } else if is_disrupted && delivered > 16_000 && next_integrity > 36_000 {
                // Recover once traffic + integrity rebound.
                is_disrupted = false;
            }
            if attrition > 0 {
                attrition_events = attrition_events.saturating_add(1);
            }
            if is_disrupted && !was_disrupted {
                disruption_events = disruption_events.saturating_add(1);
            }

            self.road_integrity_q16
                .get(idx)
                .map(|v| v.store(next_integrity, Ordering::Release));
            self.road_disrupted
                .get(idx)
                .map(|v| v.store(is_disrupted as u32, Ordering::Release));
            road_integrity_snapshot.push(next_integrity);
            road_disrupted_snapshot.push(is_disrupted as u8);

            *src_avail = src_avail.saturating_sub(delivered);
            if let Some(dst) = available.get_mut(road.to as usize) {
                *dst = dst.saturating_add(delivered);
                throughput[road.to as usize] =
                    throughput[road.to as usize].saturating_add(delivered);
                if is_disrupted {
                    if let Some(flag) = node_disrupted.get_mut(road.to as usize) {
                        *flag = true;
                    }
                }
            }
        }

        let decay_q16 = self.decay_q16.load(Ordering::Relaxed).min(65_536);
        let decay_scale = 65_536u32.saturating_sub(decay_q16);
        let mut snapshot = SupplySnapshot {
            pressure: Vec::with_capacity(nodes),
            fatigue_penalty_q16: Vec::with_capacity(nodes),
            ammo: Vec::with_capacity(nodes),
            ammo_gain: Vec::with_capacity(nodes),
            throughput_q16: Vec::with_capacity(nodes),
            avg_pressure_q16: 0,
            avg_throughput_q16: 0,
            baggage_captured: false,
            road_integrity_q16: road_integrity_snapshot,
            road_disrupted: road_disrupted_snapshot,
            disruption_events,
            attrition_events,
            command_delay_penalty_ticks: Vec::with_capacity(nodes),
            command_delay_penalty_avg_ticks: 0,
        };

        let mut total_pressure = 0u64;
        let mut total_throughput = 0u64;
        let mut command_delay_penalty_sum = 0u64;
        for idx in 0..nodes {
            let merged = available[idx];
            total_pressure = total_pressure.saturating_add(merged as u64);
            total_throughput = total_throughput.saturating_add(throughput[idx] as u64);
            let decayed =
                ((merged as u64 * decay_scale as u64) / 65_536).min(u32::MAX as u64) as u32;
            self.pressure[idx].store(decayed, Ordering::Release);

            let fatigue_penalty = compute_fatigue_penalty(decayed);
            self.fatigue_penalty_q16[idx].store(fatigue_penalty, Ordering::Release);

            let ammo_gain = ammo_from_pressure(decayed);
            let new_ammo = self.ammo[idx]
                .load(Ordering::Acquire)
                .saturating_add(ammo_gain);
            self.ammo[idx].store(new_ammo, Ordering::Release);

            snapshot.pressure.push(decayed);
            snapshot.fatigue_penalty_q16.push(fatigue_penalty);
            snapshot.ammo.push(new_ammo);
            snapshot.ammo_gain.push(ammo_gain);
            snapshot.throughput_q16.push(throughput[idx]);
            if decayed < 12_000 {
                snapshot.baggage_captured = true;
            }
            let cmd_penalty =
                compute_command_delay_penalty(decayed, *node_disrupted.get(idx).unwrap_or(&false));
            command_delay_penalty_sum =
                command_delay_penalty_sum.saturating_add(cmd_penalty as u64);
            snapshot
                .command_delay_penalty_ticks
                .push(cmd_penalty.min(u16::MAX as u32) as u16);
        }
        snapshot.avg_pressure_q16 = if nodes > 0 {
            (total_pressure / nodes as u64).min(u32::MAX as u64) as u32
        } else {
            0
        };
        snapshot.avg_throughput_q16 = if nodes > 0 {
            (total_throughput / nodes as u64).min(u32::MAX as u64) as u32
        } else {
            0
        };
        snapshot.command_delay_penalty_avg_ticks = if nodes > 0 {
            (command_delay_penalty_sum / nodes as u64).min(u16::MAX as u64) as u16
        } else {
            0
        };

        snapshot
    }

    /// Zero-copy-ish snapshot (atomic loads) of the current state.
    pub fn snapshot(&self) -> SupplySnapshot {
        let mut snap = SupplySnapshot {
            pressure: Vec::with_capacity(self.pressure.len()),
            fatigue_penalty_q16: Vec::with_capacity(self.pressure.len()),
            ammo: Vec::with_capacity(self.pressure.len()),
            ammo_gain: Vec::with_capacity(self.pressure.len()),
            throughput_q16: Vec::with_capacity(self.pressure.len()),
            avg_pressure_q16: 0,
            avg_throughput_q16: 0,
            baggage_captured: false,
            road_integrity_q16: self
                .road_integrity_q16
                .iter()
                .map(|v| v.load(Ordering::Acquire))
                .collect(),
            road_disrupted: self
                .road_disrupted
                .iter()
                .map(|v| (v.load(Ordering::Relaxed) != 0) as u8)
                .collect(),
            disruption_events: 0,
            attrition_events: 0,
            command_delay_penalty_ticks: Vec::with_capacity(self.pressure.len()),
            command_delay_penalty_avg_ticks: 0,
        };
        let mut node_disrupted = vec![false; self.pressure.len()];
        for (idx, road) in self.roads.iter().enumerate() {
            if self
                .road_disrupted
                .get(idx)
                .map(|v| v.load(Ordering::Relaxed) != 0)
                .unwrap_or(false)
            {
                if let Some(flag) = node_disrupted.get_mut(road.to as usize) {
                    *flag = true;
                }
            }
        }
        let mut command_delay_penalty_sum = 0u64;
        for idx in 0..self.pressure.len() {
            snap.pressure
                .push(self.pressure[idx].load(Ordering::Acquire));
            snap.fatigue_penalty_q16
                .push(self.fatigue_penalty_q16[idx].load(Ordering::Acquire));
            snap.ammo.push(self.ammo[idx].load(Ordering::Acquire));
            snap.ammo_gain.push(0);
            snap.throughput_q16.push(0);
            if self.pressure[idx].load(Ordering::Acquire) < 12_000 {
                snap.baggage_captured = true;
            }
            let cmd_penalty = compute_command_delay_penalty(
                self.pressure[idx].load(Ordering::Acquire),
                *node_disrupted.get(idx).unwrap_or(&false),
            );
            command_delay_penalty_sum =
                command_delay_penalty_sum.saturating_add(cmd_penalty as u64);
            snap.command_delay_penalty_ticks
                .push(cmd_penalty.min(u16::MAX as u32) as u16);
        }
        let nodes = self.pressure.len().max(1) as u64;
        let total: u64 = snap
            .pressure
            .iter()
            .fold(0u64, |acc, p| acc.saturating_add(*p as u64));
        snap.avg_pressure_q16 = (total / nodes).min(u32::MAX as u64) as u32;
        snap.command_delay_penalty_avg_ticks =
            (command_delay_penalty_sum / nodes).min(u16::MAX as u64) as u16;
        snap
    }

    fn compute_delivery(
        &self,
        road: &SupplyRoad,
        source_pressure: u32,
        weather_scale: u32,
        wind_scale: u32,
    ) -> u32 {
        let capacity = road.capacity_q16.min(65_536) as u64;
        let distance_penalty =
            65_536u64.saturating_sub((road.distance_tiles.min(128) * 256) as u64);
        let mut delivered = (source_pressure as u64 * capacity) / 65_536;
        delivered = (delivered * weather_scale as u64) / 65_536;
        delivered = (delivered * wind_scale as u64) / 65_536;
        delivered = (delivered * distance_penalty) / 65_536;
        delivered = delivered.saturating_sub(road.loss_q16 as u64);
        delivered.min(u32::MAX as u64) as u32
    }
}

verify_capsule_properties!(SupplyCapsule, 128, 256);

fn compute_fatigue_penalty(pressure_q16: u32) -> u32 {
    let deficit = 50_000i64 - pressure_q16 as i64;
    if deficit <= 0 {
        0
    } else {
        (deficit as u32 / 2).min(40_000)
    }
}

fn ammo_from_pressure(pressure_q16: u32) -> u32 {
    if pressure_q16 < 4_096 {
        0
    } else {
        // ~pressure/2048 units; clamp to a small range to stay deterministic.
        (pressure_q16 / 2_048).min(2_000)
    }
}

fn compute_attrition(road: &SupplyRoad, delivered: u32, integrity_q16: u32) -> u32 {
    let distance_penalty = (road.distance_tiles.min(128) * 64) as u32;
    let loss_penalty = (road.loss_q16.min(20_000)) / 12;
    let flow_penalty = if delivered > 48_000 {
        delivered / 96
    } else {
        delivered / 192
    };
    let integrity_penalty = (65_536u32.saturating_sub(integrity_q16)) / 64;
    distance_penalty
        .saturating_add(loss_penalty)
        .saturating_add(flow_penalty)
        .saturating_add(integrity_penalty)
        .min(4_096)
}

fn compute_repair(delivered: u32, integrity_q16: u32) -> u32 {
    if delivered < 8_000 || integrity_q16 > 50_000 {
        return 0;
    }
    (delivered / 512).min(2_048)
}

fn compute_command_delay_penalty(pressure_q16: u32, disrupted: bool) -> u32 {
    let deficit = 32_000u32.saturating_sub(pressure_q16);
    let base = deficit / 4_000;
    let disrupted_penalty = if disrupted { 3 } else { 0 };
    base.saturating_add(disrupted_penalty).min(180)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_flows_and_resupplies() {
        let mut supply = SupplyCapsule::new(3);
        supply.add_road(0, 1, 60_000, 2_000, 4);
        supply.add_road(1, 2, 65_536, 1_000, 2);
        supply.inject_pressure(0, 60_000);
        supply.set_ammo(2, 0);
        supply.set_decay_q16(2_000);

        let snap = supply.step();
        assert!(snap.pressure[1] > 0);
        assert!(snap.pressure[2] > 0);
        assert!(snap.fatigue_penalty_q16[2] < 40_000);
        assert!(snap.ammo[2] > 0);
    }

    #[test]
    fn bad_weather_increases_fatigue_and_reduces_delivery() {
        let mut supply = SupplyCapsule::new(2);
        supply.add_road(0, 1, 65_536, 0, 8);
        supply.inject_pressure(0, 50_000);
        supply.set_weather(50_000, 20_000);
        supply.set_decay_q16(15_000);

        let snap = supply.step();
        assert!(snap.pressure[1] < 50_000);
        assert!(snap.fatigue_penalty_q16[1] > 0);
    }

    #[test]
    fn disrupted_roads_reduce_integrity_and_add_command_penalty() {
        let mut supply = SupplyCapsule::new(2);
        supply.add_road(0, 1, 40_000, 8_000, 24);
        supply.inject_pressure(0, 30_000);

        let snap = supply.step();
        assert_eq!(snap.road_integrity_q16.len(), 1);
        assert!(snap.road_integrity_q16[0] < 65_536);
        // Severe distance/loss should trigger attrition or disruption tracking.
        assert!(snap.disruption_events > 0 || snap.attrition_events > 0);
        assert!(snap.command_delay_penalty_avg_ticks > 0);
    }
}
