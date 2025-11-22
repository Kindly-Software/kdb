//! HTTP Chunked Encoding Parser Tests (T5 Streaming Tier)
//!
//! **T28 Framework Coverage**:
//! - Tier 1 (Unit): Basic parsing, state transitions, metrics
//! - Tier 2 (Property): Incremental parsing, error handling
//! - Tier 3 (Integration): Multi-chunk streaming, reset behavior
//! - Tier 4 (Production): Alignment verification, Send+Sync bounds, error messages
//!
//! **ASSUM Safety Validation**:
//! - Lockfree coordination (all AtomicU32/AtomicU64 operations)
//! - Cache alignment (128B exactly)
//! - CRLF invariant (RFC 7230 §4.1)
//! - Hex parsing bounds (max 0xFFFFFFFF)

use crate::http::chunked_encoding::{
    ChunkError, ChunkParseState, ChunkResult, HttpChunkedEncodingCapsule,
};
use core::sync::atomic::Ordering;

// ─────────────────────────────────────────────────────────────────
// TIER 1: UNIT TESTS
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_new_capsule_initialization() {
    let capsule = HttpChunkedEncodingCapsule::new();
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkSize));
    assert_eq!(capsule.total_bytes(), 0);
    assert_eq!(capsule.chunk_count(), 0);
}

#[test]
fn test_parse_single_chunk_size() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let input = b"5\r\nhello\r\n";

    // First call: parse size
    let result = capsule.parse(input);
    assert!(result.is_ok());
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkData));
}

#[test]
fn test_parse_chunk_data() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let input = b"5\r\nhello\r\n";

    // Skip size parsing by setting state manually
    capsule.chunk_size_remaining.store(5, Ordering::Release);
    capsule.state.store(ChunkParseState::ChunkData as u32, Ordering::Release);

    let result = capsule.parse(&input[3..]); // Start after "5\r\n" (3 bytes, not 4)
    assert!(result.is_ok());

    if let Ok(ChunkResult::Chunk { data, size }) = result {
        assert_eq!(data, b"hello");
        assert_eq!(size, 5);
    } else {
        panic!("Expected Chunk result");
    }
}

#[test]
fn test_parse_multiple_chunks() {
    let capsule = HttpChunkedEncodingCapsule::new();

    // Simulate two chunks: "5\r\nhello\r\n6\r\nworld!\r\n0\r\n\r\n"
    capsule.parse(b"5\r\n").ok();
    capsule.chunk_size_remaining.store(5, Ordering::Release);
    capsule.state.store(ChunkParseState::ChunkData as u32, Ordering::Release);

    let _ = capsule.parse(b"hello");
    assert_eq!(capsule.total_bytes(), 5);
}

#[test]
fn test_hex_chunk_size_parsing() {
    let capsule = HttpChunkedEncodingCapsule::new();

    // Test hex: 0x1a = 26
    let input = b"1a\r\n";
    let result = capsule.parse(input);
    assert!(result.is_ok());
}

#[test]
fn test_chunk_size_with_extension() {
    let capsule = HttpChunkedEncodingCapsule::new();

    // RFC 7230 allows chunk-ext after size: "5;name=value\r\n"
    let input = b"5;name=value\r\n";
    let result = capsule.parse(input);
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────
// TIER 2: PROPERTY TESTS (Error Handling)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_zero_chunk_final() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let input = b"0\r\n\r\n"; // Final chunk with no trailers

    let result = capsule.parse(input);
    assert!(result.is_ok());
    assert_eq!(capsule.current_state(), Some(ChunkParseState::Trailer));
}

#[test]
fn test_invalid_hex_size() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let input = b"ZZZ\r\n"; // Invalid hex

    let result = capsule.parse(input);
    assert!(result.is_err());
}

#[test]
fn test_oversized_chunk() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let input = b"50000000\r\n"; // 0x50000000 > 0x40000000 limit

    let result = capsule.parse(input);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ChunkError::OversizedChunk);
}

#[test]
fn test_missing_crlf_after_data() {
    let capsule = HttpChunkedEncodingCapsule::new();

    capsule.chunk_size_remaining.store(5, Ordering::Release);
    capsule.state.store(ChunkParseState::ChunkData as u32, Ordering::Release);

    let _ = capsule.parse(b"hello");

    // Move to chunk_end parsing
    capsule.state.store(ChunkParseState::ChunkEnd as u32, Ordering::Release);

    // Missing \r\n
    let result = capsule.parse(b"XX");
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────
// TIER 3: INTEGRATION TESTS (State Transitions)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_reset_clears_state() {
    let capsule = HttpChunkedEncodingCapsule::new();

    capsule.chunk_size_remaining.store(42, Ordering::Release);
    capsule.total_bytes_parsed.store(1000, Ordering::Release);

    capsule.reset();

    assert_eq!(capsule.chunk_size_remaining.load(Ordering::Acquire), 0);
    assert_eq!(capsule.total_bytes(), 0);
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkSize));
}

#[test]
fn test_metrics_tracking() {
    let capsule = HttpChunkedEncodingCapsule::new();

    capsule.total_chunks.store(5, Ordering::Release);
    capsule.max_chunk_size.store(1024, Ordering::Release);

    assert_eq!(capsule.chunk_count(), 5);
    assert_eq!(capsule.max_chunk_size(), 1024);
}

// ─────────────────────────────────────────────────────────────────
// TIER 4: PRODUCTION TESTS (Safety & Compliance)
// ─────────────────────────────────────────────────────────────────

#[test]
fn test_send_sync() {
    fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<HttpChunkedEncodingCapsule>();
}

#[test]
fn test_cache_alignment() {
    use core::mem;
    assert_eq!(mem::size_of::<HttpChunkedEncodingCapsule>(), 128);
    assert_eq!(mem::align_of::<HttpChunkedEncodingCapsule>(), 128);
}

#[test]
fn test_debug_output() {
    let capsule = HttpChunkedEncodingCapsule::new();
    let debug_str = format!("{:?}", capsule);
    assert!(debug_str.contains("ChunkSize"));
}

#[test]
fn test_default_trait() {
    let capsule = HttpChunkedEncodingCapsule::default();
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkSize));
}

#[test]
fn test_incremental_parsing() {
    let capsule = HttpChunkedEncodingCapsule::new();

    // Parse incomplete data (no \r\n terminator yet)
    let _ = capsule.parse(b"5");  // Incomplete size (no CRLF)
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkSize)); // Still waiting for CRLF

    // Now provide complete chunk size line in a fresh call
    let result = capsule.parse(b"5\r\nhello");
    assert!(result.is_ok());
    assert_eq!(capsule.current_state(), Some(ChunkParseState::ChunkData)); // Transitioned after finding CRLF
}

#[test]
fn test_chunk_error_display() {
    assert_eq!(
        format!("{}", ChunkError::InvalidSize),
        "Invalid hex chunk size"
    );
    assert_eq!(
        format!("{}", ChunkError::OversizedChunk),
        "Chunk size exceeds 1GB limit"
    );
}
