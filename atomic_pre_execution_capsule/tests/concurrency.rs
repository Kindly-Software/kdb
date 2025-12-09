use atomic_pre_execution_capsule::{pipeline::PexRouter, pipeline::PexWriter, PexCapsule};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::vec::Vec;

const ITERATIONS: usize = 5_000;
const PLAY_COUNT: usize = 4;

#[test]
fn cross_thread_publish_and_read_consistency() {
    let capsule = Arc::new(PexCapsule::new());
    let publish_counter = Arc::new(AtomicUsize::new(0));
    let read_counter = Arc::new(AtomicUsize::new(0));
    let fired_counter = Arc::new(AtomicUsize::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let writer_capsule = Arc::clone(&capsule);
    let publish_counter_writer = Arc::clone(&publish_counter);
    let running_writer = Arc::clone(&running);
    let writer_handle = thread::spawn(move || {
        let capsule_ref: &PexCapsule = &writer_capsule;
        let mut writer = PexWriter::new(capsule_ref);
        {
            let draft = writer.draft_mut();
            draft.header.account_id = 101;
            draft.header.symbol_count = 2;
            draft.header.default_ttl_ms = 900;
            for play in draft.plays.iter_mut() {
                play.enable = true;
                play.ttl_ms = 900;
                play.lat_budget_us = 512;
                play.trig_mask = 0b0001_1111;
                play.qty = 10;
            }
        }

        for i in 0..ITERATIONS {
            let draft = writer.draft_mut();
            draft.header.created_ms_coarse = draft.header.created_ms_coarse.wrapping_add(1);
            draft.defaults.lat_budget_default_us = (i as u16) & 0x0fff;
            for (lane, play) in draft.plays.iter_mut().enumerate() {
                play.qty = ((i + lane) % (1 << 18)) as u32;
                play.priority = ((lane as u8).wrapping_mul(7).wrapping_add(i as u8)) & 0x3f;
                play.sym_id = 10 + lane as u16;
                play.px_ticks = (lane as i16) - 1;
            }
            writer.publish();
            publish_counter_writer.fetch_add(1, Ordering::Release);
        }
        running_writer.store(false, Ordering::Release);
    });

    let reader_capsule = Arc::clone(&capsule);
    let publish_counter_reader = Arc::clone(&publish_counter);
    let read_counter_reader = Arc::clone(&read_counter);
    let fired_counter_reader = Arc::clone(&fired_counter);
    let running_reader = Arc::clone(&running);
    let reader_handle = thread::spawn(move || {
        let capsule_ref: &PexCapsule = &reader_capsule;
        let mut router = PexRouter::new(capsule_ref);
        let mut last_seq: u16 = 0;
        loop {
            if let Some(snapshot) = router.poll_snapshot() {
                let header = snapshot.header();
                assert!(header.seq_head >= last_seq, "sequence regression detected");
                last_seq = header.seq_head;
                read_counter_reader.fetch_add(1, Ordering::Relaxed);

                let mut indices: Vec<usize> = (0..PLAY_COUNT).collect();
                indices.sort_by(|a, b| snapshot.play(*b).priority.cmp(&snapshot.play(*a).priority));
                for idx in indices.into_iter().take(PLAY_COUNT) {
                    let play = snapshot.play(idx);
                    if !play.enable {
                        continue;
                    }
                    fired_counter_reader.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            } else if !running_reader.load(Ordering::Acquire)
                && read_counter_reader.load(Ordering::Acquire)
                    >= publish_counter_reader.load(Ordering::Acquire)
            {
                break;
            } else {
                thread::yield_now();
            }
        }
    });

    writer_handle.join().expect("writer thread");
    reader_handle.join().expect("reader thread");

    assert_eq!(publish_counter.load(Ordering::Relaxed), ITERATIONS);
    assert_eq!(read_counter.load(Ordering::Relaxed), ITERATIONS);
    assert!(fired_counter.load(Ordering::Relaxed) >= ITERATIONS);
}
