use crate::fire_doctrine::FireDoctrineMode;
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::array::from_fn;
use core::sync::atomic::{AtomicU64, Ordering};

/// Order payload capsule (written by single producer, read by single consumer).
///
/// Uses a simple versioned header (even = committed) to prevent torn reads.
#[repr(C, align(64))]
pub struct OrderCapsule {
    header: AtomicU64,
    payload_a: AtomicU64,
    payload_b: AtomicU64,
    _padding: [u8; 40],
}

impl OrderCapsule {
    pub const fn new() -> Self {
        Self {
            header: AtomicU64::new(0),
            payload_a: AtomicU64::new(0),
            payload_b: AtomicU64::new(0),
            _padding: [0; 40],
        }
    }

    pub fn write(&self, kind: OrderKind, formation_id: u32, payload_a: u64, payload_b: u64) {
        // version bump odd -> even for commit-flip
        let current = self.header.load(Ordering::Relaxed);
        let gen = ((current >> 32) & 0xFFFFFF) as u32;
        let inflight = pack_header(kind, formation_id, gen + 1, true);

        self.payload_a.store(payload_a, Ordering::Relaxed);
        self.payload_b.store(payload_b, Ordering::Relaxed);
        self.header.store(inflight, Ordering::Release);

        let committed = pack_header(kind, formation_id, gen + 2, false);
        self.header.store(committed, Ordering::Release);
    }

    pub fn read(&self) -> Option<OrderData> {
        let header = self.header.load(Ordering::Acquire);
        if header & INFLIGHT_MASK != 0 {
            return None; // odd = inflight
        }
        let kind = unpack_kind(header);
        let formation_id = unpack_formation_id(header);
        let generation = ((header >> 32) & 0xFFFFFF) as u32;
        let payload_a = self.payload_a.load(Ordering::Acquire);
        let payload_b = self.payload_b.load(Ordering::Acquire);

        Some(OrderData {
            kind,
            formation_id,
            generation,
            payload_a,
            payload_b,
        })
    }
}

verify_capsule_properties!(OrderCapsule, 64, 64);

/// Delay slot for command-latency buffering (ready_tick = u64::MAX means empty).
#[repr(C, align(64))]
pub struct CommandDelaySlot {
    header: AtomicU64,
    payload_a: AtomicU64,
    payload_b: AtomicU64,
    ready_tick: AtomicU64,
}

impl CommandDelaySlot {
    const EMPTY: u64 = u64::MAX;

    pub const fn new() -> Self {
        Self {
            header: AtomicU64::new(0),
            payload_a: AtomicU64::new(0),
            payload_b: AtomicU64::new(0),
            ready_tick: AtomicU64::new(Self::EMPTY),
        }
    }

    pub fn write(&self, order: &OrderData, ready_tick: u64) {
        let hdr = pack_header(order.kind, order.formation_id, order.generation, false);
        self.payload_a.store(order.payload_a, Ordering::Relaxed);
        self.payload_b.store(order.payload_b, Ordering::Relaxed);
        self.header.store(hdr, Ordering::Release);
        self.ready_tick.store(ready_tick, Ordering::Release);
    }

    pub fn take_if_ready(&self, now_tick: u64) -> Option<OrderData> {
        let ready = self.ready_tick.load(Ordering::Acquire);
        if ready == Self::EMPTY || ready > now_tick {
            return None;
        }
        let header = self.header.load(Ordering::Relaxed);
        let kind = unpack_kind(header);
        let formation_id = unpack_formation_id(header);
        let generation = ((header >> 32) & 0xFFFFFF) as u32;
        let payload_a = self.payload_a.load(Ordering::Relaxed);
        let payload_b = self.payload_b.load(Ordering::Relaxed);
        self.ready_tick.store(Self::EMPTY, Ordering::Release);
        Some(OrderData {
            kind,
            formation_id,
            generation,
            payload_a,
            payload_b,
        })
    }
}

/// Command delay buffer capsule: staged orders gated by ready_tick.
#[repr(C, align(128))]
pub struct CommandDelayBufferCapsule {
    slots: [CommandDelaySlot; ORDER_QUEUE_CAPACITY],
}

/// Snapshot of a delayed command (ready at tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDelaySnapshot {
    pub ready_tick: u64,
    pub order: OrderData,
}

impl CommandDelayBufferCapsule {
    pub fn new() -> Self {
        Self {
            slots: from_fn(|_| CommandDelaySlot::new()),
        }
    }

    /// Enqueue an order for future dispatch; returns true on success.
    pub fn enqueue(&self, order: &OrderData, ready_tick: u64) -> bool {
        for slot in &self.slots {
            if slot.ready_tick.load(Ordering::Acquire) == CommandDelaySlot::EMPTY {
                slot.write(order, ready_tick);
                return true;
            }
        }
        false
    }

    /// Drain all ready orders at or before `now_tick` into `out`.
    pub fn drain_ready(&self, now_tick: u64, out: &mut Vec<OrderData>) {
        for slot in &self.slots {
            if let Some(order) = slot.take_if_ready(now_tick) {
                out.push(order);
            }
        }
    }

    /// Snapshot all pending delayed orders (no mutation).
    pub fn pending_snapshots(&self) -> Vec<CommandDelaySnapshot> {
        let mut out = Vec::new();
        for slot in &self.slots {
            let ready = slot.ready_tick.load(Ordering::Acquire);
            if ready == CommandDelaySlot::EMPTY {
                continue;
            }
            let header = slot.header.load(Ordering::Relaxed);
            let kind = unpack_kind(header);
            let formation_id = unpack_formation_id(header);
            let generation = ((header >> 32) & 0xFFFFFF) as u32;
            let payload_a = slot.payload_a.load(Ordering::Relaxed);
            let payload_b = slot.payload_b.load(Ordering::Relaxed);
            out.push(CommandDelaySnapshot {
                ready_tick: ready,
                order: OrderData {
                    kind,
                    formation_id,
                    generation,
                    payload_a,
                    payload_b,
                },
            });
        }
        out
    }

    /// Clear all slots.
    pub fn clear(&self) {
        for slot in &self.slots {
            slot.ready_tick
                .store(CommandDelaySlot::EMPTY, Ordering::Release);
        }
    }

    /// Restore slots from snapshots; returns number successfully enqueued.
    pub fn restore_from(&self, snaps: &[CommandDelaySnapshot]) -> usize {
        self.clear();
        let mut restored = 0;
        for snap in snaps {
            if self.enqueue(&snap.order, snap.ready_tick) {
                restored += 1;
            }
        }
        restored
    }
}

verify_alignment_only!(CommandDelayBufferCapsule, 128);

/// SPSC order queue capsule containing preallocated order slots.
#[repr(C, align(128))]
pub struct OrderQueueCapsule {
    head: AtomicU64, // next pop index
    tail: AtomicU64, // next push index
    dropped: AtomicU64,
    slots: [OrderCapsule; ORDER_QUEUE_CAPACITY],
}

impl OrderQueueCapsule {
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            slots: from_fn(|_| OrderCapsule::new()),
        }
    }

    /// Enqueue an order payload into the ring. Returns Ok(slot) on success.
    pub fn push_order(
        &self,
        kind: OrderKind,
        formation_id: u32,
        payload_a: u64,
        payload_b: u64,
    ) -> Result<usize, OrderState> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        if tail.wrapping_sub(head) as usize >= ORDER_QUEUE_CAPACITY {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return Err(OrderState::Full);
        }

        let slot = (tail as usize) & (ORDER_QUEUE_CAPACITY - 1);
        self.slots[slot].write(kind, formation_id, payload_a, payload_b);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(slot)
    }

    /// Pop the next order if available.
    pub fn pop_order(&self) -> Option<OrderData> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }

        let slot = (head as usize) & (ORDER_QUEUE_CAPACITY - 1);
        let data = self.slots[slot].read()?;
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(data)
    }

    pub fn stats(&self) -> QueueStats {
        QueueStats {
            head: self.head.load(Ordering::Relaxed),
            tail: self.tail.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            capacity: ORDER_QUEUE_CAPACITY as u64,
        }
    }
}

verify_alignment_only!(OrderQueueCapsule, 128);

pub const ORDER_QUEUE_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderState {
    Full,
}

#[derive(Debug, Clone, Copy)]
pub struct QueueStats {
    pub head: u64,
    pub tail: u64,
    pub dropped: u64,
    pub capacity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKind {
    Move = 0,
    ChangePosture = 1,
    Fire = 2,
    Hold = 3,
    ArtilleryFire = 4,
    FallBack = 5,
    Withdraw = 6,
    FireControl = 7,
    Charge = 8,
    Brace = 9,
    SetFireDoctrine = 10,
    GarrisonEnter = 11,
    GarrisonExit = 12,
    Grenade = 13,
}

impl OrderKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Move),
            1 => Some(Self::ChangePosture),
            2 => Some(Self::Fire),
            3 => Some(Self::Hold),
            4 => Some(Self::ArtilleryFire),
            5 => Some(Self::FallBack),
            6 => Some(Self::Withdraw),
            7 => Some(Self::FireControl),
            8 => Some(Self::Charge),
            9 => Some(Self::Brace),
            10 => Some(Self::SetFireDoctrine),
            11 => Some(Self::GarrisonEnter),
            12 => Some(Self::GarrisonExit),
            13 => Some(Self::Grenade),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderData {
    pub kind: OrderKind,
    pub formation_id: u32,
    pub generation: u32,
    pub payload_a: u64,
    pub payload_b: u64,
}

const INFLIGHT_MASK: u64 = 1 << 63;

/// Compact AI-issued order payload (packed target + kind + score).
#[derive(Debug, Clone, Copy)]
pub struct AiOrderPayload {
    pub target_formation_id: u32,
    pub order: OrderKind,
    pub score_q8: u8,
}

pub fn pack_ai_order_payload(target_formation_id: u32, order: OrderKind, score_q8: u8) -> u64 {
    ((target_formation_id.min(0xFFFF) as u64) << 16) | ((order as u64) << 8) | (score_q8 as u64)
}

pub fn unpack_ai_order_payload(payload: u64) -> AiOrderPayload {
    AiOrderPayload {
        target_formation_id: ((payload >> 16) & 0xFFFF) as u32,
        order: OrderKind::from_u8(((payload >> 8) & 0xFF) as u8).unwrap_or(OrderKind::Hold),
        score_q8: (payload & 0xFF) as u8,
    }
}

pub fn pack_move_payload(x_q16: u32, z_q16: u32) -> u64 {
    ((x_q16 as u64) & 0xFFFF_FFFF) | ((z_q16 as u64) << 32)
}

pub fn unpack_move_payload(payload_a: u64) -> (u32, u32) {
    (payload_a as u32, (payload_a >> 32) as u32)
}

pub fn pack_posture_payload(posture: u8, stance: u8) -> u64 {
    posture as u64 | ((stance as u64) << 8)
}

pub fn unpack_posture_payload(payload_a: u64) -> (u8, u8) {
    (payload_a as u8, ((payload_a >> 8) & 0xFF) as u8)
}

pub fn pack_charge_meta(charge_posture: u8, commit: bool) -> u64 {
    (charge_posture as u64) | ((commit as u64) << 8)
}

pub fn unpack_charge_meta(payload_b: u64) -> (u8, bool) {
    (payload_b as u8, ((payload_b >> 8) & 1) != 0)
}

pub fn pack_brace_payload(braced: bool) -> u64 {
    braced as u64
}

pub fn unpack_brace_payload(payload_a: u64) -> bool {
    (payload_a & 1) != 0
}

pub fn pack_garrison_payload(structure_id: u32, slot: u16) -> u64 {
    (structure_id as u64) | ((slot as u64) << 32)
}

pub fn unpack_garrison_payload(payload_a: u64) -> (u32, u16) {
    (payload_a as u32, (payload_a >> 32) as u16)
}

/// Fire doctrine payload: mode (u8) | cadence_ticks (u16) packed in payload_a.
pub fn pack_fire_doctrine_payload(mode: FireDoctrineMode, cadence_ticks: u16) -> u64 {
    (mode as u64 & 0xFF) | ((cadence_ticks as u64) << 8)
}

pub fn unpack_fire_doctrine_payload(payload_a: u64) -> (FireDoctrineMode, u16) {
    let mode = FireDoctrineMode::from_u8((payload_a & 0xFF) as u8);
    let cadence = ((payload_a >> 8) & 0xFFFF) as u16;
    (mode, cadence)
}

/// Grenade payload: target coords (payload_a) + fuse/fragments (payload_b).
pub fn pack_grenade_payload(target_x_q16: u32, target_z_q16: u32) -> u64 {
    pack_move_payload(target_x_q16, target_z_q16)
}

pub fn unpack_grenade_payload(payload_a: u64) -> (u32, u32) {
    unpack_move_payload(payload_a)
}

pub fn pack_grenade_meta(fuse_ms: u16, fragments: u16) -> u64 {
    (fuse_ms as u64) | ((fragments as u64) << 16)
}

pub fn unpack_grenade_meta(payload_b: u64) -> (u16, u16) {
    (payload_b as u16, (payload_b >> 16) as u16)
}

pub fn pack_fire_payload(target_x_q16: u32, target_z_q16: u32) -> u64 {
    pack_move_payload(target_x_q16, target_z_q16)
}

pub fn unpack_fire_payload(payload_a: u64) -> (u32, u32) {
    unpack_move_payload(payload_a)
}

pub fn pack_fire_meta(volley: u16, fuse_ms: u16) -> u64 {
    volley as u64 | ((fuse_ms as u64) << 16)
}

pub fn unpack_fire_meta(payload_b: u64) -> (u16, u16) {
    (payload_b as u16, (payload_b >> 16) as u16)
}

/// Extended fire meta: volley | fuse_ms | dispersion_mils | airburst flag.
pub fn pack_fire_meta_extended(
    volley: u16,
    fuse_ms: u16,
    dispersion_mils: u16,
    airburst: bool,
) -> u64 {
    (volley as u64)
        | ((fuse_ms as u64) << 16)
        | ((dispersion_mils as u64) << 32)
        | ((airburst as u64) << 48)
}

pub fn unpack_fire_meta_extended(payload_b: u64) -> (u16, u16, u16, bool) {
    let volley = payload_b as u16;
    let fuse_ms = ((payload_b >> 16) & 0xFFFF) as u16;
    let dispersion_mils = ((payload_b >> 32) & 0xFFFF) as u16;
    let airburst = ((payload_b >> 48) & 1) == 1;
    (volley, fuse_ms, dispersion_mils, airburst)
}

/// Fire-control payload: target coords + battery id (payload_b low 16 bits).
pub fn pack_fire_control_payload(
    target_x_q16: u32,
    target_z_q16: u32,
    battery_id: u16,
) -> (u64, u64) {
    let payload_a = pack_move_payload(target_x_q16, target_z_q16);
    let payload_b = battery_id as u64;
    (payload_a, payload_b)
}

pub fn unpack_fire_control_payload(payload_a: u64, payload_b: u64) -> (u32, u32, u16) {
    let (x, z) = unpack_move_payload(payload_a);
    (x, z, payload_b as u16)
}

pub fn pack_retreat_payload(target_x_q16: u32, target_z_q16: u32) -> u64 {
    pack_move_payload(target_x_q16, target_z_q16)
}

pub fn unpack_retreat_payload(payload_a: u64) -> (u32, u32) {
    unpack_move_payload(payload_a)
}

/// Meta: bit0 backstep (keep facing), bits8-23 command_delay_ms, bits24-39 suppression/fatigue
pub fn pack_retreat_meta(backstep: bool, command_delay_ms: u16, suppression: u16) -> u64 {
    (backstep as u64) | ((command_delay_ms as u64) << 8) | ((suppression as u64) << 24)
}

pub fn unpack_retreat_meta(payload_b: u64) -> (bool, u16, u16) {
    let backstep = (payload_b & 1) == 1;
    let command_delay_ms = ((payload_b >> 8) & 0xFFFF) as u16;
    let suppression = ((payload_b >> 24) & 0xFFFF) as u16;
    (backstep, command_delay_ms, suppression)
}

/// Drain queue and apply to in-memory formations (index = formation_id).
pub fn process_orders_for_formations(
    queue: &OrderQueueCapsule,
    formations: &[crate::formation::FormationCapsule],
    telemetry: &crate::telemetry::TelemetryCapsule,
) {
    while let Some(order) = queue.pop_order() {
        if let Some(formation) = formations.get(order.formation_id as usize) {
            formation.apply_order(&order, telemetry);
        } else {
            telemetry.log_event(); // dropped/unknown formation
        }
    }
}

fn pack_header(kind: OrderKind, formation_id: u32, generation: u32, inflight: bool) -> u64 {
    (kind as u64 & 0xFF)
        | ((formation_id as u64 & 0xFFFFFF) << 8)
        | ((generation as u64 & 0xFFFFFF) << 32)
        | if inflight { INFLIGHT_MASK } else { 0 }
}

fn unpack_kind(header: u64) -> OrderKind {
    let k = (header & 0xFF) as u8;
    OrderKind::from_u8(k).unwrap_or(OrderKind::Hold)
}

fn unpack_formation_id(header: u64) -> u32 {
    ((header >> 8) & 0xFFFFFF) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_push_pop_orders() {
        let q = OrderQueueCapsule::new();
        let payload = pack_move_payload(100, 200);
        q.push_order(OrderKind::Move, 42, payload, 0).unwrap();
        let order = q.pop_order().unwrap();
        assert_eq!(order.kind, OrderKind::Move);
        assert_eq!(order.formation_id, 42);
        assert_eq!(unpack_move_payload(order.payload_a), (100, 200));
    }

    #[test]
    fn queue_full_drops() {
        let q = OrderQueueCapsule::new();
        for i in 0..ORDER_QUEUE_CAPACITY {
            q.push_order(OrderKind::Hold, i as u32, 0, 0).unwrap();
        }
        assert_eq!(
            q.push_order(OrderKind::Hold, 999, 0, 0),
            Err(OrderState::Full)
        );
        let stats = q.stats();
        assert_eq!(stats.dropped, 1);
    }

    #[test]
    fn retreat_meta_round_trips() {
        let payload = pack_retreat_payload(123, 456);
        let (x, z) = unpack_retreat_payload(payload);
        assert_eq!((x, z), (123, 456));

        let meta = pack_retreat_meta(true, 250, 12);
        let (backstep, delay, suppression) = unpack_retreat_meta(meta);
        assert!(backstep);
        assert_eq!(delay, 250);
        assert_eq!(suppression, 12);
    }

    #[test]
    fn fire_meta_extended_round_trips() {
        let meta = pack_fire_meta_extended(6, 1200, 45, true);
        let (volley, fuse, dispersion, airburst) = unpack_fire_meta_extended(meta);
        assert_eq!(volley, 6);
        assert_eq!(fuse, 1200);
        assert_eq!(dispersion, 45);
        assert!(airburst);
    }

    #[test]
    fn fire_doctrine_payload_round_trips() {
        let payload = pack_fire_doctrine_payload(FireDoctrineMode::ByRank, 3);
        let (mode, cadence) = unpack_fire_doctrine_payload(payload);
        assert_eq!(mode, FireDoctrineMode::ByRank);
        assert_eq!(cadence, 3);
    }
}
