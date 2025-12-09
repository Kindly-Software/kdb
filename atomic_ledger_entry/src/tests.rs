use super::*;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::vec::Vec;

use crate::layout::unpack;

#[test]
fn meta_pack_roundtrip() {
    let meta = AleMeta::new(
        42_000,
        EventCodes::ORDER_SENT,
        3,
        512,
        0xAB,
        -1_024,
        Route2::Maker,
    );
    let bits = meta.pack().unwrap();
    let decoded = unpack(bits);
    assert_eq!(decoded.ts_sec_of_day, 42_000);
    assert_eq!(decoded.event, EventCodes::ORDER_SENT);
    assert_eq!(decoded.actor, 3);
    assert_eq!(decoded.sym_id, 512);
    assert_eq!(decoded.seq, 0xAB);
    assert_eq!(decoded.payload, -1_024);
    assert_eq!(decoded.route, Route2::Maker);
}

#[test]
fn payload_clamping() {
    assert_eq!(clamp_payload(10_000), PAYLOAD_MAX);
    assert_eq!(clamp_payload(-10_000), PAYLOAD_MIN);
    assert_eq!(clamp_payload(1234), 1234);
}

#[test]
fn hash_vector_matches() {
    let key = AleKey::new(*b"0123456789abcdef0123456789abcdef");
    let prev_entry = 0x1111_2222_3333_4444_5555_6666_7777_8888u128;
    let meta = AleMeta::new(
        12_345,
        EventCodes::FILL_DONE,
        5,
        1023,
        0x5A,
        -80,
        Route2::Taker,
    );
    let bits = meta.pack().unwrap();
    let hash = chain_prev_hash(&key, prev_entry, bits);
    assert_eq!(hash, 0xf7bd_4bcd_351d_80d0);
}

#[test]
fn writer_appends_and_updates_ring() {
    let ring = AleRing::with_capacity_pow2(8);
    let key = AleKey::new([0x11; 32]);
    let genesis = derive_genesis_hash(&key, b"ALE|day|stream|boot");
    let mut writer = Writer::new(
        &ring,
        &key,
        WriterConfig {
            head: 0,
            initial_seq: 0,
            genesis_prev_hash: genesis,
            last_entry: None,
        },
    );
    let event = AleEvent {
        ts_ns: 123_456_789,
        event: EventCodes::ORDER_SENT,
        actor: 2,
        sym_id: 77,
        route: Route2::Maker,
        payload: 700,
    };
    let entry = writer.append(event).expect("append succeeds");
    assert_eq!(writer.position(), 1);
    assert_eq!(ring.published(), 1);
    assert_eq!(
        ring.load(0, core::sync::atomic::Ordering::Relaxed),
        entry.raw()
    );
}

#[test]
fn validator_detects_tamper() {
    let ring = AleRing::with_capacity_pow2(16);
    let key = AleKey::new([0x22; 32]);
    let genesis = derive_genesis_hash(&key, b"ALE|day|stream|boot");
    let mut writer = Writer::new(
        &ring,
        &key,
        WriterConfig {
            head: 0,
            initial_seq: 0,
            genesis_prev_hash: genesis,
            last_entry: None,
        },
    );
    for i in 0..8 {
        let event = AleEvent {
            ts_ns: 1_000_000_000 + (i as u64) * 123_456,
            event: EventCodes::ORDER_ACK,
            actor: 2,
            sym_id: i as u16,
            route: Route2::Maker,
            payload: i,
        };
        writer.append(event).unwrap();
    }
    let mut entries = Vec::new();
    for i in 0..writer.position() {
        entries.push(ring.load(i, core::sync::atomic::Ordering::Relaxed));
    }
    assert!(verify_chain(&entries, &key, genesis).is_ok());
    entries[4] ^= 1 << 17;
    match verify_chain(&entries, &key, genesis) {
        Err(VerifyError::Chain(mismatch)) => assert_eq!(mismatch.index, 4),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn validator_flags_sequence_gap() {
    let ring = AleRing::with_capacity_pow2(8);
    let key = AleKey::new([0x33; 32]);
    let genesis = derive_genesis_hash(&key, b"ALE|day|stream|boot");
    let mut writer = Writer::new(
        &ring,
        &key,
        WriterConfig {
            head: 0,
            initial_seq: 0,
            genesis_prev_hash: genesis,
            last_entry: None,
        },
    );
    for payload in [10, 20, 30, 40] {
        let event = AleEvent {
            ts_ns: 2_000_000_000,
            event: EventCodes::FILL_PART,
            actor: 3,
            sym_id: 99,
            route: Route2::Taker,
            payload,
        };
        writer.append(event).unwrap();
    }
    let mut original = Vec::new();
    for i in 0..writer.position() {
        original.push(ring.load(i, core::sync::atomic::Ordering::Relaxed));
    }
    let mut tampered = Vec::with_capacity(original.len());
    for (idx, raw) in original.iter().copied().enumerate() {
        let entry = AleEntry::from(raw);
        let mut meta = entry.meta();
        if idx == 2 {
            meta.seq = meta.seq.wrapping_add(3);
        }
        let bits = meta.pack().unwrap();
        let prev_hash = if idx == 0 {
            genesis
        } else {
            chain_prev_hash(&key, tampered[idx - 1], bits)
        };
        let rebuilt = AleEntry::new(prev_hash, bits);
        tampered.push(rebuilt.raw());
    }
    match verify_chain(&tampered, &key, genesis) {
        Err(VerifyError::Sequence(gap)) => assert_eq!(gap.index, 2),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn random_bit_flip_breaks_chain() {
    let ring = AleRing::with_capacity_pow2(32);
    let key = AleKey::new([0x44; 32]);
    let genesis = derive_genesis_hash(&key, b"ALE|day|stream|boot");
    let mut writer = Writer::new(
        &ring,
        &key,
        WriterConfig {
            head: 0,
            initial_seq: 0,
            genesis_prev_hash: genesis,
            last_entry: None,
        },
    );
    for i in 0..20 {
        let event = AleEvent {
            ts_ns: 3_000_000_000 + (i as u64) * 1_000,
            event: EventCodes::ORDER_SENT,
            actor: (i % 4) as u8,
            sym_id: (100 + i) as u16,
            route: if i % 2 == 0 {
                Route2::Maker
            } else {
                Route2::Taker
            },
            payload: i * 5 - 40,
        };
        writer.append(event).unwrap();
    }
    let mut baseline = Vec::new();
    for i in 0..writer.position() {
        baseline.push(ring.load(i, core::sync::atomic::Ordering::Relaxed));
    }
    assert!(verify_chain(&baseline, &key, genesis).is_ok());

    let mut rng = StdRng::seed_from_u64(0xfeed_beef);
    for _ in 0..10 {
        let mut tampered = baseline.clone();
        let target = rng.gen_range(0..tampered.len());
        let bit = rng.gen_range(0..128);
        tampered[target] ^= 1u128 << bit;
        assert!(verify_chain(&tampered, &key, genesis).is_err());
    }
}

#[cfg(feature = "stream")]
#[test]
fn ledger_stream_flushes_events() {
    let key = AleKey::new([0x55; 32]);
    let builder = LedgerStreamBuilder::new(key.clone())
        .ring_capacity(32)
        .queue_capacity(32)
        .genesis_context(b"ALE|test-stream".to_vec())
        .thread_name("ale-test");
    let stream = builder.spawn().expect("stream spawns");
    let producer = stream.producer();
    for i in 0..16 {
        let event =
            AleEvent::order_sent(5_000_000_000 + (i as u64) * 1_000, 1, 7, Route2::Maker, i);
        producer.enqueue_blocking(event).expect("event enqueued");
    }
    drop(producer);
    let ring = stream.ring().clone();
    let stats = stream.shutdown().expect("writer joins");
    assert_eq!(stats.appended, 16);
    assert_eq!(stats.meta_errors, 0);
    assert_eq!(ring.published(), 16);
    let mut entries = Vec::new();
    for idx in 0..ring.published() {
        entries.push(ring.load(idx, core::sync::atomic::Ordering::Relaxed));
    }
    let genesis = derive_genesis_hash(&key, b"ALE|test-stream");
    assert!(verify_chain(&entries, &key, genesis).is_ok());
}
