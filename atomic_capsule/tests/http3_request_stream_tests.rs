//! HTTP/3 Request Stream Comprehensive Test Suite (T28 Framework)
//!
//! Tests for Http3RequestStreamCapsule implementing RFC 9114 §4.1 message framing.
//! Organized into 4 tiers: Unit (Q1-Q7), Property (Q8-Q14), Integration (Q15-Q21), Production (Q22-Q28)

use atomic_capsule::quic::{
    BodyChunk, ChunkFlags, Http3RequestStreamCapsule, Http3Result, Http3StreamError,
    HttpMethod, RequestStreamState,
};
use std::sync::atomic::Ordering;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn test_new_stream() {
    let stream = Http3RequestStreamCapsule::new(42, 5000);
    assert_eq!(stream.get_stream_id(), 42);
    assert_eq!(stream.get_state(), RequestStreamState::Headers);
    assert_eq!(stream.get_bytes_received(), 0);
    assert_eq!(stream.get_queue_size(), 0);
}

#[test]
fn test_append_single_chunk() {
    let stream = Http3RequestStreamCapsule::new(1, 1024);

    let result = stream.append_body_chunk(0, 1024, true);
    assert!(result.is_ok());

    assert_eq!(stream.get_bytes_received(), 1024);
    assert_eq!(stream.is_complete(), true);
    assert_eq!(stream.get_queue_size(), 1);
}

#[test]
fn test_append_multiple_chunks() {
    let stream = Http3RequestStreamCapsule::new(2, 3000);

    stream.append_body_chunk(0, 1000, false).unwrap();
    stream.append_body_chunk(1000, 1000, false).unwrap();
    stream.append_body_chunk(2000, 1000, true).unwrap();

    assert_eq!(stream.get_bytes_received(), 3000);
    assert_eq!(stream.is_complete(), true);
    assert_eq!(stream.get_queue_size(), 3);
}

#[test]
fn test_consume_chunks() {
    let stream = Http3RequestStreamCapsule::new(3, 2000);

    stream.append_body_chunk(0, 1000, false).unwrap();
    stream.append_body_chunk(1000, 1000, true).unwrap();

    if let Some(chunk) = stream.consume_body_chunk() {
        assert_eq!(chunk.offset.load(Ordering::Acquire), 0);
        assert_eq!(chunk.length.load(Ordering::Acquire), 1000);
        assert!(!chunk.get_flags().is_fin());
    } else {
        panic!("Expected chunk");
    }

    if let Some(chunk) = stream.consume_body_chunk() {
        assert_eq!(chunk.offset.load(Ordering::Acquire), 1000);
        assert_eq!(chunk.length.load(Ordering::Acquire), 1000);
        assert!(chunk.get_flags().is_fin());
    } else {
        panic!("Expected chunk");
    }

    assert!(stream.consume_body_chunk().is_none());
}

#[test]
fn test_method_operations() {
    let stream = Http3RequestStreamCapsule::new(8, 0);

    assert_eq!(stream.get_method(), HttpMethod::Get as u8);

    stream.set_method(HttpMethod::Post);
    assert_eq!(stream.get_method(), HttpMethod::Post as u8);

    stream.set_method(HttpMethod::Delete);
    assert_eq!(stream.get_method(), HttpMethod::Delete as u8);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn test_backpressure() {
    let stream = Http3RequestStreamCapsule::new(4, 1000000);

    // Fill queue with 128 chunks (max capacity)
    for i in 0..128 {
        let result = stream.append_body_chunk(i * 1000, 1000, false);
        assert!(result.is_ok(), "Failed at chunk {}", i);
    }

    // Next append should fail with backpressure
    let result = stream.append_body_chunk(128000, 1000, false);
    assert_eq!(result, Err(Http3StreamError::QueueFull));
}

#[test]
fn test_progress_unknown_length() {
    let stream = Http3RequestStreamCapsule::new(5, 0); // unknown length

    stream.append_body_chunk(0, 500, false).unwrap();

    // Progress should be 0.5 (conservative estimate for chunked)
    let progress = stream.get_progress().unwrap();
    assert_eq!(progress, 0.5);
}

#[test]
fn test_progress_known_length() {
    let stream = Http3RequestStreamCapsule::new(6, 1000);

    stream.append_body_chunk(0, 500, false).unwrap();

    // Progress should be 0.5 (500/1000)
    let progress = stream.get_progress().unwrap();
    assert!(progress > 0.49 && progress < 0.51);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn test_state_transitions() {
    let stream = Http3RequestStreamCapsule::new(7, 500);

    assert_eq!(stream.get_state(), RequestStreamState::Headers);

    // Transition to Body
    stream.append_body_chunk(0, 250, false).unwrap();
    assert_eq!(stream.get_state(), RequestStreamState::Body);

    // Transition to Complete
    stream.append_body_chunk(250, 250, true).unwrap();
    assert_eq!(stream.get_state(), RequestStreamState::Complete);
}

#[test]
fn test_full_request_lifecycle() {
    let stream = Http3RequestStreamCapsule::new(100, 4000);

    // Set HTTP method
    stream.set_method(HttpMethod::Post);
    assert_eq!(stream.get_method(), HttpMethod::Post as u8);

    // Append body chunks
    assert!(stream.append_body_chunk(0, 1000, false).is_ok());
    assert!(stream.append_body_chunk(1000, 1000, false).is_ok());
    assert!(stream.append_body_chunk(2000, 1000, false).is_ok());
    assert!(stream.append_body_chunk(3000, 1000, true).is_ok());

    // Verify state
    assert_eq!(stream.get_bytes_received(), 4000);
    assert_eq!(stream.is_complete(), true);
    assert_eq!(stream.get_state(), RequestStreamState::Complete);

    // Consume all chunks
    let mut count = 0;
    while let Some(chunk) = stream.consume_body_chunk() {
        assert!(chunk.offset.load(Ordering::Acquire) < 4000);
        assert!(chunk.length.load(Ordering::Acquire) <= 1000);
        count += 1;
    }
    assert_eq!(count, 4);

    // Queue should be empty
    assert!(stream.consume_body_chunk().is_none());
}

#[test]
fn test_progress_percentage() {
    let stream = Http3RequestStreamCapsule::new(101, 10000);

    // 0%
    let progress = stream.get_progress().unwrap();
    assert_eq!(progress, 0.0);

    // 25%
    stream.append_body_chunk(0, 2500, false).unwrap();
    let progress = stream.get_progress().unwrap();
    assert!(progress > 0.24 && progress < 0.26);

    // 50%
    stream.append_body_chunk(2500, 2500, false).unwrap();
    let progress = stream.get_progress().unwrap();
    assert!(progress > 0.49 && progress < 0.51);

    // 100%
    stream.append_body_chunk(5000, 5000, true).unwrap();
    let progress = stream.get_progress().unwrap();
    assert!(progress > 0.99 && progress <= 1.0);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn test_ring_buffer_wraparound() {
    let stream = Http3RequestStreamCapsule::new(9, 256000);

    // Add 256 chunks (2 full wraparounds of 128)
    for i in 0..256 {
        let result = stream.append_body_chunk((i * 1000) as u32, 1000, i == 255);
        assert!(result.is_ok(), "Failed at chunk {}", i);
    }

    assert_eq!(stream.get_bytes_received(), 256000);

    // Consume all chunks
    let mut count = 0;
    while stream.consume_body_chunk().is_some() {
        count += 1;
    }
    assert_eq!(count, 256);

    assert!(stream.consume_body_chunk().is_none());
}

#[test]
fn test_chunk_flags() {
    let flags = ChunkFlags::new();
    assert!(!flags.is_fin());

    let flags_fin = flags.with_fin();
    assert!(flags_fin.is_fin());

    // Raw value check
    assert_eq!(flags.raw(), 0);
    assert_eq!(flags_fin.raw(), ChunkFlags::FIN);
}

#[test]
fn test_large_body_streaming() {
    let stream = Http3RequestStreamCapsule::new(102, 1_000_000);

    // Simulate large body with many small chunks
    let mut offset = 0;
    for chunk_id in 0..1000 {
        let fin = chunk_id == 999;
        let result = stream.append_body_chunk(offset, 1000, fin);

        if chunk_id < 128 {
            // First 128 chunks should succeed
            assert!(result.is_ok());
        } else if chunk_id < 256 {
            // Chunks 128-255 will fail with backpressure
            // This is expected behavior - application must consume to continue
            if result.is_err() {
                // Consume one chunk to make room
                stream.consume_body_chunk();
                // Retry should succeed
                let retry = stream.append_body_chunk(offset, 1000, fin);
                assert!(retry.is_ok());
            }
        }

        offset += 1000;
    }
}

#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let stream = Arc::new(Http3RequestStreamCapsule::new(103, 100000));
    let mut handles = vec![];

    // Spawn producer thread
    let producer_stream = Arc::clone(&stream);
    let producer = thread::spawn(move || {
        for i in 0..100 {
            loop {
                match producer_stream.append_body_chunk((i * 1000) as u32, 1000, i == 99) {
                    Ok(()) => break,
                    Err(Http3StreamError::QueueFull) => {
                        // Backpressure - consumer must be slow, retry after tiny delay
                        std::thread::yield_now();
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
        }
    });
    handles.push(producer);

    // Spawn consumer thread
    let consumer_stream = Arc::clone(&stream);
    let consumer = thread::spawn(move || {
        let mut count = 0;
        loop {
            if let Some(chunk) = consumer_stream.consume_body_chunk() {
                count += 1;
            } else if consumer_stream.is_complete() && consumer_stream.get_queue_size() == 0 {
                // All chunks consumed and stream complete
                break;
            } else {
                std::thread::yield_now();
            }
        }
        count
    });
    handles.push(consumer);

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }

    // Verify final state
    assert_eq!(stream.get_bytes_received(), 100000);
    assert_eq!(stream.is_complete(), true);
}

#[test]
fn test_memory_layout() {
    use std::mem;

    // Verify size
    assert_eq!(mem::size_of::<Http3RequestStreamCapsule>(), 2048);

    // Verify alignment
    assert_eq!(mem::align_of::<Http3RequestStreamCapsule>(), 256);

    // Verify BodyChunk size
    assert_eq!(mem::size_of::<BodyChunk>(), 16);
}

#[test]
fn test_zero_length_chunks() {
    let stream = Http3RequestStreamCapsule::new(104, 1000);

    // Append zero-length chunk (valid in HTTP/3)
    assert!(stream.append_body_chunk(0, 0, false).is_ok());
    assert_eq!(stream.get_bytes_received(), 0);

    // Final zero-length chunk with FIN
    assert!(stream.append_body_chunk(0, 0, true).is_ok());
    assert_eq!(stream.is_complete(), true);
}

#[test]
fn test_max_length_chunks() {
    let stream = Http3RequestStreamCapsule::new(105, u32::MAX as u64);

    // Append max-length chunk (u16::MAX = 65535)
    assert!(stream.append_body_chunk(0, u16::MAX, false).is_ok());
    assert_eq!(stream.get_bytes_received(), u16::MAX as u64);

    // Second chunk
    assert!(stream.append_body_chunk(u16::MAX as u32, u16::MAX, true).is_ok());
    assert_eq!(stream.get_bytes_received(), 2 * u16::MAX as u64);
    assert_eq!(stream.is_complete(), true);
}
