use core::fmt;

use crate::layout::{clamp_payload, AleMeta, Route2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AleEvent {
    pub ts_ns: u64,
    pub event: u8,
    pub actor: u8,
    pub sym_id: u16,
    pub route: Route2,
    pub payload: i32,
}

impl AleEvent {
    #[inline]
    pub const fn new(
        ts_ns: u64,
        event: u8,
        actor: u8,
        sym_id: u16,
        route: Route2,
        payload: i32,
    ) -> Self {
        Self {
            ts_ns,
            event,
            actor,
            sym_id,
            route,
            payload,
        }
    }

    pub fn into_meta(self, seq: u8) -> AleMeta {
        let ts_sec = ((self.ts_ns / 1_000_000_000) % 86_400) as u32;
        let payload = clamp_payload(self.payload);
        AleMeta::new(
            ts_sec,
            self.event,
            self.actor,
            self.sym_id,
            seq,
            payload,
            self.route,
        )
    }

    pub fn order_sent(ts_ns: u64, actor: u8, sym_id: u16, route: Route2, qty_delta: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ORDER_SENT,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn order_ack(ts_ns: u64, actor: u8, sym_id: u16, route: Route2, qty_delta: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ORDER_ACK,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn fill_partial(ts_ns: u64, actor: u8, sym_id: u16, route: Route2, qty_delta: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::FILL_PART,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn fill_complete(
        ts_ns: u64,
        actor: u8,
        sym_id: u16,
        route: Route2,
        qty_delta: i32,
    ) -> Self {
        Self::new(
            ts_ns,
            EventCodes::FILL_DONE,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn cancel_request(
        ts_ns: u64,
        actor: u8,
        sym_id: u16,
        route: Route2,
        qty_delta: i32,
    ) -> Self {
        Self::new(
            ts_ns,
            EventCodes::CANCEL_REQ,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn cancel_ok(ts_ns: u64, actor: u8, sym_id: u16, route: Route2, qty_delta: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::CANCEL_OK,
            actor,
            sym_id,
            route,
            qty_delta,
        )
    }

    pub fn reject(ts_ns: u64, actor: u8, sym_id: u16, reason_code: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::REJECT,
            actor,
            sym_id,
            Route2::None,
            reason_code,
        )
    }

    pub fn aeb_published(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::AEB_PUBLISHED,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn aeb_stale(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::AEB_STALE,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn breaker_transition(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::BREAKER_LX_LY,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn eco_lockout(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ECO_LOCKOUT,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn eco_resume(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ECO_RESUME,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn apc_update(ts_ns: u64, actor: u8, sym_id: u16, pnl_delta_ticks: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::APC_UPDATE,
            actor,
            sym_id,
            Route2::None,
            pnl_delta_ticks,
        )
    }

    pub fn are_hit(ts_ns: u64, actor: u8, sym_id: u16, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ARE_HIT,
            actor,
            sym_id,
            Route2::None,
            payload,
        )
    }

    pub fn policy_change(ts_ns: u64, actor: u8, sym_id: u16, policy_id: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::POLICY_CHANGE,
            actor,
            sym_id,
            Route2::None,
            policy_id,
        )
    }

    pub fn error_io(ts_ns: u64, actor: u8, sym_id: u16, code: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ERROR_IO,
            actor,
            sym_id,
            Route2::None,
            code,
        )
    }

    pub fn error_latency(ts_ns: u64, actor: u8, sym_id: u16, code: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ERROR_LATENCY,
            actor,
            sym_id,
            Route2::None,
            code,
        )
    }

    pub fn error_route(ts_ns: u64, actor: u8, sym_id: u16, code: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::ERROR_ROUTE,
            actor,
            sym_id,
            Route2::None,
            code,
        )
    }

    pub fn checkpoint(ts_ns: u64, actor: u8, payload: i32) -> Self {
        Self::new(
            ts_ns,
            EventCodes::CHECKPOINT,
            actor,
            0,
            Route2::None,
            payload,
        )
    }

    pub fn boot(ts_ns: u64, actor: u8, payload: i32) -> Self {
        Self::new(ts_ns, EventCodes::BOOT, actor, 0, Route2::None, payload)
    }

    pub fn shutdown(ts_ns: u64, actor: u8, payload: i32) -> Self {
        Self::new(ts_ns, EventCodes::SHUTDOWN, actor, 0, Route2::None, payload)
    }
}

pub struct EventCodes;

impl EventCodes {
    pub const ORDER_SENT: u8 = 0x01;
    pub const ORDER_ACK: u8 = 0x02;
    pub const FILL_PART: u8 = 0x03;
    pub const FILL_DONE: u8 = 0x04;
    pub const CANCEL_REQ: u8 = 0x05;
    pub const CANCEL_OK: u8 = 0x06;
    pub const REJECT: u8 = 0x07;
    pub const AEB_PUBLISHED: u8 = 0x10;
    pub const AEB_STALE: u8 = 0x11;
    pub const BREAKER_L0_L1: u8 = 0x20;
    pub const BREAKER_L1_L2: u8 = 0x21;
    pub const BREAKER_LX_LY: u8 = 0x22;
    pub const ECO_LOCKOUT: u8 = 0x30;
    pub const ECO_RESUME: u8 = 0x31;
    pub const APC_UPDATE: u8 = 0x40;
    pub const ARE_HIT: u8 = 0x41;
    pub const POLICY_CHANGE: u8 = 0x50;
    pub const ERROR_IO: u8 = 0x60;
    pub const ERROR_LATENCY: u8 = 0x61;
    pub const ERROR_ROUTE: u8 = 0x62;
    pub const CHECKPOINT: u8 = 0x70;
    pub const BOOT: u8 = 0x71;
    pub const SHUTDOWN: u8 = 0x72;
}

impl fmt::Debug for EventCodes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventCodes")
    }
}
