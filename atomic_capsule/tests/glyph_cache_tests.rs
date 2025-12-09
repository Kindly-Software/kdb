//! T28 Tests for GlyphCacheCapsule
//!
//! Test coverage:
//! - Q1-Q7: Unit tests (10 tests)
//! - Q8-Q14: Property tests (6 tests)
//! - Q15-Q21: Integration tests (4 tests)
//!
//! Total: 20 tests

#![cfg(all(feature = "std", feature = "terminal-gpu"))]

use atomic_capsule::terminal::render::{
    GlyphCacheCapsule, GlyphEntry, GlyphId, GlyphMetrics, RenderError,
};

// ============================================================================
// Q1-Q7: UNIT TESTS (10 tests)
// ============================================================================

#[test]
fn test_glyph_id_construction() {
    let id = GlyphId::new(1, 0x1234);
    assert_eq!(id.font_id(), 1);
    assert_eq!(id.codepoint(), 0x1234);
}

#[test]
fn test_glyph_id_edge_cases() {
    // Max font_id
    let id = GlyphId::new(255, 0);
    assert_eq!(id.font_id(), 255);
    assert_eq!(id.codepoint(), 0);

    // Max codepoint (24 bits)
    let id = GlyphId::new(0, 0xFFFFFF);
    assert_eq!(id.font_id(), 0);
    assert_eq!(id.codepoint(), 0xFFFFFF);

    // Combined max
    let id = GlyphId::new(255, 0xFFFFFF);
    assert_eq!(id.0, 0xFFFFFFFF);
}

#[test]
fn test_glyph_metrics_q8_8_fixed_point() {
    // Q8.8: 256 = 1.0, 128 = 0.5, etc.
    let metrics = GlyphMetrics::new(256, -128);
    assert_eq!(metrics.advance_x, 256); // 1.0
    assert_eq!(metrics.bearing_y, -128); // -0.5

    // Test conversions
    assert!((metrics.advance_x_f32() - 1.0).abs() < 0.01);
    assert!((metrics.bearing_y_f32() - (-0.5)).abs() < 0.01);
}

#[test]
fn test_glyph_metrics_from_f32() {
    let metrics = GlyphMetrics::from_f32(8.5, -2.25);

    // Q8.8 encoding
    assert_eq!(metrics.advance_x, 2176); // 8.5 * 256
    assert_eq!(metrics.bearing_y, -576); // -2.25 * 256

    // Round-trip verification
    assert!((metrics.advance_x_f32() - 8.5).abs() < 0.01);
    assert!((metrics.bearing_y_f32() - (-2.25)).abs() < 0.01);
}

#[test]
fn test_glyph_entry_default_is_empty() {
    let entry = GlyphEntry::default();
    assert!(entry.is_empty());
    assert_eq!(entry.glyph_id().0, 0);
    assert_eq!(entry.atlas_index, 0);
    assert_eq!(entry.access_count, 0);
}

#[test]
fn test_glyph_entry_construction() {
    let id = GlyphId::new(1, 0x41); // Font 1, 'A'
    let metrics = GlyphMetrics::new(512, 256);
    let entry = GlyphEntry::new(id, 10, metrics, 100);

    assert!(!entry.is_empty());
    assert_eq!(entry.glyph_id(), id);
    assert_eq!(entry.atlas_index, 10);
    assert_eq!(entry.access_count, 1); // Initial access
    assert_eq!(entry.last_access, 100);
    assert_eq!(entry.metrics().advance_x, 512);
    assert_eq!(entry.metrics().bearing_y, 256);
}

#[test]
fn test_cache_construction() {
    let cache = GlyphCacheCapsule::new(16);
    assert_eq!(cache.capacity, 16);
    assert_eq!(cache.eviction_threshold, 12); // 75% of 16

    let (gen, hits, misses) = cache.stats();
    assert_eq!(gen, 0);
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
#[should_panic(expected = "Capacity must be <= 32")]
fn test_cache_capacity_limit() {
    let _cache = GlyphCacheCapsule::new(64); // Should panic
}

#[test]
fn test_cache_default() {
    let cache = GlyphCacheCapsule::default();
    assert_eq!(cache.capacity, 32);
    assert_eq!(cache.eviction_threshold, 24); // 75% of 32
}

#[test]
fn test_cache_stats_initialization() {
    let cache = GlyphCacheCapsule::new(8);

    let (gen, hits, misses) = cache.stats();
    assert_eq!(gen, 0);
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (6 tests)
// ============================================================================

#[test]
fn test_cache_insert_no_duplicates() {
    let cache = GlyphCacheCapsule::new(16);
    let metrics = GlyphMetrics::new(512, 256);

    // Insert same glyph twice
    let id = GlyphId::new(0, 0x41);
    assert!(cache.insert(id, 5, metrics).is_ok());
    assert!(cache.insert(id, 5, metrics).is_ok()); // Should succeed (no-op)

    // Count should be 1
    assert_eq!(cache.count(), 1);
}

#[test]
fn test_cache_lookup_increments_access() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);
    let id = GlyphId::new(0, 0x41);

    // Insert
    assert!(cache.insert(id, 5, metrics).is_ok());

    // First lookup
    let entry1 = cache.lookup(id).unwrap();
    assert_eq!(entry1.access_count, 2); // 1 (insert) + 1 (lookup)

    // Second lookup
    let entry2 = cache.lookup(id).unwrap();
    assert_eq!(entry2.access_count, 3); // Previous + 1
}

#[test]
fn test_cache_miss_tracking() {
    let cache = GlyphCacheCapsule::new(8);

    // Lookup non-existent glyphs
    let id1 = GlyphId::new(0, 0x41);
    let id2 = GlyphId::new(0, 0x42);
    let id3 = GlyphId::new(0, 0x43);

    assert!(cache.lookup(id1).is_none());
    assert!(cache.lookup(id2).is_none());
    assert!(cache.lookup(id3).is_none());

    let (_, hits, misses) = cache.stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 3);
}

#[test]
fn test_cache_hit_tracking() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);

    // Insert 3 glyphs
    for i in 0..3 {
        let id = GlyphId::new(0, i);
        let _ = cache.insert(id, i as u16, metrics);
    }

    // Lookup each twice
    for i in 0..3 {
        let id = GlyphId::new(0, i);
        let _ = cache.lookup(id);
        let _ = cache.lookup(id);
    }

    let (_, hits, misses) = cache.stats();
    assert_eq!(hits, 6); // 3 glyphs × 2 lookups
    assert_eq!(misses, 0);
}

#[test]
fn test_lru_access_ordering() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);

    // Insert 5 glyphs
    for i in 0..5 {
        let id = GlyphId::new(0, i);
        let _ = cache.insert(id, i as u16, metrics);
    }

    // Access pattern: 0, 1, 2, 0, 1
    let _ = cache.lookup(GlyphId::new(0, 0));
    let _ = cache.lookup(GlyphId::new(0, 1));
    let _ = cache.lookup(GlyphId::new(0, 2));
    let _ = cache.lookup(GlyphId::new(0, 0));
    let _ = cache.lookup(GlyphId::new(0, 1));

    // Verify access counts
    let entry0 = cache.lookup(GlyphId::new(0, 0)).unwrap();
    let entry1 = cache.lookup(GlyphId::new(0, 1)).unwrap();
    let entry2 = cache.lookup(GlyphId::new(0, 2)).unwrap();

    assert_eq!(entry0.access_count, 4); // 1 insert + 2 access + 1 verify
    assert_eq!(entry1.access_count, 4); // 1 insert + 2 access + 1 verify
    assert_eq!(entry2.access_count, 3); // 1 insert + 1 access + 1 verify
}

#[test]
fn test_collision_handling() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);

    // Insert many glyphs (may cause hash collisions)
    for i in 0..8 {
        let id = GlyphId::new(0, i * 100); // Spread out codepoints
        let _ = cache.insert(id, i as u16, metrics);
    }

    // Verify all are retrievable
    for i in 0..8 {
        let id = GlyphId::new(0, i * 100);
        let entry = cache.lookup(id);
        assert!(entry.is_some(), "Failed to find glyph {}", i);
        assert_eq!(entry.unwrap().atlas_index, i as u16);
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (4 tests)
// ============================================================================

#[test]
fn test_batch_insert() {
    let cache = GlyphCacheCapsule::new(16);
    let metrics = GlyphMetrics::new(512, 256);

    // Create batch of 10 glyphs
    let glyphs: Vec<_> = (0..10)
        .map(|i| (GlyphId::new(0, i), i as u16, metrics))
        .collect();

    // Batch insert
    let inserted = cache.batch_insert(&glyphs);
    assert_eq!(inserted, 10);
    assert_eq!(cache.count(), 10);

    // Verify all present
    for i in 0..10 {
        let id = GlyphId::new(0, i);
        let entry = cache.lookup(id);
        assert!(entry.is_some(), "Glyph {} not found", i);
        assert_eq!(entry.unwrap().atlas_index, i as u16);
    }
}

#[test]
fn test_eviction() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);

    // Fill beyond threshold (8 * 0.75 = 6)
    for i in 0..7 {
        let id = GlyphId::new(0, i);
        let _ = cache.insert(id, i as u16, metrics);
    }

    assert_eq!(cache.count(), 7);

    // Trigger eviction
    let evicted = cache.evict_if_needed();
    assert_eq!(evicted.len(), 1);
    assert_eq!(cache.count(), 6);

    // Verify evicted glyph is not in cache
    if let Some(evicted_id) = evicted.first() {
        assert!(cache.lookup(*evicted_id).is_none());
    }
}

#[test]
fn test_frame_advance() {
    let cache = GlyphCacheCapsule::new(8);

    let frame0 = cache.current_frame();
    assert_eq!(frame0, 0);

    cache.advance_frame();
    let frame1 = cache.current_frame();
    assert_eq!(frame1, 1);

    cache.advance_frame();
    cache.advance_frame();
    let frame3 = cache.current_frame();
    assert_eq!(frame3, 3);
}

#[test]
fn test_pending_uploads() {
    let cache = GlyphCacheCapsule::new(8);
    let metrics = GlyphMetrics::new(512, 256);

    // Insert 3 glyphs
    let glyphs = vec![
        (GlyphId::new(0, 0), 0, metrics),
        (GlyphId::new(0, 1), 1, metrics),
        (GlyphId::new(0, 2), 2, metrics),
    ];

    for (id, atlas, m) in glyphs {
        let _ = cache.insert(id, atlas, m);
    }

    // Get pending uploads
    let pending = cache.get_pending_uploads();
    assert!(pending.len() >= 1 && pending.len() <= 3);

    // Mark first as uploaded
    if let Some(&first) = pending.first() {
        cache.mark_uploaded(first);

        // Verify pending list updated
        let new_pending = cache.get_pending_uploads();
        // Either list shrunk or specific glyph removed
        assert!(
            new_pending.len() < pending.len() || !new_pending.contains(&first),
            "Pending list not updated after mark_uploaded"
        );
    }
}

// ============================================================================
// PERFORMANCE BENCHMARKS (informational, not tests)
// ============================================================================

#[test]
fn test_lookup_performance_profile() {
    let cache = GlyphCacheCapsule::new(32);
    let metrics = GlyphMetrics::new(512, 256);

    // Fill cache
    for i in 0..32 {
        let id = GlyphId::new(0, i);
        let _ = cache.insert(id, i as u16, metrics);
    }

    // Measure lookup time (informational)
    use std::time::Instant;
    let start = Instant::now();
    for _ in 0..10000 {
        let id = GlyphId::new(0, 15); // Mid-range
        let _ = cache.lookup(id);
    }
    let elapsed = start.elapsed();

    // Target: <50ns per lookup
    // 10000 lookups should be <500μs
    println!(
        "10K lookups: {:?} ({:.2}ns avg)",
        elapsed,
        elapsed.as_nanos() as f64 / 10000.0
    );
    assert!(elapsed.as_micros() < 1000, "Lookups too slow");
}

#[test]
fn test_insert_performance_profile() {
    let cache = GlyphCacheCapsule::new(32);
    let metrics = GlyphMetrics::new(512, 256);

    // Measure insert time (informational)
    use std::time::Instant;
    let start = Instant::now();
    for i in 0..32 {
        let id = GlyphId::new(0, i);
        let _ = cache.insert(id, i as u16, metrics);
    }
    let elapsed = start.elapsed();

    // Target: <100ns per insert
    // 32 inserts should be <3.2μs
    println!(
        "32 inserts: {:?} ({:.2}ns avg)",
        elapsed,
        elapsed.as_nanos() as f64 / 32.0
    );
    assert!(elapsed.as_micros() < 10, "Inserts too slow");
}
