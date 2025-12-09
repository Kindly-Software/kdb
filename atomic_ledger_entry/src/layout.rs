use core::fmt;

pub const TS_MAX: u32 = 86_399;
pub const EVENT_MAX: u8 = 0x7F;
pub const ACTOR_MAX: u8 = 0x0F;
pub const SYMBOL_MAX: u16 = 0x0FFF;
pub const PAYLOAD_MIN: i16 = -8_191;
pub const PAYLOAD_MAX: i16 = 8_191;

const ROUTE_SHIFT: u32 = 0;
const ROUTE_MASK: u64 = 0b11;
const PAYLOAD_SHIFT: u32 = 2;
const PAYLOAD_MASK: u64 = 0x3FFF; // 14 bits
const SEQ_SHIFT: u32 = 16;
const SEQ_MASK: u64 = 0xFF;
const SYM_SHIFT: u32 = 24;
const SYM_MASK: u64 = 0x0FFF;
const ACTOR_SHIFT: u32 = 36;
const ACTOR_MASK: u64 = 0x0F;
const EVENT_SHIFT: u32 = 40;
const EVENT_MASK: u64 = 0x7F;
const TS_SHIFT: u32 = 47;
const TS_MASK: u64 = 0x1FFFF; // 17 bits

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route2 {
    None = 0,
    Maker = 0b01,
    Taker = 0b10,
    ReduceOnly = 0b11,
}

impl Route2 {
    pub const fn bits(self) -> u8 {
        self as u8
    }

    pub fn from_bits(value: u8) -> Result<Self, MetaError> {
        match value & 0b11 {
            0 => Ok(Route2::None),
            1 => Ok(Route2::Maker),
            2 => Ok(Route2::Taker),
            3 => Ok(Route2::ReduceOnly),
            _ => Err(MetaError::RouteOutOfRange(value)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AleMeta {
    pub ts_sec_of_day: u32,
    pub event: u8,
    pub actor: u8,
    pub sym_id: u16,
    pub seq: u8,
    pub payload: i16,
    pub route: Route2,
}

impl AleMeta {
    pub const fn new(
        ts_sec_of_day: u32,
        event: u8,
        actor: u8,
        sym_id: u16,
        seq: u8,
        payload: i16,
        route: Route2,
    ) -> Self {
        Self {
            ts_sec_of_day,
            event,
            actor,
            sym_id,
            seq,
            payload,
            route,
        }
    }

    pub fn pack(self) -> Result<u64, MetaError> {
        if self.ts_sec_of_day > TS_MAX {
            return Err(MetaError::TimestampOutOfRange(self.ts_sec_of_day));
        }
        if self.event > EVENT_MAX {
            return Err(MetaError::EventOutOfRange(self.event));
        }
        if self.actor > ACTOR_MAX {
            return Err(MetaError::ActorOutOfRange(self.actor));
        }
        if self.sym_id > SYMBOL_MAX {
            return Err(MetaError::SymbolOutOfRange(self.sym_id));
        }
        let seq_masked = (self.seq as u64) & SEQ_MASK;
        if self.payload < PAYLOAD_MIN || self.payload > PAYLOAD_MAX {
            return Err(MetaError::PayloadOutOfRange(self.payload));
        }
        let payload_bits = encode_payload(self.payload);
        let mut bits = 0u64;
        bits |= (self.route.bits() as u64 & ROUTE_MASK) << ROUTE_SHIFT;
        bits |= (payload_bits as u64 & PAYLOAD_MASK) << PAYLOAD_SHIFT;
        bits |= (seq_masked & SEQ_MASK) << SEQ_SHIFT;
        bits |= ((self.sym_id as u64) & SYM_MASK) << SYM_SHIFT;
        bits |= ((self.actor as u64) & ACTOR_MASK) << ACTOR_SHIFT;
        bits |= ((self.event as u64) & EVENT_MASK) << EVENT_SHIFT;
        bits |= ((self.ts_sec_of_day as u64) & TS_MASK) << TS_SHIFT;
        Ok(bits)
    }
}

impl fmt::Debug for AleMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AleMeta")
            .field("ts_sec_of_day", &self.ts_sec_of_day)
            .field("event", &self.event)
            .field("actor", &self.actor)
            .field("sym_id", &self.sym_id)
            .field("seq", &self.seq)
            .field("payload", &self.payload)
            .field("route", &self.route)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaError {
    TimestampOutOfRange(u32),
    EventOutOfRange(u8),
    ActorOutOfRange(u8),
    SymbolOutOfRange(u16),
    PayloadOutOfRange(i16),
    RouteOutOfRange(u8),
}

pub fn clamp_payload(value: i32) -> i16 {
    if value < PAYLOAD_MIN as i32 {
        PAYLOAD_MIN
    } else if value > PAYLOAD_MAX as i32 {
        PAYLOAD_MAX
    } else {
        value as i16
    }
}

pub fn unpack(bits: u64) -> AleMeta {
    let route_bits = ((bits >> ROUTE_SHIFT) & ROUTE_MASK) as u8;
    let payload_bits = ((bits >> PAYLOAD_SHIFT) & PAYLOAD_MASK) as u16;
    let seq = ((bits >> SEQ_SHIFT) & SEQ_MASK) as u8;
    let sym_id = ((bits >> SYM_SHIFT) & SYM_MASK) as u16;
    let actor = ((bits >> ACTOR_SHIFT) & ACTOR_MASK) as u8;
    let event = ((bits >> EVENT_SHIFT) & EVENT_MASK) as u8;
    let ts = ((bits >> TS_SHIFT) & TS_MASK) as u32;
    let payload = decode_payload(payload_bits);
    let route = Route2::from_bits(route_bits).unwrap_or(Route2::None);
    AleMeta::new(ts, event, actor, sym_id, seq, payload, route)
}

const fn encode_payload(value: i16) -> u16 {
    (value as i32 & PAYLOAD_MASK as i32) as u16
}

const fn decode_payload(bits: u16) -> i16 {
    let raw = (bits & PAYLOAD_MASK as u16) as i16;
    (raw << (16 - 14)) >> (16 - 14)
}

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            MetaError::TimestampOutOfRange(v) => write!(f, "timestamp {v} exceeds 24h span"),
            MetaError::EventOutOfRange(v) => write!(f, "event code {v} exceeds 7-bit range"),
            MetaError::ActorOutOfRange(v) => write!(f, "actor id {v} exceeds 4-bit range"),
            MetaError::SymbolOutOfRange(v) => write!(f, "symbol id {v} exceeds 12-bit range"),
            MetaError::PayloadOutOfRange(v) => write!(f, "payload {v} exceeds ±8191"),
            MetaError::RouteOutOfRange(v) => write!(f, "route bits {v:#04b} invalid"),
        }
    }
}
