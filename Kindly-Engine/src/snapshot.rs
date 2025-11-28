use crate::formation::FormationCapsule;
use crate::order::OrderQueueCapsule;
use crate::telemetry::{TelemetryCapsule, TelemetrySnapshot};
use crate::world::WorldSlabCapsule;
use atomic_capsule::mmap::{MmapError, MmapLayout, MmapManager};
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

const SNAP_MAGIC: u32 = 0x574C4457; // "WLDW"
const SNAP_VERSION: u32 = 4;

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
        prev_hash: u64,
    ) -> Vec<u8> {
        let stats = orders.stats();
        let tele = telemetry.snapshot();
        let mut buf = Vec::with_capacity(64 + formations.len() * 64);
        buf.extend_from_slice(&SNAP_MAGIC.to_le_bytes());
        buf.extend_from_slice(&SNAP_VERSION.to_le_bytes());
        buf.extend_from_slice(&(formations.len() as u32).to_le_bytes());
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
    )> {
        // Minimum length for version 3 snapshots (header + queue stats + telemetry + footer).
        if bytes.len() < 156 {
            return None;
        }
        let footer_hash = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().ok()?);
        let body = &bytes[..bytes.len() - 8];
        if hash64(prev_hash, body) != footer_hash {
            return None;
        }
        let magic = u32::from_le_bytes(body[0..4].try_into().ok()?);
        let version = u32::from_le_bytes(body[4..8].try_into().ok()?);
        if magic != SNAP_MAGIC || version != SNAP_VERSION {
            return None;
        }
        let formation_count = u32::from_le_bytes(body[8..12].try_into().ok()?) as usize;
        let head = u64::from_le_bytes(body[12..20].try_into().ok()?);
        let tail = u64::from_le_bytes(body[20..28].try_into().ok()?);
        let dropped = u64::from_le_bytes(body[28..36].try_into().ok()?);
        let capacity = u64::from_le_bytes(body[36..44].try_into().ok()?);
        let tele = if version >= 4 {
            // Version 4 includes supply accumulators.
            if body.len() < 172 {
                return None;
            }
            TelemetrySnapshot {
                events: u64::from_le_bytes(body[44..52].try_into().ok()?),
                casualties: u64::from_le_bytes(body[52..60].try_into().ok()?),
                shock_weight_q16: u64::from_le_bytes(body[60..68].try_into().ok()?),
                ammo_spent: u64::from_le_bytes(body[68..76].try_into().ok()?),
                tick_last_flush: u64::from_le_bytes(body[76..84].try_into().ok()?),
                retreats: u64::from_le_bytes(body[84..92].try_into().ok()?),
                musket_shots: u64::from_le_bytes(body[92..100].try_into().ok()?),
                artillery_shots: u64::from_le_bytes(body[100..108].try_into().ok()?),
                formation_breaks: u64::from_le_bytes(body[108..116].try_into().ok()?),
                morale_shocks: u64::from_le_bytes(body[116..124].try_into().ok()?),
                supply_pressure_accum_q16: u64::from_le_bytes(body[124..132].try_into().ok()?),
                supply_fatigue_accum_q16: u64::from_le_bytes(body[132..140].try_into().ok()?),
                supply_samples: u64::from_le_bytes(body[140..148].try_into().ok()?),
                charge_orders: u64::from_le_bytes(body[148..156].try_into().ok()?),
                charge_commits: u64::from_le_bytes(body[156..164].try_into().ok()?),
                brace_orders: u64::from_le_bytes(body[164..172].try_into().ok()?),
            }
        } else {
            TelemetrySnapshot {
                events: u64::from_le_bytes(body[44..52].try_into().ok()?),
                casualties: u64::from_le_bytes(body[52..60].try_into().ok()?),
                shock_weight_q16: u64::from_le_bytes(body[60..68].try_into().ok()?),
                ammo_spent: u64::from_le_bytes(body[68..76].try_into().ok()?),
                tick_last_flush: u64::from_le_bytes(body[76..84].try_into().ok()?),
                retreats: u64::from_le_bytes(body[84..92].try_into().ok()?),
                musket_shots: u64::from_le_bytes(body[92..100].try_into().ok()?),
                artillery_shots: u64::from_le_bytes(body[100..108].try_into().ok()?),
                formation_breaks: u64::from_le_bytes(body[108..116].try_into().ok()?),
                morale_shocks: u64::from_le_bytes(body[116..124].try_into().ok()?),
                charge_orders: u64::from_le_bytes(body[124..132].try_into().ok()?),
                charge_commits: u64::from_le_bytes(body[132..140].try_into().ok()?),
                brace_orders: u64::from_le_bytes(body[140..148].try_into().ok()?),
                supply_pressure_accum_q16: 0,
                supply_fatigue_accum_q16: 0,
                supply_samples: 0,
            }
        };

        let mut cursor = if version >= 4 { 172 } else { 148 };
        let mut formations_out = Vec::with_capacity(formation_count);
        for _ in 0..formation_count {
            if cursor + 46 > body.len() {
                return None;
            }
            let formation_id = u32::from_le_bytes(body[cursor..cursor + 4].try_into().ok()?);
            let posture = body[cursor + 4];
            let stance = body[cursor + 5];
            let generation = u32::from_le_bytes(body[cursor + 6..cursor + 10].try_into().ok()?);
            let cohesion_q16 = u32::from_le_bytes(body[cursor + 10..cursor + 14].try_into().ok()?);
            let fatigue_q16 = u32::from_le_bytes(body[cursor + 14..cursor + 18].try_into().ok()?);
            let ammo = u32::from_le_bytes(body[cursor + 18..cursor + 22].try_into().ok()?);
            let morale_q16 = u32::from_le_bytes(body[cursor + 22..cursor + 26].try_into().ok()?);
            let facing_deg_q16 =
                u32::from_le_bytes(body[cursor + 26..cursor + 30].try_into().ok()?);
            let position_x_q16 =
                u32::from_le_bytes(body[cursor + 30..cursor + 34].try_into().ok()?);
            let position_z_q16 =
                u32::from_le_bytes(body[cursor + 34..cursor + 38].try_into().ok()?);
            let command_delay_ms =
                u32::from_le_bytes(body[cursor + 38..cursor + 42].try_into().ok()?);
            let retreat_mode_flags =
                u16::from_le_bytes(body[cursor + 42..cursor + 44].try_into().ok()?);
            let charge_posture = *body.get(cursor + 44)?;
            let braced = *body.get(cursor + 45)? != 0;
            cursor += 46;
            let (
                density_q16,
                mass_q16,
                variance_q16,
                damping_q16,
                velocity_q16,
                physics_flags,
                consumed,
            ) = if version >= 3 {
                if cursor + 20 + 2 > body.len() {
                    return None;
                }
                (
                    u32::from_le_bytes(body[cursor..cursor + 4].try_into().ok()?),
                    u32::from_le_bytes(body[cursor + 4..cursor + 8].try_into().ok()?),
                    u32::from_le_bytes(body[cursor + 8..cursor + 12].try_into().ok()?),
                    u32::from_le_bytes(body[cursor + 12..cursor + 16].try_into().ok()?),
                    u32::from_le_bytes(body[cursor + 16..cursor + 20].try_into().ok()?),
                    u16::from_le_bytes(body[cursor + 20..cursor + 22].try_into().ok()?),
                    22,
                )
            } else {
                // Backward compatibility: default to line infantry physics.
                (40_000, 44_000, 28_000, 12_000, 24_000, 0, 0)
            };
            cursor += consumed;
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
        Some((
            tele,
            crate::order::QueueStats {
                head,
                tail,
                dropped,
                capacity,
            },
            formations_out,
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
        let buf = snapper.serialize(&formations, &orders, &telemetry, 0);
        let mut world = WorldSlabCapsule::new(1);
        let (tele, stats, forms) = snapper.deserialize_formations(&buf, &mut world, 0).unwrap();
        assert_eq!(tele.events, 0);
        assert_eq!(world.len(), 2);
        assert_eq!(stats.head, 0);
        assert_eq!(stats.tail, 0);
        assert_eq!(forms.len(), 2);
    }

    #[test]
    fn snapshot_mmap_round_trip_and_tamper_detection() {
        use std::path::Path;
        let formations = vec![FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let orders = OrderQueueCapsule::new();
        let telemetry = TelemetryCapsule::new();
        let snapper = CampaignSnapshotCapsule::new();
        let buf = snapper.serialize(&formations, &orders, &telemetry, 0);

        let tmp_path = std::env::temp_dir().join("kindly_engine_snapshot.bin");
        let mmap_capsule =
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
        let buf = snapper.serialize(&formations, &orders, &telemetry, 123);
        let tmp_path = std::env::temp_dir().join("kindly_engine_snapshot2.bin");
        let mmap_capsule =
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
