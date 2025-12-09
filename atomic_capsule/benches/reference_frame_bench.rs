//! # ReferenceFrameCapsule B32 Benchmarks
//!
//! Fair baseline comparisons with 95% CI, 1000+ iterations

use atomic_capsule::encoder::{ReferenceFrameCapsule, ReferenceType};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::thread;

// ========== Baseline: Naive Mutex-Based Implementation ==========

use std::sync::Mutex;

struct NaiveReferenceFrameManager {
    slots: Mutex<[Option<(usize, u16, u8)>; 8]>, // (dummy_addr, frame_id, order_hint)
    refresh_flags: Mutex<u8>,
    occupancy: Mutex<u8>,
}

impl NaiveReferenceFrameManager {
    fn new() -> Self {
        Self {
            slots: Mutex::new([None; 8]),
            refresh_flags: Mutex::new(0),
            occupancy: Mutex::new(0),
        }
    }

    fn allocate_slot(&self, frame_id: u16) -> Option<u8> {
        let mut slots = self.slots.lock().unwrap();
        let mut occupancy = self.occupancy.lock().unwrap();

        for (i, slot) in slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some((0, frame_id, 0));
                *occupancy += 1;
                return Some(i as u8);
            }
        }

        // Evict oldest
        slots[0] = Some((0, frame_id, 0));
        Some(0)
    }

    fn get_reference(&self, ref_type: ReferenceType) -> Option<usize> {
        let slots = self.slots.lock().unwrap();
        slots[ref_type.to_slot() as usize].map(|(addr, _, _)| addr)
    }

    fn update_slot(&self, slot: u8, frame_addr: usize, frame_id: u16) {
        let mut slots = self.slots.lock().unwrap();
        if let Some(s) = slots.get_mut(slot as usize) {
            *s = Some((frame_addr, frame_id, 0));
        }
    }

    fn mark_for_refresh(&self, refresh_mask: u8) {
        let mut flags = self.refresh_flags.lock().unwrap();
        *flags = refresh_mask;
    }

    fn apply_refresh(&self, new_frame: usize, frame_id: u16, order_hint: u8) {
        let flags = *self.refresh_flags.lock().unwrap();
        let mut slots = self.slots.lock().unwrap();

        for i in 0..8 {
            if (flags & (1 << i)) != 0 {
                slots[i] = Some((new_frame, frame_id, order_hint));
            }
        }

        drop(slots);
        *self.refresh_flags.lock().unwrap() = 0;
    }

    fn get_dpb_occupancy(&self) -> u8 {
        *self.occupancy.lock().unwrap()
    }
}

// ========== Benchmarks ==========

fn bench_allocate_slot(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate_slot");

    group.bench_function("capsule", |b| {
        let capsule = ReferenceFrameCapsule::new();
        let mut frame_id = 0u16;
        b.iter(|| {
            frame_id = frame_id.wrapping_add(1);
            black_box(capsule.allocate_slot(frame_id))
        });
    });

    group.bench_function("naive_mutex", |b| {
        let manager = NaiveReferenceFrameManager::new();
        let mut frame_id = 0u16;
        b.iter(|| {
            frame_id = frame_id.wrapping_add(1);
            black_box(manager.allocate_slot(frame_id))
        });
    });

    group.finish();
}

fn bench_get_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_reference");

    // Setup
    let capsule = ReferenceFrameCapsule::new();
    let frame_ptr = 0x1000_0000 as *const u8;
    let frame_addr = 0x1000_0000usize;
    capsule.allocate_slot(100);
    capsule.update_slot(ReferenceType::Last.to_slot(), frame_ptr, 100);

    let manager = NaiveReferenceFrameManager::new();
    manager.allocate_slot(100);
    manager.update_slot(ReferenceType::Last.to_slot(), frame_addr, 100);

    group.bench_function("capsule", |b| {
        b.iter(|| black_box(capsule.get_reference(ReferenceType::Last)))
    });

    group.bench_function("naive_mutex", |b| {
        b.iter(|| black_box(manager.get_reference(ReferenceType::Last)))
    });

    group.finish();
}

fn bench_update_slot(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_slot");

    let capsule = ReferenceFrameCapsule::new();
    capsule.allocate_slot(100);
    let frame_ptr = 0x1000_0000 as *const u8;
    let frame_addr = 0x1000_0000usize;

    let manager = NaiveReferenceFrameManager::new();
    manager.allocate_slot(100);

    group.bench_function("capsule", |b| {
        let mut frame_id = 100u16;
        b.iter(|| {
            frame_id = frame_id.wrapping_add(1);
            capsule.update_slot(0, frame_ptr, frame_id)
        });
    });

    group.bench_function("naive_mutex", |b| {
        let mut frame_id = 100u16;
        b.iter(|| {
            frame_id = frame_id.wrapping_add(1);
            manager.update_slot(0, frame_addr, frame_id)
        });
    });

    group.finish();
}

fn bench_apply_refresh(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_refresh");

    let capsule = ReferenceFrameCapsule::new();
    for i in 0..8 {
        capsule.allocate_slot(100 + i as u16);
    }
    let frame_ptr = 0x1000_0000 as *const u8;
    let frame_addr = 0x1000_0000usize;

    let manager = NaiveReferenceFrameManager::new();
    for i in 0..8 {
        manager.allocate_slot(100 + i as u16);
    }

    // Benchmark different refresh patterns
    for num_slots in [1, 2, 4, 8] {
        let mask = (1u8 << num_slots) - 1;

        group.bench_with_input(BenchmarkId::new("capsule", num_slots), &num_slots, |b, _| {
            b.iter(|| {
                capsule.mark_for_refresh(mask);
                capsule.apply_refresh(frame_ptr, 200, 50);
            })
        });

        group.bench_with_input(BenchmarkId::new("naive_mutex", num_slots), &num_slots, |b, _| {
            b.iter(|| {
                manager.mark_for_refresh(mask);
                manager.apply_refresh(frame_addr, 200, 50);
            })
        });
    }

    group.finish();
}

fn bench_get_dpb_occupancy(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_dpb_occupancy");

    let capsule = ReferenceFrameCapsule::new();
    for i in 0..5 {
        capsule.allocate_slot(100 + i as u16);
    }

    let manager = NaiveReferenceFrameManager::new();
    for i in 0..5 {
        manager.allocate_slot(100 + i as u16);
    }

    group.bench_function("capsule", |b| {
        b.iter(|| black_box(capsule.get_dpb_occupancy()))
    });

    group.bench_function("naive_mutex", |b| {
        b.iter(|| black_box(manager.get_dpb_occupancy()))
    });

    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    // Capsule: 4 threads, 1000 ops each
    group.bench_function("capsule_4_threads", |b| {
        b.iter(|| {
            let capsule = Arc::new(ReferenceFrameCapsule::new());
            for i in 0..8 {
                capsule.allocate_slot(100 + i as u16);
            }

            let mut handles = vec![];
            for tid in 0..4 {
                let c = Arc::clone(&capsule);
                let h = thread::spawn(move || {
                    let frame_ptr = (0x1000_0000usize + tid * 0x1000) as *const u8;
                    for i in 0..1000 {
                        c.update_slot((tid % 8) as u8, frame_ptr, (tid * 1000 + i) as u16);
                        let _ = c.get_reference(ReferenceType::Last);
                    }
                });
                handles.push(h);
            }

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Naive: 4 threads, 1000 ops each
    group.bench_function("naive_mutex_4_threads", |b| {
        b.iter(|| {
            let manager = Arc::new(NaiveReferenceFrameManager::new());
            for i in 0..8 {
                manager.allocate_slot(100 + i as u16);
            }

            let mut handles = vec![];
            for tid in 0..4 {
                let m = Arc::clone(&manager);
                let h = thread::spawn(move || {
                    let frame_addr = 0x1000_0000usize + tid * 0x1000;
                    for i in 0..1000 {
                        m.update_slot((tid % 8) as u8, frame_addr, (tid * 1000 + i) as u16);
                        let _ = m.get_reference(ReferenceType::Last);
                    }
                });
                handles.push(h);
            }

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_typical_encode_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("typical_encode_flow");

    // Simulate encoding 60 frames (1 second @ 60fps)
    group.bench_function("capsule_60_frames", |b| {
        b.iter(|| {
            let capsule = ReferenceFrameCapsule::new();
            let gop_size = 16;

            for frame_id in 0..60u16 {
                let frame_ptr = (0x1000_0000usize + (frame_id % 8) as usize * 0x1000_0000) as *const u8;

                if frame_id % gop_size == 0 {
                    // I-frame
                    let slot = capsule.allocate_slot(frame_id).unwrap_or(0);
                    capsule.update_slot(slot, frame_ptr, frame_id);
                    capsule.update_slot(ReferenceType::Last.to_slot(), frame_ptr, frame_id);
                    capsule.update_slot(ReferenceType::Golden.to_slot(), frame_ptr, frame_id);
                } else {
                    // P-frame
                    let _ = capsule.get_reference(ReferenceType::Last);
                    let _ = capsule.get_reference(ReferenceType::Last2);
                    let _ = capsule.get_reference(ReferenceType::Golden);

                    let slot = capsule.allocate_slot(frame_id).unwrap_or((frame_id % 8) as u8);
                    capsule.update_slot(ReferenceType::Last.to_slot(), frame_ptr, frame_id);
                }

                if frame_id % 8 == 0 && frame_id > 0 {
                    capsule.update_slot(ReferenceType::Golden.to_slot(), frame_ptr, frame_id);
                }
            }
        });
    });

    group.bench_function("naive_mutex_60_frames", |b| {
        b.iter(|| {
            let manager = NaiveReferenceFrameManager::new();
            let gop_size = 16;

            for frame_id in 0..60u16 {
                let frame_addr = 0x1000_0000usize + (frame_id % 8) as usize * 0x1000_0000;

                if frame_id % gop_size == 0 {
                    let slot = manager.allocate_slot(frame_id).unwrap_or(0);
                    manager.update_slot(slot, frame_addr, frame_id);
                    manager.update_slot(ReferenceType::Last.to_slot(), frame_addr, frame_id);
                    manager.update_slot(ReferenceType::Golden.to_slot(), frame_addr, frame_id);
                } else {
                    let _ = manager.get_reference(ReferenceType::Last);
                    let _ = manager.get_reference(ReferenceType::Last2);
                    let _ = manager.get_reference(ReferenceType::Golden);

                    let slot = manager.allocate_slot(frame_id).unwrap_or((frame_id % 8) as u8);
                    manager.update_slot(ReferenceType::Last.to_slot(), frame_addr, frame_id);
                }

                if frame_id % 8 == 0 && frame_id > 0 {
                    manager.update_slot(ReferenceType::Golden.to_slot(), frame_addr, frame_id);
                }
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_allocate_slot,
    bench_get_reference,
    bench_update_slot,
    bench_apply_refresh,
    bench_get_dpb_occupancy,
    bench_concurrent_access,
    bench_typical_encode_flow,
);

criterion_main!(benches);
