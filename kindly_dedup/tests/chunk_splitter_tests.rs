//! ChunkSplitterCapsule Tests (T28 Framework)
//!
//! Unit, Property, Integration, and Production tests for T5 Streaming
//! zero-copy corpus splitting capsule.
//!
//! Framework: UCE34 Q21-Q28 (Testing)

use kindly_dedup::universal::{ChunkSplitterCapsule, ChunkDescriptor, ChunkSplitterStats};

// ===== UNIT TESTS (Q1-Q7) =====

#[test]
fn test_chunk_splitter_construction() {
    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);
    assert_eq!(splitter.total_docs(), 12_100_000);
    assert_eq!(splitter.num_chunks(), 16);
}

#[test]
fn test_chunk_descriptor_layout() {
    // VERIFY: #ASSUME_ZERO_COPY
    assert_eq!(std::mem::size_of::<ChunkDescriptor>(), 16);
    assert!(std::mem::size_of::<ChunkDescriptor>() <= 32); // Fits in cache line
}

#[test]
fn test_chunk_splitter_alignment() {
    // VERIFY: Cache alignment (64-byte boundary)
    assert_eq!(std::mem::align_of::<ChunkSplitterCapsule>(), 64);
}

#[test]
fn test_chunk_splitter_size() {
    // Expected: 3 × AtomicU64 (24 bytes) + 40 bytes padding = 64 bytes
    assert_eq!(std::mem::size_of::<ChunkSplitterCapsule>(), 64);
}

// ===== PROPERTY TESTS (Q8-Q14) =====

#[test]
fn test_chunk_splitting_preserves_all_documents() {
    // VERIFY: #ASSUME_COMPLETE_COVERAGE
    // Property: ∑(chunk sizes) == total_docs

    for total_docs in [1, 10, 100, 1000, 12_100_000] {
        for num_chunks in [1, 2, 4, 8, 16] {
            let splitter = ChunkSplitterCapsule::new(total_docs, num_chunks);
            let chunks = splitter.split();

            let sum: u64 = chunks.iter().map(|c| c.doc_count()).sum();
            assert_eq!(
                sum, total_docs,
                "Total docs: {}, chunks: {} → sum mismatch",
                total_docs, num_chunks
            );
        }
    }
}

#[test]
fn test_chunk_splitting_even_distribution() {
    // VERIFY: #ASSUME_EVEN_DISTRIBUTION
    // Property: All chunks within ±1 doc of each other

    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);
    let chunks = splitter.split();

    let sizes: Vec<u64> = chunks.iter().map(|c| c.doc_count()).collect();
    let min = *sizes.iter().min().unwrap();
    let max = *sizes.iter().max().unwrap();

    assert!(max - min <= 1, "Uneven distribution: min={}, max={}", min, max);
}

#[test]
fn test_chunk_splitting_non_overlapping() {
    // VERIFY: #ASSUME_NON_OVERLAPPING
    // Property: chunk[i].end == chunk[i+1].start

    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);
    let chunks = splitter.split();

    for i in 0..chunks.len() - 1 {
        assert_eq!(
            chunks[i].end_doc_id, chunks[i + 1].start_doc_id,
            "Chunks {} and {} don't align",
            i, i + 1
        );
    }
}

#[test]
fn test_chunk_splitting_monotonic() {
    // Property: chunk[i].chunk_id is sequential

    let splitter = ChunkSplitterCapsule::new(1_000_000, 8);
    let chunks = splitter.split();

    for (i, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.chunk_id as usize, i);
    }
}

#[test]
fn test_chunk_descriptor_doc_count() {
    let chunk = ChunkDescriptor::new(0, 100, 200);
    assert_eq!(chunk.doc_count(), 100);

    let empty_chunk = ChunkDescriptor::new(1, 500, 500);
    assert_eq!(empty_chunk.doc_count(), 0);
    assert!(empty_chunk.is_empty());
}

// ===== INTEGRATION TESTS (Q15-Q21) =====

#[test]
fn test_chunk_splitter_1k_docs_8_chunks() {
    let splitter = ChunkSplitterCapsule::new(1000, 8);
    let chunks = splitter.split();

    assert_eq!(chunks.len(), 8);
    assert_eq!(chunks[0].start_doc_id, 0);
    assert_eq!(chunks[7].end_doc_id, 1000);

    let total: u64 = chunks.iter().map(|c| c.doc_count()).sum();
    assert_eq!(total, 1000);
}

#[test]
fn test_chunk_splitter_100k_docs_16_chunks() {
    let splitter = ChunkSplitterCapsule::new(100_000, 16);
    let chunks = splitter.split();

    assert_eq!(chunks.len(), 16);
    let sizes: Vec<u64> = chunks.iter().map(|c| c.doc_count()).collect();
    let expected_size = 6250; // 100,000 / 16

    for size in &sizes {
        assert!(*size == expected_size || *size == expected_size);
    }
}

#[test]
fn test_chunk_splitter_get_chunk() {
    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);

    // Valid chunks
    assert!(splitter.get_chunk(0).is_some());
    assert!(splitter.get_chunk(15).is_some());

    // Invalid chunk
    assert!(splitter.get_chunk(16).is_none());
    assert!(splitter.get_chunk(1000).is_none());
}

#[test]
fn test_chunk_splitter_stats() {
    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);
    let stats = splitter.stats();

    assert_eq!(stats.total_docs, 12_100_000);
    assert_eq!(stats.num_chunks, 16);
    assert!(stats.chunk_size > 0);
}

// ===== PRODUCTION TESTS (Q22-Q28) =====

#[test]
fn test_chunk_splitter_c4_12m_docs_16_jobs() {
    // Realistic C4 corpus benchmark
    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);
    let chunks = splitter.split();

    // Verify chunk count
    assert_eq!(chunks.len(), 16);

    // Verify complete coverage
    let total: u64 = chunks.iter().map(|c| c.doc_count()).sum();
    assert_eq!(total, 12_100_000);

    // Verify even distribution (allow ±1 doc variance)
    let sizes: Vec<u64> = chunks.iter().map(|c| c.doc_count()).collect();
    let min = *sizes.iter().min().unwrap();
    let max = *sizes.iter().max().unwrap();
    assert!(max - min <= 1, "Uneven distribution: min={}, max={}", min, max);

    // Verify expected chunk size (~756K docs per chunk)
    let expected_chunk_size = (12_100_000 + 15) / 16; // 756_250
    let actual_chunk_size = splitter.chunk_size();
    assert_eq!(actual_chunk_size, expected_chunk_size);
}

#[test]
fn test_chunk_splitter_extreme_cases() {
    // 1 document
    let splitter = ChunkSplitterCapsule::new(1, 1);
    let chunks = splitter.split();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].doc_count(), 1);

    // More chunks than documents
    let splitter = ChunkSplitterCapsule::new(5, 10);
    let chunks = splitter.split();
    assert_eq!(chunks.len(), 10);
    // Some chunks will be empty (doc_count == 0)
    let non_empty = chunks.iter().filter(|c| !c.is_empty()).count();
    assert_eq!(non_empty, 5);
}

#[test]
fn test_chunk_splitter_atomicity() {
    // Verify atomic loads are consistent
    let splitter = ChunkSplitterCapsule::new(1_000_000, 8);

    let total1 = splitter.total_docs();
    let total2 = splitter.total_docs();
    assert_eq!(total1, total2);

    let chunks1 = splitter.num_chunks();
    let chunks2 = splitter.num_chunks();
    assert_eq!(chunks1, chunks2);
}

#[test]
fn test_chunk_splitter_iter_consistency() {
    // Verify split() returns consistent results across multiple calls
    let splitter = ChunkSplitterCapsule::new(12_100_000, 16);

    let chunks1 = splitter.split();
    let chunks2 = splitter.split();

    assert_eq!(chunks1, chunks2);
}
