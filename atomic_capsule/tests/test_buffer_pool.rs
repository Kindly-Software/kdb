//! Integration tests for BufferPoolCapsule

use atomic_capsule::gui::render::{BufferPoolCapsule, BufferState};

#[test]
fn test_buffer_pool_creation() {
    let pool = BufferPoolCapsule::new(1024);
    assert_eq!(pool.pending_count(), 0);
    assert_eq!(pool.total_frames(), 0);
}

#[test]
fn test_buffer_pool_full_cycle() {
    let pool = BufferPoolCapsule::new(2048);

    // Acquire buffer
    let idx = pool.acquire_write_buffer().unwrap();
    assert_eq!(pool.buffer_state(idx), BufferState::Writing);

    // Submit buffer
    pool.set_used_bytes(idx, 256);
    pool.submit_buffer(idx);
    assert_eq!(pool.buffer_state(idx), BufferState::Pending);
    assert_eq!(pool.pending_count(), 1);

    // Render buffer
    let render_idx = pool.begin_render().unwrap();
    assert_eq!(render_idx, idx);
    assert_eq!(pool.buffer_state(render_idx), BufferState::Rendering);
    assert_eq!(pool.pending_count(), 0);

    // Complete render
    pool.complete_render(render_idx);
    assert_eq!(pool.buffer_state(render_idx), BufferState::Free);
    assert_eq!(pool.total_frames(), 1);
    assert_eq!(pool.used_bytes(render_idx), 0); // Reset
}

#[test]
fn test_buffer_pool_triple_buffering() {
    let pool = BufferPoolCapsule::new(1024);

    // Acquire all 3 buffers
    let idx0 = pool.acquire_write_buffer().unwrap();
    assert_eq!(idx0, 0);

    let idx1 = pool.acquire_write_buffer().unwrap();
    assert_eq!(idx1, 1);

    let idx2 = pool.acquire_write_buffer().unwrap();
    assert_eq!(idx2, 2);

    // No more buffers available
    assert!(pool.acquire_write_buffer().is_none());

    // Submit and render buffer 0
    pool.submit_buffer(idx0);
    pool.begin_render().unwrap();
    pool.complete_render(idx0);

    // Buffer 0 should be free again
    let idx_new = pool.acquire_write_buffer().unwrap();
    assert_eq!(idx_new, 0);
}

#[test]
fn test_buffer_pool_size_alignment() {
    use core::mem::{align_of, size_of};

    assert_eq!(size_of::<BufferPoolCapsule>(), 256);
    assert_eq!(align_of::<BufferPoolCapsule>(), 64);
}

#[test]
fn test_buffer_pool_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let pool = Arc::new(BufferPoolCapsule::new(1024));
    let mut handles = vec![];

    // Spawn 3 threads, each processing one buffer
    for thread_id in 0..3 {
        let pool_clone = Arc::clone(&pool);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                if let Some(idx) = pool_clone.acquire_write_buffer() {
                    pool_clone.set_used_bytes(idx, (thread_id + 1) * 100);
                    pool_clone.submit_buffer(idx);

                    if let Some(render_idx) = pool_clone.begin_render() {
                        // Simulate GPU work
                        thread::sleep(std::time::Duration::from_micros(1));
                        pool_clone.complete_render(render_idx);
                    }
                }
                thread::yield_now();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All buffers should be back to free
    for i in 0..3 {
        assert_eq!(pool.buffer_state(i), BufferState::Free);
    }

    // Should have processed 30 frames total
    assert_eq!(pool.total_frames(), 30);
}
