//! GPU Sync Capsule Integration Tests (T28 Q15-Q21)
//!
//! Integration testing for GpuSyncCapsule with realistic scenarios.

#![cfg(test)]

use atomic_capsule::gpu::{
    GpuSyncCapsule,
    MemoryBarrier,
    PipelineStage,
    AccessFlags,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// === Q15: Render loop simulation (integration test) ===

#[test]
fn integration_render_loop_simulation() {
    let capsule = GpuSyncCapsule::new(2); // Double buffering
    let num_frames = 100;

    for frame in 0..num_frames {
        // Acquire fence for current frame
        let fence_idx = capsule.allocate_fence().expect("Fence allocation failed");
        capsule.set_fence_handle(fence_idx, frame);

        // Signal timeline
        let timeline_value = capsule.signal_timeline();
        assert_eq!(timeline_value, frame + 1);

        // Record barrier (render target → shader read)
        capsule.record_barrier(&MemoryBarrier::render_to_sample());

        // Advance frame
        let next_frame = capsule.advance_frame();
        assert_eq!(next_frame, (frame + 1) % 2);

        // Free fence
        capsule.free_fence(fence_idx);
    }

    // Verify final state
    assert_eq!(capsule.get_total_syncs(), num_frames);
    assert_eq!(capsule.get_total_signals(), num_frames);
    assert_eq!(capsule.get_total_barriers(), num_frames);
    assert_eq!(capsule.get_fence_utilization(), 0);
}

// === Q16: Multi-threaded producer-consumer (integration test) ===

#[test]
fn integration_producer_consumer_timeline() {
    let capsule = Arc::new(GpuSyncCapsule::new(3));
    let num_items = 100;

    // Producer thread: signals timeline
    let producer = {
        let capsule = Arc::clone(&capsule);
        thread::spawn(move || {
            for i in 1..=num_items {
                capsule.signal_timeline();
                if i % 10 == 0 {
                    thread::sleep(Duration::from_micros(10));
                }
            }
        })
    };

    // Consumer thread: waits for timeline values
    let consumer = {
        let capsule = Arc::clone(&capsule);
        thread::spawn(move || {
            let mut consumed = 0;
            while consumed < num_items {
                if capsule.wait_timeline(consumed + 1) {
                    consumed += 1;
                }
                thread::yield_now();
            }
            consumed
        })
    };

    producer.join().unwrap();
    let consumed = consumer.join().unwrap();

    assert_eq!(consumed, num_items);
    assert_eq!(capsule.get_timeline_value(), num_items);
}

// === Q17: Fence pool stress test (integration test) ===

#[test]
fn integration_fence_pool_stress() {
    let capsule = Arc::new(GpuSyncCapsule::new(2));
    let num_threads = 4;
    let allocations_per_thread = 100;

    let mut handles = vec![];
    for _ in 0..num_threads {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..allocations_per_thread {
                // Try to allocate fence
                if let Some(fence) = capsule.allocate_fence() {
                    // Simulate work
                    thread::sleep(Duration::from_micros(1));
                    // Free fence
                    capsule.free_fence(fence);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All fences should be freed
    assert_eq!(capsule.get_fence_utilization(), 0);
}

// === Q18: Barrier batching scenario (integration test) ===

#[test]
fn integration_barrier_batching() {
    let capsule = GpuSyncCapsule::new(2);

    // Simulate deferred rendering pass with multiple barriers
    let barriers = vec![
        MemoryBarrier::render_to_sample(),       // G-buffer → fragment shader
        MemoryBarrier::compute_to_compute(),     // Compute pass
        MemoryBarrier::transfer_to_shader(),     // Upload → shader
        MemoryBarrier::device_to_host(),         // GPU → CPU readback
    ];

    // Batch all barriers
    for barrier in &barriers {
        capsule.record_barrier(barrier);
    }

    assert_eq!(capsule.get_total_barriers(), barriers.len() as u64);
}

// === Q19: Frame synchronization with overflow (integration test) ===

#[test]
fn integration_frame_sync_overflow() {
    let capsule = GpuSyncCapsule::new(3); // Triple buffering
    let num_frames = 1000;

    for _ in 0..num_frames {
        let frame = capsule.advance_frame();
        assert!(frame < 3, "Frame index overflow");
    }

    // Check final wraparound
    let final_frame = capsule.get_current_frame();
    assert_eq!(final_frame, (num_frames % 3) as u64);
}

// === Q20: Complex rendering pipeline (integration test) ===

#[test]
fn integration_complex_rendering_pipeline() {
    let capsule = Arc::new(GpuSyncCapsule::new(2));

    // Simulate graphics + compute pipeline
    let graphics_thread = {
        let capsule = Arc::clone(&capsule);
        thread::spawn(move || {
            for _ in 0..50 {
                // Allocate fence
                let fence = capsule.allocate_fence().expect("Fence allocation failed");

                // Record graphics barriers
                capsule.record_barrier(&MemoryBarrier::render_to_sample());

                // Signal timeline
                capsule.signal_timeline();

                // Free fence
                capsule.free_fence(fence);

                thread::sleep(Duration::from_micros(10));
            }
        })
    };

    let compute_thread = {
        let capsule = Arc::clone(&capsule);
        thread::spawn(move || {
            for _ in 0..50 {
                // Record compute barriers
                capsule.record_barrier(&MemoryBarrier::compute_to_compute());

                // Signal timeline
                capsule.signal_timeline();

                thread::sleep(Duration::from_micros(10));
            }
        })
    };

    graphics_thread.join().unwrap();
    compute_thread.join().unwrap();

    // Verify total operations
    assert_eq!(capsule.get_total_barriers(), 100); // 50 graphics + 50 compute
    assert_eq!(capsule.get_total_signals(), 100);
}

// === Q21: Binary semaphore usage (integration test) ===

#[test]
fn integration_binary_semaphore_queue_sync() {
    let capsule = GpuSyncCapsule::new(2);

    // Simulate queue synchronization with binary semaphores
    for i in 0..4 {
        let sem_handle = 0x1000 + i;
        capsule.set_binary_semaphore(i as u8, sem_handle);
        assert_eq!(capsule.get_binary_semaphore(i as u8), sem_handle);
    }

    // Verify signals recorded
    assert_eq!(capsule.get_total_signals(), 4);
}
