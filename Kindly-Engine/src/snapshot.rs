use crate::diplomacy::{DiplomaticRelationSnapshot, DiplomaticSnapshot};
use crate::order::CommandDelaySnapshot;
use crate::province_economy::{BuildOrderSnapshot, EconomySnapshot};
use crate::formation::FormationCapsule;
use crate::order::OrderQueueCapsule;
use crate::siege::SiegeSectionSnapshot;
use crate::strategic_map::{StrategicEventKind, StrategicEventSnapshot, StrategicSnapshot};
use crate::structure::{StructureCapsule, StructureSnapshot};
use crate::telemetry::{TelemetryCapsule, TelemetrySnapshot};
use crate::world::WorldSlabCapsule;
use atomic_capsule::mmap::{MmapError, MmapLayout, MmapManager};
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

const SNAP_MAGIC: u32 = 0x574C4457; // "WLDW"
const SNAP_VERSION: u32 = 12;

/// Optional strategic payload persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct StrategicPersistSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub hash_chain: u64,
    pub prev_hash_chain: u64,
    pub events: Vec<StrategicEventSnapshot>,
}

/// Optional diplomatic payload persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct DiplomaticPersistSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub hash_chain: u64,
    pub prev_hash_chain: u64,
    pub relations: Vec<DiplomaticRelationSnapshot>,
}

/// Optional province economy payload persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct EconomyPersistSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub hash_chain: u64,
    pub prev_hash_chain: u64,
    pub orders: Vec<BuildOrderSnapshot>,
}

/// Optional campaign payload persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct CampaignPersistSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub hash_chain: u64,
    pub prev_hash_chain: u64,
    pub war_exhaustion_avg_q16: u32,
}

/// Optional command delay buffer snapshot persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct CommandDelayPersistSnapshot {
    pub pending: Vec<CommandDelaySnapshot>,
}

/// Optional siege payload persisted alongside tactical snapshots.
#[derive(Debug, Clone)]
pub struct SiegePersistSnapshot {
    pub sections: Vec<SiegeSectionSnapshot>,
}

/// Rehydrate a command delay buffer from a persisted snapshot.
pub fn restore_command_delays(
    snapshot: Option<&CommandDelayPersistSnapshot>,
    buffer: &crate::order::CommandDelayBufferCapsule,
) -> usize {
    if let Some(snap) = snapshot {
        buffer.restore_from(&snap.pending)
    } else {
        buffer.clear();
        0
    }
}

/// Snapshot capsule: serialize formations/orders/telemetry into a contiguous buffer.
#[repr(C, align(64))]
pub struct CampaignSnapshotCapsule {
    _padding: [u8; 64],
}

verify_capsule_properties!(CampaignSnapshotCapsule, 64, 64);

impl CampaignSnapshotCapsule {
    pub const fn new() -> Self {
        Self { _padding: [0; 64] }
    }

    pub fn serialize(
        &self,
        formations: &[FormationCapsule],
        orders: &OrderQueueCapsule,
        telemetry: &TelemetryCapsule,
        structures: &[StructureCapsule],
        strategic: Option<&StrategicSnapshot>,
        diplomatic: Option<&DiplomaticSnapshot>,
        economy: Option<&EconomySnapshot>,
        campaign: Option<&crate::campaign::CampaignFrame>,
        command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
        siege: Option<&crate::siege::SiegeSnapshot>,
        prev_hash: u64,
    ) -> Vec<u8> {
        let stats = orders.stats();
        let tele = telemetry.snapshot();
        let mut buf = Vec::with_capacity(96 + formations.len() * 64 + structures.len() * 48);
        buf.extend_from_slice(&SNAP_MAGIC.to_le_bytes());
        buf.extend_from_slice(&SNAP_VERSION.to_le_bytes());
        buf.extend_from_slice(&(formations.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(structures.len() as u32).to_le_bytes());
        buf.extend_from_slice(&stats.head.to_le_bytes());
        buf.extend_from_slice(&stats.tail.to_le_bytes());
        buf.extend_from_slice(&stats.dropped.to_le_bytes());
        buf.extend_from_slice(&stats.capacity.to_le_bytes());
        buf.extend_from_slice(&tele.events.to_le_bytes());
        buf.extend_from_slice(&tele.casualties.to_le_bytes());
        buf.extend_from_slice(&tele.shock_weight_q16.to_le_bytes());
        buf.extend_from_slice(&tele.ammo_spent.to_le_bytes());
        buf.extend_from_slice(&tele.tick_last_flush.to_le_bytes());
        buf.extend_from_slice(&tele.retreats.to_le_bytes());
        buf.extend_from_slice(&tele.musket_shots.to_le_bytes());
        buf.extend_from_slice(&tele.artillery_shots.to_le_bytes());
        buf.extend_from_slice(&tele.formation_breaks.to_le_bytes());
        buf.extend_from_slice(&tele.morale_shocks.to_le_bytes());
        buf.extend_from_slice(&tele.supply_pressure_accum_q16.to_le_bytes());
        buf.extend_from_slice(&tele.supply_fatigue_accum_q16.to_le_bytes());
        buf.extend_from_slice(&tele.supply_samples.to_le_bytes());
        buf.extend_from_slice(&tele.ai_orders.to_le_bytes());
        buf.extend_from_slice(&tele.charge_orders.to_le_bytes());
        buf.extend_from_slice(&tele.charge_commits.to_le_bytes());
        buf.extend_from_slice(&tele.brace_orders.to_le_bytes());

        for f in formations {
            let s = f.snapshot();
            buf.extend_from_slice(&s.formation_id.to_le_bytes());
            buf.push(s.posture);
            buf.push(s.stance);
            buf.extend_from_slice(&s.generation.to_le_bytes());
            buf.extend_from_slice(&s.cohesion_q16.to_le_bytes());
            buf.extend_from_slice(&s.fatigue_q16.to_le_bytes());
            buf.extend_from_slice(&s.ammo.to_le_bytes());
            buf.extend_from_slice(&s.morale_q16.to_le_bytes());
            buf.extend_from_slice(&s.facing_deg_q16.to_le_bytes());
            buf.extend_from_slice(&s.position_x_q16.to_le_bytes());
            buf.extend_from_slice(&s.position_z_q16.to_le_bytes());
            buf.extend_from_slice(&s.command_delay_ms.to_le_bytes());
            buf.extend_from_slice(&s.retreat_mode_flags.to_le_bytes());
            buf.push(s.charge_posture);
            buf.push(s.braced as u8);
            buf.extend_from_slice(&s.density_q16.to_le_bytes());
            buf.extend_from_slice(&s.mass_q16.to_le_bytes());
            buf.extend_from_slice(&s.variance_q16.to_le_bytes());
            buf.extend_from_slice(&s.damping_q16.to_le_bytes());
            buf.extend_from_slice(&s.velocity_q16.to_le_bytes());
            buf.extend_from_slice(&s.physics_flags.to_le_bytes());
        }

        for s in structures {
            let snap: StructureSnapshot = s.snapshot();
            buf.extend_from_slice(&snap.structure_id.to_le_bytes());
            buf.extend_from_slice(&snap.position_x_q16.to_le_bytes());
            buf.extend_from_slice(&snap.position_z_q16.to_le_bytes());
            buf.extend_from_slice(&snap.half_extent_x_q16.to_le_bytes());
            buf.extend_from_slice(&snap.half_extent_z_q16.to_le_bytes());
            for &face in &snap.cover_q16 {
                buf.extend_from_slice(&face.to_le_bytes());
            }
            buf.extend_from_slice(&snap.breach_mask.to_le_bytes());
            buf.extend_from_slice(&snap.slots_total.to_le_bytes());
            buf.extend_from_slice(&snap.slots_used.to_le_bytes());
            buf.extend_from_slice(&snap.apertures.to_le_bytes());
            buf.extend_from_slice(&snap.generation.to_le_bytes());
        }
        // Strategic block (optional).
        let empty_events: [StrategicEventSnapshot; 0] = [];
        let (strat_tick, strat_gen, strat_hash, strat_prev, strat_events) =
            if let Some(strat) = strategic {
                (
                    strat.tick,
                    strat.generation,
                    strat.hash_chain,
                    strat.prev_hash_chain,
                    strat.events.as_slice(),
                )
            } else {
                (0, 0, 0, 0, empty_events.as_slice())
            };
        buf.extend_from_slice(&strat_tick.to_le_bytes());
        buf.extend_from_slice(&strat_gen.to_le_bytes());
        buf.extend_from_slice(&strat_hash.to_le_bytes());
        buf.extend_from_slice(&strat_prev.to_le_bytes());
        buf.extend_from_slice(&(strat_events.len() as u32).to_le_bytes());
        for ev in strat_events {
            buf.push(ev.kind as u8);
            buf.extend_from_slice(&ev.province_id.to_le_bytes());
            buf.extend_from_slice(&ev.from_owner_id.to_le_bytes());
            buf.extend_from_slice(&ev.to_owner_id.to_le_bytes());
            buf.extend_from_slice(&ev.from_infra_q16.to_le_bytes());
            buf.extend_from_slice(&ev.to_infra_q16.to_le_bytes());
            buf.extend_from_slice(&ev.resistance_q16.to_le_bytes());
            buf.extend_from_slice(&ev.generation.to_le_bytes());
        }
        // Diplomatic block (optional).
        let empty_relations: [DiplomaticRelationSnapshot; 0] = [];
        let (dip_tick, dip_gen, dip_hash, dip_prev, dip_relations) =
            if let Some(dip) = diplomatic {
                (
                    dip.tick,
                    dip.generation,
                    dip.hash_chain,
                    dip.prev_hash_chain,
                    dip.relations.as_slice(),
                )
            } else {
                (0, 0, 0, 0, empty_relations.as_slice())
            };
        buf.extend_from_slice(&dip_tick.to_le_bytes());
        buf.extend_from_slice(&dip_gen.to_le_bytes());
        buf.extend_from_slice(&dip_hash.to_le_bytes());
        buf.extend_from_slice(&dip_prev.to_le_bytes());
        buf.extend_from_slice(&(dip_relations.len() as u32).to_le_bytes());
        for rel in dip_relations {
            buf.push(rel.state as u8);
            buf.extend_from_slice(&rel.a.to_le_bytes());
            buf.extend_from_slice(&rel.b.to_le_bytes());
            buf.extend_from_slice(&rel.truce_until_tick.to_le_bytes());
            buf.extend_from_slice(&rel.casus_belli_until_tick.to_le_bytes());
            buf.extend_from_slice(&rel.war_exhaustion_q16.to_le_bytes());
            buf.extend_from_slice(&rel.generation.to_le_bytes());
        }
        // Economy block (optional).
        let empty_orders: [BuildOrderSnapshot; 0] = [];
        let (econ_tick, econ_gen, econ_hash, econ_prev, econ_orders) =
            if let Some(econ) = economy {
                (
                    econ.tick,
                    econ.generation,
                    econ.hash_chain,
                    econ.prev_hash_chain,
                    econ.orders.as_slice(),
                )
            } else {
                (0, 0, 0, 0, empty_orders.as_slice())
            };
        buf.extend_from_slice(&econ_tick.to_le_bytes());
        buf.extend_from_slice(&econ_gen.to_le_bytes());
        buf.extend_from_slice(&econ_hash.to_le_bytes());
        buf.extend_from_slice(&econ_prev.to_le_bytes());
        buf.extend_from_slice(&(econ_orders.len() as u32).to_le_bytes());
        for order in econ_orders {
            buf.push(order.kind.as_u8());
            buf.extend_from_slice(&order.province_id.to_le_bytes());
            buf.extend_from_slice(&order.target_infra_q16.to_le_bytes());
            buf.extend_from_slice(&order.remaining_ticks.to_le_bytes());
            buf.extend_from_slice(&order.generation.to_le_bytes());
        }
        // Campaign block (optional).
        let (camp_tick, camp_gen, camp_hash, camp_prev, camp_war_exh) =
            if let Some(camp) = campaign {
                (
                    camp.tick,
                    camp.generation,
                    camp.hash_chain,
                    camp.prev_hash_chain,
                    camp.war_exhaustion_avg_q16,
                )
            } else {
                (0, 0, 0, 0, 0)
            };
        buf.extend_from_slice(&camp_tick.to_le_bytes());
        buf.extend_from_slice(&camp_gen.to_le_bytes());
        buf.extend_from_slice(&camp_hash.to_le_bytes());
        buf.extend_from_slice(&camp_prev.to_le_bytes());
        buf.extend_from_slice(&camp_war_exh.to_le_bytes());
        // Command delay buffer (optional).
        let empty_delays: [CommandDelaySnapshot; 0] = [];
        let delay_orders = if let Some(delays) = command_delays {
            delays.pending_snapshots()
        } else {
            empty_delays.to_vec()
        };
        buf.extend_from_slice(&(delay_orders.len() as u32).to_le_bytes());
        for d in &delay_orders {
            buf.extend_from_slice(&d.ready_tick.to_le_bytes());
            buf.push(d.order.kind as u8);
            buf.extend_from_slice(&d.order.formation_id.to_le_bytes());
            buf.extend_from_slice(&d.order.generation.to_le_bytes());
            buf.extend_from_slice(&d.order.payload_a.to_le_bytes());
            buf.extend_from_slice(&d.order.payload_b.to_le_bytes());
        }
        // Siege sections (optional).
        let empty_siege: [SiegeSectionSnapshot; 0] = [];
        let siege_sections = if let Some(s) = siege {
            s.sections.as_slice()
        } else {
            empty_siege.as_slice()
        };
        buf.extend_from_slice(&(siege_sections.len() as u32).to_le_bytes());
        for sec in siege_sections {
            buf.extend_from_slice(&sec.structure_id.to_le_bytes());
            buf.push(sec.face_idx);
            buf.extend_from_slice(&sec.base_integrity_q16.to_le_bytes());
            buf.extend_from_slice(&sec.integrity_q16.to_le_bytes());
            buf.extend_from_slice(&sec.breach_progress_q16.to_le_bytes());
            buf.extend_from_slice(&sec.sapper_attrition_q16.to_le_bytes());
            buf.push(sec.breached as u8);
            buf.extend_from_slice(&sec.generation.to_le_bytes());
        }
        let hash = hash64(prev_hash, &buf);
        buf.extend_from_slice(&hash.to_le_bytes());
        buf
    }

    pub fn deserialize_formations(
        &self,
        bytes: &[u8],
        world: &mut WorldSlabCapsule,
        prev_hash: u64,
    ) -> Option<(
        TelemetrySnapshot,
        crate::order::QueueStats,
        Vec<crate::formation::FormationSnapshot>,
        Vec<StructureSnapshot>,
        Option<StrategicPersistSnapshot>,
        Option<DiplomaticPersistSnapshot>,
        Option<EconomyPersistSnapshot>,
        Option<CampaignPersistSnapshot>,
        Option<CommandDelayPersistSnapshot>,
        Option<SiegePersistSnapshot>,
    )> {
        // Minimum length for versioned snapshots (header + queue stats + telemetry + footer).
        if bytes.len() < 156 {
            return None;
        }
        let footer_hash = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().ok()?);
        let body = &bytes[..bytes.len() - 8];
        if hash64(prev_hash, body) != footer_hash {
            return None;
        }

        fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
            if *cursor + 4 > bytes.len() {
                return None;
            }
            let v = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().ok()?);
            *cursor += 4;
            Some(v)
        }
        fn read_u64(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
            if *cursor + 8 > bytes.len() {
                return None;
            }
            let v = u64::from_le_bytes(bytes[*cursor..*cursor + 8].try_into().ok()?);
            *cursor += 8;
            Some(v)
        }
        fn read_u16(bytes: &[u8], cursor: &mut usize) -> Option<u16> {
            if *cursor + 2 > bytes.len() {
                return None;
            }
            let v = u16::from_le_bytes(bytes[*cursor..*cursor + 2].try_into().ok()?);
            *cursor += 2;
            Some(v)
        }
        fn read_u8(bytes: &[u8], cursor: &mut usize) -> Option<u8> {
            if *cursor + 1 > bytes.len() {
                return None;
            }
            let v = *bytes.get(*cursor)?;
            *cursor += 1;
            Some(v)
        }

        let mut cursor = 0;
        let magic = read_u32(body, &mut cursor)?;
        let version = read_u32(body, &mut cursor)?;
        if magic != SNAP_MAGIC || version < 3 || version > SNAP_VERSION {
            return None;
        }

        let formation_count = read_u32(body, &mut cursor)? as usize;
        let structure_count = if version >= 5 {
            read_u32(body, &mut cursor)? as usize
        } else {
            0
        };
        let head = read_u64(body, &mut cursor)?;
        let tail = read_u64(body, &mut cursor)?;
        let dropped = read_u64(body, &mut cursor)?;
        let capacity = read_u64(body, &mut cursor)?;

        let tele = if version >= 6 {
            TelemetrySnapshot {
                events: read_u64(body, &mut cursor)?,
                casualties: read_u64(body, &mut cursor)?,
                shock_weight_q16: read_u64(body, &mut cursor)?,
                ammo_spent: read_u64(body, &mut cursor)?,
                tick_last_flush: read_u64(body, &mut cursor)?,
                retreats: read_u64(body, &mut cursor)?,
                musket_shots: read_u64(body, &mut cursor)?,
                artillery_shots: read_u64(body, &mut cursor)?,
                formation_breaks: read_u64(body, &mut cursor)?,
                morale_shocks: read_u64(body, &mut cursor)?,
                supply_pressure_accum_q16: read_u64(body, &mut cursor)?,
                supply_fatigue_accum_q16: read_u64(body, &mut cursor)?,
                supply_samples: read_u64(body, &mut cursor)?,
                ai_orders: read_u64(body, &mut cursor)?,
                charge_orders: read_u64(body, &mut cursor)?,
                charge_commits: read_u64(body, &mut cursor)?,
                brace_orders: read_u64(body, &mut cursor)?,
            }
        } else if version >= 4 {
            TelemetrySnapshot {
                events: read_u64(body, &mut cursor)?,
                casualties: read_u64(body, &mut cursor)?,
                shock_weight_q16: read_u64(body, &mut cursor)?,
                ammo_spent: read_u64(body, &mut cursor)?,
                tick_last_flush: read_u64(body, &mut cursor)?,
                retreats: read_u64(body, &mut cursor)?,
                musket_shots: read_u64(body, &mut cursor)?,
                artillery_shots: read_u64(body, &mut cursor)?,
                formation_breaks: read_u64(body, &mut cursor)?,
                morale_shocks: read_u64(body, &mut cursor)?,
                supply_pressure_accum_q16: read_u64(body, &mut cursor)?,
                supply_fatigue_accum_q16: read_u64(body, &mut cursor)?,
                supply_samples: read_u64(body, &mut cursor)?,
                ai_orders: 0,
                charge_orders: read_u64(body, &mut cursor)?,
                charge_commits: read_u64(body, &mut cursor)?,
                brace_orders: read_u64(body, &mut cursor)?,
            }
        } else {
            TelemetrySnapshot {
                events: read_u64(body, &mut cursor)?,
                casualties: read_u64(body, &mut cursor)?,
                shock_weight_q16: read_u64(body, &mut cursor)?,
                ammo_spent: read_u64(body, &mut cursor)?,
                tick_last_flush: read_u64(body, &mut cursor)?,
                retreats: read_u64(body, &mut cursor)?,
                musket_shots: read_u64(body, &mut cursor)?,
                artillery_shots: read_u64(body, &mut cursor)?,
                formation_breaks: read_u64(body, &mut cursor)?,
                morale_shocks: read_u64(body, &mut cursor)?,
                ai_orders: 0,
                charge_orders: read_u64(body, &mut cursor)?,
                charge_commits: read_u64(body, &mut cursor)?,
                brace_orders: read_u64(body, &mut cursor)?,
                supply_pressure_accum_q16: 0,
                supply_fatigue_accum_q16: 0,
                supply_samples: 0,
            }
        };

        let mut formations_out = Vec::with_capacity(formation_count);
        for _ in 0..formation_count {
            let formation_id = read_u32(body, &mut cursor)?;
            let posture = read_u8(body, &mut cursor)?;
            let stance = read_u8(body, &mut cursor)?;
            let generation = read_u32(body, &mut cursor)?;
            let cohesion_q16 = read_u32(body, &mut cursor)?;
            let fatigue_q16 = read_u32(body, &mut cursor)?;
            let ammo = read_u32(body, &mut cursor)?;
            let morale_q16 = read_u32(body, &mut cursor)?;
            let facing_deg_q16 = read_u32(body, &mut cursor)?;
            let position_x_q16 = read_u32(body, &mut cursor)?;
            let position_z_q16 = read_u32(body, &mut cursor)?;
            let command_delay_ms = read_u32(body, &mut cursor)?;
            let retreat_mode_flags = read_u16(body, &mut cursor)?;
            let charge_posture = read_u8(body, &mut cursor)?;
            let braced = read_u8(body, &mut cursor)? != 0;
            let (density_q16, mass_q16, variance_q16, damping_q16, velocity_q16, physics_flags) =
                if version >= 3 {
                    (
                        read_u32(body, &mut cursor)?,
                        read_u32(body, &mut cursor)?,
                        read_u32(body, &mut cursor)?,
                        read_u32(body, &mut cursor)?,
                        read_u32(body, &mut cursor)?,
                        read_u16(body, &mut cursor)?,
                    )
                } else {
                    (40_000, 44_000, 28_000, 12_000, 24_000, 0)
                };
            let snap = crate::formation::FormationSnapshot {
                formation_id,
                posture,
                stance,
                generation,
                cohesion_q16,
                fatigue_q16,
                ammo,
                morale_q16,
                facing_deg_q16,
                position_x_q16,
                position_z_q16,
                command_delay_ms,
                retreat_mode_flags,
                charge_posture,
                braced,
                density_q16,
                mass_q16,
                variance_q16,
                damping_q16,
                velocity_q16,
                physics_flags,
                gap_close_q16: 65_536,
                rank_variance_scale_q16: 65_536,
                gap_fatigue_penalty_q16: crate::formation::gap_fatigue_penalty(65_536),
            };
            formations_out.push(snap);
            world.push_unit(crate::world::UnitCapsule::from_snapshot(
                crate::world::UnitSnapshot {
                    pos_x_q16: position_x_q16,
                    pos_z_q16: position_z_q16,
                    heading_deg_q16: facing_deg_q16,
                    regiment_id: formation_id as u16,
                    state: 0,
                    bloodiness: 0,
                    generation,
                },
            ));
        }

        let mut structures_out = Vec::with_capacity(structure_count);
        for _ in 0..structure_count {
            let structure_id = read_u32(body, &mut cursor)?;
            let position_x_q16 = read_u32(body, &mut cursor)?;
            let position_z_q16 = read_u32(body, &mut cursor)?;
            let half_extent_x_q16 = read_u32(body, &mut cursor)?;
            let half_extent_z_q16 = read_u32(body, &mut cursor)?;
            let mut cover_q16 = [0u32; 4];
            for face in &mut cover_q16 {
                *face = read_u32(body, &mut cursor)?;
            }
            let breach_mask = read_u32(body, &mut cursor)?;
            let slots_total = read_u32(body, &mut cursor)? as u16;
            let slots_used = read_u32(body, &mut cursor)? as u16;
            let apertures = read_u32(body, &mut cursor)? as u16;
            let generation = read_u32(body, &mut cursor)?;

            structures_out.push(StructureSnapshot {
                structure_id,
                position_x_q16,
                position_z_q16,
                half_extent_x_q16,
                half_extent_z_q16,
                cover_q16,
                breach_mask,
                slots_total,
                slots_used,
                apertures,
                generation,
            });
        }
        let strategic = if version >= 7 {
            let tick = read_u64(body, &mut cursor)?;
            let generation = read_u64(body, &mut cursor)?;
            let hash_chain = read_u64(body, &mut cursor)?;
            let prev_hash_chain = read_u64(body, &mut cursor)?;
            let event_count = read_u32(body, &mut cursor)? as usize;
            let mut events = Vec::with_capacity(event_count);
            for _ in 0..event_count {
                let kind_raw = read_u8(body, &mut cursor)?;
                let province_id = read_u32(body, &mut cursor)?;
                let from_owner_id = read_u32(body, &mut cursor)?;
                let to_owner_id = read_u32(body, &mut cursor)?;
                let from_infra_q16 = read_u32(body, &mut cursor)?;
                let to_infra_q16 = read_u32(body, &mut cursor)?;
                let resistance_q16 = read_u32(body, &mut cursor)?;
                let generation_ev = read_u64(body, &mut cursor)?;
                let kind =
                    StrategicEventKind::from_u8(kind_raw).unwrap_or(StrategicEventKind::OwnershipChange);
                events.push(StrategicEventSnapshot {
                    kind,
                    province_id,
                    from_owner_id,
                    to_owner_id,
                    from_infra_q16,
                    to_infra_q16,
                    resistance_q16,
                    generation: generation_ev,
                });
            }
            if tick == 0
                && generation == 0
                && hash_chain == 0
                && prev_hash_chain == 0
                && events.is_empty()
            {
                None
            } else {
                Some(StrategicPersistSnapshot {
                    tick,
                    generation,
                    hash_chain,
                    prev_hash_chain,
                    events,
                })
            }
        } else {
            None
        };

        let diplomatic = if version >= 8 {
            let tick = read_u64(body, &mut cursor)?;
            let generation = read_u64(body, &mut cursor)?;
            let hash_chain = read_u64(body, &mut cursor)?;
            let prev_hash_chain = read_u64(body, &mut cursor)?;
            let relation_count = read_u32(body, &mut cursor)? as usize;
            let mut relations = Vec::with_capacity(relation_count);
            for _ in 0..relation_count {
                let state_raw = read_u8(body, &mut cursor)?;
                let a = read_u16(body, &mut cursor)?;
                let b = read_u16(body, &mut cursor)?;
                let truce_until = read_u64(body, &mut cursor)?;
                let cb_until = read_u64(body, &mut cursor)?;
                let war_exhaustion_q16 = read_u32(body, &mut cursor)?;
                let generation_rel = read_u64(body, &mut cursor)?;
                let state =
                    crate::diplomacy::DiplomaticState::from_u8(state_raw).unwrap_or(crate::diplomacy::DiplomaticState::Peace);
                relations.push(DiplomaticRelationSnapshot {
                    a,
                    b,
                    state,
                    truce_until_tick: truce_until,
                    casus_belli_until_tick: cb_until,
                    war_exhaustion_q16,
                    generation: generation_rel,
                });
            }
            if tick == 0
                && generation == 0
                && hash_chain == 0
                && prev_hash_chain == 0
                && relations.is_empty()
            {
                None
            } else {
                Some(DiplomaticPersistSnapshot {
                    tick,
                    generation,
                    hash_chain,
                    prev_hash_chain,
                    relations,
                })
            }
        } else {
            None
        };

        let economy = if version >= 9 {
            let tick = read_u64(body, &mut cursor)?;
            let generation = read_u64(body, &mut cursor)?;
            let hash_chain = read_u64(body, &mut cursor)?;
            let prev_hash_chain = read_u64(body, &mut cursor)?;
            let order_count = read_u32(body, &mut cursor)? as usize;
            let mut orders = Vec::with_capacity(order_count);
            for _ in 0..order_count {
                let kind_raw = read_u8(body, &mut cursor)?;
                let province_id = read_u32(body, &mut cursor)?;
                let target_infra_q16 = read_u32(body, &mut cursor)?;
                let remaining_ticks = read_u32(body, &mut cursor)?;
                let generation_order = read_u64(body, &mut cursor)?;
                let kind = crate::province_economy::BuildOrderKind::from_u8(kind_raw)
                    .unwrap_or(crate::province_economy::BuildOrderKind::Infrastructure);
                orders.push(BuildOrderSnapshot {
                    province_id,
                    kind,
                    target_infra_q16,
                    remaining_ticks,
                    generation: generation_order,
                });
            }
            if tick == 0
                && generation == 0
                && hash_chain == 0
                && prev_hash_chain == 0
                && orders.is_empty()
            {
                None
            } else {
                Some(EconomyPersistSnapshot {
                    tick,
                    generation,
                    hash_chain,
                    prev_hash_chain,
                    orders,
                })
            }
        } else {
            None
        };

        let campaign = if version >= 12 {
            let tick = read_u64(body, &mut cursor)?;
            let generation = read_u64(body, &mut cursor)?;
            let hash_chain = read_u64(body, &mut cursor)?;
            let prev_hash_chain = read_u64(body, &mut cursor)?;
            let war_exhaustion_avg_q16 = read_u32(body, &mut cursor)?;
            if tick == 0 && generation == 0 && hash_chain == 0 {
                None
            } else {
                Some(CampaignPersistSnapshot {
                    tick,
                    generation,
                    hash_chain,
                    prev_hash_chain,
                    war_exhaustion_avg_q16,
                })
            }
        } else {
            None
        };

        let command_delays = if version >= 10 {
            let delay_count = read_u32(body, &mut cursor)? as usize;
            let mut pending = Vec::with_capacity(delay_count);
            for _ in 0..delay_count {
                let ready_tick = read_u64(body, &mut cursor)?;
                let kind_raw = read_u8(body, &mut cursor)?;
                let formation_id = read_u32(body, &mut cursor)?;
                let generation_order = read_u32(body, &mut cursor)?;
                let payload_a = read_u64(body, &mut cursor)?;
                let payload_b = read_u64(body, &mut cursor)?;
                let kind = crate::order::OrderKind::from_u8(kind_raw)
                    .unwrap_or(crate::order::OrderKind::Hold);
                pending.push(CommandDelaySnapshot {
                    ready_tick,
                    order: crate::order::OrderData {
                        kind,
                        formation_id,
                        generation: generation_order,
                        payload_a,
                        payload_b,
                    },
                });
            }
            if pending.is_empty() {
                None
            } else {
                Some(CommandDelayPersistSnapshot { pending })
            }
        } else {
            None
        };

        let siege = if version >= 11 {
            let section_count = read_u32(body, &mut cursor)? as usize;
            let mut sections = Vec::with_capacity(section_count);
            for _ in 0..section_count {
                let structure_id = read_u32(body, &mut cursor)?;
                let face_idx = read_u8(body, &mut cursor)? as u8;
                let base_integrity_q16 = read_u32(body, &mut cursor)?;
                let integrity_q16 = read_u32(body, &mut cursor)?;
                let breach_progress_q16 = read_u32(body, &mut cursor)?;
                let sapper_attrition_q16 = read_u32(body, &mut cursor)?;
                let breached = read_u8(body, &mut cursor)? != 0;
                let generation = read_u32(body, &mut cursor)?;
                sections.push(SiegeSectionSnapshot {
                    structure_id,
                    face_idx,
                    base_integrity_q16,
                    integrity_q16,
                    breach_progress_q16,
                    sapper_attrition_q16,
                    breached,
                    generation,
                });
            }
            if sections.is_empty() {
                None
            } else {
                Some(SiegePersistSnapshot { sections })
            }
        } else {
            None
        };

        Some((
            tele,
            crate::order::QueueStats {
                head,
                tail,
                dropped,
                capacity,
            },
            formations_out,
            structures_out,
            strategic,
            diplomatic,
            economy,
            campaign,
            command_delays,
            siege,
        ))
    }

    /// Deserialize a snapshot and optionally restore a command delay buffer in-place.
    ///
    /// Returns the same tuple as `deserialize_formations` plus the number of restored delayed
    /// orders written into `command_delays` (if provided).
    pub fn deserialize_into_world(
        &self,
        bytes: &[u8],
        world: &mut WorldSlabCapsule,
        prev_hash: u64,
        command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    ) -> Option<(
        TelemetrySnapshot,
        crate::order::QueueStats,
        Vec<crate::formation::FormationSnapshot>,
        Vec<StructureSnapshot>,
        Option<StrategicPersistSnapshot>,
        Option<DiplomaticPersistSnapshot>,
        Option<EconomyPersistSnapshot>,
        Option<CampaignPersistSnapshot>,
        Option<CommandDelayPersistSnapshot>,
        Option<SiegePersistSnapshot>,
        usize,
    )> {
        let decoded = self.deserialize_formations(bytes, world, prev_hash)?;
        let restored_count = if let Some(buf) = command_delays {
            restore_command_delays(decoded.8.as_ref(), buf)
        } else {
            0
        };
        Some((
            decoded.0,
            decoded.1,
            decoded.2,
            decoded.3,
            decoded.4,
            decoded.5,
            decoded.6,
            decoded.7, // campaign
            decoded.8, // command delays
            decoded.9, // siege
            restored_count,
        ))
    }
}

fn hash64(prev: u64, bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = prev ^ FNV_OFFSET;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Mmap-backed snapshot persistence capsule with hash chaining.
#[repr(C, align(128))]
pub struct SnapshotMmapCapsule {
    manager: MmapManager,
    region_idx: usize,
    hash_chain: AtomicU64,
    last_offset: AtomicU64,
    _padding: [u8; 64],
}

impl SnapshotMmapCapsule {
    pub fn new(
        path: &std::path::Path,
        file_size: u64,
        region_count: usize,
    ) -> Result<Self, MmapError> {
        let layout = MmapLayout::new(file_size, region_count)?;
        let manager = MmapManager::new(path, &layout)?;
        Ok(Self {
            manager,
            region_idx: 0,
            hash_chain: AtomicU64::new(0xDEAD_BEEF_F00D_F00D),
            last_offset: AtomicU64::new(0),
            _padding: [0; 64],
        })
    }

    /// Append a serialized snapshot into mmap and update hash chain.
    pub fn append(&mut self, snapshot_bytes: &[u8], prev_hash: u64) -> Result<u64, MmapError> {
        if snapshot_bytes.is_empty() {
            return Ok(self.hash_chain.load(Ordering::Relaxed));
        }
        let len = snapshot_bytes.len() as u32;
        let region = self.manager.region(self.region_idx).ok_or_else(|| {
            MmapError::invalid_region_index(self.region_idx, self.manager.region_count())
        })?;
        let offset = region.allocate(len)?;
        let ptr = unsafe { self.manager.ptr_at_offset(offset as u64)? };
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr, len as usize) };
        buf.copy_from_slice(snapshot_bytes);
        self.manager.fsync()?;
        let chain = hash64(prev_hash, snapshot_bytes);
        self.hash_chain.store(chain, Ordering::Release);
        self.last_offset.store(offset as u64, Ordering::Release);
        Ok(chain)
    }

    /// Append and immediately verify the hash chain against stored bytes.
    /// Returns (chain_hash, offset, len) on success.
    pub fn append_and_verify(
        &mut self,
        snapshot_bytes: &[u8],
        prev_hash: u64,
    ) -> Result<(u64, u32, u32), MmapError> {
        let chain = self.append(snapshot_bytes, prev_hash)?;
        let offset = self.last_offset.load(Ordering::Acquire) as u32;
        let len = snapshot_bytes.len() as u32;
        let ok = self.verify(offset, len, prev_hash)?;
        if !ok {
            return Err(MmapError::GenerationMismatch {
                expected: chain,
                actual: 0,
            });
        }
        Ok((chain, offset, len))
    }

    /// Verify a snapshot slice at offset/len against prev_hash; returns Ok(true) if intact.
    pub fn verify(&self, offset: u32, len: u32, prev_hash: u64) -> Result<bool, MmapError> {
        let ptr = unsafe { self.manager.ptr_at_offset(offset.into())? };
        let buf = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        if len < 8 {
            return Ok(false);
        }
        let body = &buf[..len as usize - 8];
        let footer = u64::from_le_bytes(buf[len as usize - 8..len as usize].try_into().unwrap());
        if hash64(prev_hash, body) != footer {
            return Ok(false);
        }
        let chain = hash64(prev_hash, buf);
        Ok(chain == self.hash_chain.load(Ordering::Relaxed))
    }

    pub fn hash_chain(&self) -> u64 {
        self.hash_chain.load(Ordering::Relaxed)
    }

    pub fn last_offset(&self) -> u64 {
        self.last_offset.load(Ordering::Relaxed)
    }

    /// Load and verify a snapshot slice; returns Ok(Some(bytes)) if hash chain matches.
    pub fn load_verified(
        &self,
        offset: u32,
        len: u32,
        prev_hash: u64,
    ) -> Result<Option<Vec<u8>>, MmapError> {
        let ptr = unsafe { self.manager.ptr_at_offset(offset.into())? };
        let buf = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        if len < 8 {
            return Ok(None);
        }
        let body = &buf[..len as usize - 8];
        let footer = u64::from_le_bytes(buf[len as usize - 8..len as usize].try_into().unwrap());
        if hash64(prev_hash, body) != footer {
            return Ok(None);
        }
        Ok(Some(buf.to_vec()))
    }
}

verify_capsule_properties!(SnapshotMmapCapsule, 128, 256);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diplomacy::DiplomaticStateCapsule;
    use crate::formation::FormationCapsule;
    use crate::telemetry::TelemetryCapsule;

    #[test]
    fn snapshot_roundtrip_basic() {
        let formations = vec![
            FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0),
            FormationCapsule::new(2, 0, 0, 10, 10, 0, 0, 0, 0, 0),
        ];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let snapper = CampaignSnapshotCapsule::new();
        let buf = snapper.serialize(
            &formations,
            &orders,
            &telemetry,
            &[],
            None,
            None,
            None,
            None,
            None,
            None,
            0,
        );
        let mut world = WorldSlabCapsule::new(1);
        let (tele, stats, forms, structures, strat, dip, econ, campaign, delays, siege) =
            snapper.deserialize_formations(&buf, &mut world, 0).unwrap();
        assert_eq!(tele.events, 0);
        assert_eq!(world.len(), 2);
        assert_eq!(stats.head, 0);
        assert_eq!(stats.tail, 0);
        assert_eq!(forms.len(), 2);
        assert!(structures.is_empty());
        assert!(strat.is_none());
        assert!(dip.is_none());
        assert!(econ.is_none());
        assert!(campaign.is_none());
        assert!(delays.is_none());
        assert!(siege.is_none());
    }

    #[test]
    fn diplomatic_snapshot_roundtrips() {
        let formations = vec![FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let dip = DiplomaticStateCapsule::new(3);
        dip.set_war(0, 1);
        dip.set_casus_belli(1, 2, 120);
        let dip_snap = dip.snapshot(10);
        let snapper = CampaignSnapshotCapsule::new();
        let buf = snapper.serialize(
            &formations,
            &orders,
            &telemetry,
            &[],
            None,
            Some(&dip_snap),
            None,
            None,
            None,
            None,
            0,
        );
        let mut world = WorldSlabCapsule::new(1);
        let (_tele, _stats, _forms, _structs, strat, dip_out, econ_out, campaign_out, delays_out, siege_out) =
            snapper.deserialize_formations(&buf, &mut world, 0).unwrap();
        assert!(strat.is_none());
        assert!(campaign_out.is_none());
        let dip_decoded = dip_out.expect("diplomatic snapshot");
        assert_eq!(dip_decoded.tick, dip_snap.tick);
        assert_eq!(dip_decoded.hash_chain, dip_snap.hash_chain);
        assert_eq!(dip_decoded.relations.len(), dip_snap.relations.len());
        assert_eq!(dip_decoded.relations[0].state, dip_snap.relations[0].state);
        assert!(econ_out.is_none());
        assert!(delays_out.is_none());
        assert!(siege_out.is_none());
    }

    #[test]
    fn command_delay_buffer_persists() {
        use crate::order::{pack_move_payload, CommandDelayBufferCapsule, OrderKind};
        let formations = vec![FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let delays = CommandDelayBufferCapsule::new();
        let order = crate::order::OrderData {
            kind: OrderKind::Move,
            formation_id: 0,
            generation: 1,
            payload_a: pack_move_payload(10, 0),
            payload_b: 0,
        };
        assert!(delays.enqueue(&order, 42));
        let snapper = CampaignSnapshotCapsule::new();
        let buf = snapper.serialize(
            &formations,
            &orders,
            &telemetry,
            &[],
            None,
            None,
            None,
            None,
            Some(&delays),
            None,
            0,
        );
        let mut world = WorldSlabCapsule::new(1);
        let (_tele, _stats, _forms, _structs, _strat, _dip, _econ, _camp, delays_snap, siege_snap) =
            snapper.deserialize_formations(&buf, &mut world, 0).unwrap();
        let delays_snap = delays_snap.expect("delays snapshot");
        assert_eq!(delays_snap.pending.len(), 1);
        assert_eq!(delays_snap.pending[0].ready_tick, 42);
        assert_eq!(delays_snap.pending[0].order.kind, OrderKind::Move);
        assert!(siege_snap.is_none());

        // Restore into a fresh buffer and drain at the ready tick.
        let restored = CommandDelayBufferCapsule::new();
        let count = crate::snapshot::restore_command_delays(Some(&delays_snap), &restored);
        assert_eq!(count, 1);
        let mut out = Vec::new();
        restored.drain_ready(42, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, OrderKind::Move);
    }

    #[test]
    fn deserialize_into_world_restores_command_delays() {
        use crate::order::{pack_move_payload, CommandDelayBufferCapsule, OrderKind};
        let formations = vec![FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let delays = CommandDelayBufferCapsule::new();
        let order = crate::order::OrderData {
            kind: OrderKind::Move,
            formation_id: 0,
            generation: 1,
            payload_a: pack_move_payload(10, 0),
            payload_b: 0,
        };
        assert!(delays.enqueue(&order, 7));
        let snapper = CampaignSnapshotCapsule::new();
        let buf = snapper.serialize(
            &formations,
            &orders,
            &telemetry,
            &[],
            None,
            None,
            None,
            None,
            Some(&delays),
            None,
            0,
        );
        let mut world = WorldSlabCapsule::new(1);
        let restored_buffer = CommandDelayBufferCapsule::new();
        let (
            _tele,
            _stats,
            _forms,
            _structs,
            _strat,
            _dip,
            _econ,
            _campaign,
            delays_snap,
            siege_snap,
            restored_count,
        ) = snapper
            .deserialize_into_world(&buf, &mut world, 0, Some(&restored_buffer))
            .expect("snapshot decode");
        assert_eq!(restored_count, 1);
        assert!(delays_snap.is_some());
        assert!(siege_snap.is_none());
        let mut out = Vec::new();
        restored_buffer.drain_ready(7, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, OrderKind::Move);
    }

    #[test]
    fn snapshot_mmap_round_trip_and_tamper_detection() {
        use std::path::Path;
        let formations = vec![FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let snapper = CampaignSnapshotCapsule::new();
        let buf =
            snapper.serialize(&formations, &orders, &telemetry, &[], None, None, None, None, None, None, 0);

        let tmp_path = std::env::temp_dir().join("kindly_engine_snapshot.bin");
        let mut mmap_capsule =
            SnapshotMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create snapshot mmap");
        let chain = mmap_capsule.append(&buf, 0).expect("append snapshot");
        assert_ne!(chain, 0);
        let verified = mmap_capsule
            .verify(mmap_capsule.last_offset() as u32, buf.len() as u32, 0)
            .expect("verify");
        assert!(verified);

        // Tamper with file bytes.
        std::fs::write(&tmp_path, &[0u8; 32]).unwrap();
        let tampered = mmap_capsule
            .verify(mmap_capsule.last_offset() as u32, buf.len() as u32, 0)
            .unwrap_or(true);
        assert!(!tampered);
        let _ = std::fs::remove_file(Path::new(&tmp_path));
    }

    #[test]
    fn append_and_verify_chain_is_intact() {
        let formations = vec![FormationCapsule::new(3, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let snapper = CampaignSnapshotCapsule::new();
        let buf =
            snapper.serialize(&formations, &orders, &telemetry, &[], None, None, None, None, None, None, 123);
        let tmp_path = std::env::temp_dir().join("kindly_engine_snapshot2.bin");
        let mut mmap_capsule =
            SnapshotMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create snapshot mmap");
        let (chain, offset, len) = mmap_capsule
            .append_and_verify(&buf, 123)
            .expect("append + verify");
        assert_eq!(offset, mmap_capsule.last_offset() as u32);
        assert_eq!(len as usize, buf.len());
        assert_eq!(chain, mmap_capsule.hash_chain());
        let _ = std::fs::remove_file(&tmp_path);
    }
}
