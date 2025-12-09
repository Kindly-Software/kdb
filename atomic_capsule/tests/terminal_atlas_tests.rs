//! TerminalAtlasCapsule T28 Tests
//!
//! Complete test suite for T7 Heterogeneous GPU glyph atlas management.
//!
//! # T28 Test Coverage
//! - Q1-Q7: Unit tests (8 tests)
//! - Q8-Q14: Property tests (4 tests)
//! - Q15-Q21: Integration tests (4 tests)
//!
//! Total: 16 tests

use atomic_capsule::terminal::render::{AtlasRegion, GlyphId, TerminalAtlasCapsule, RenderError};

// ============================================================================
// T28 Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn test_new_atlas() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    assert_eq!(atlas.dimensions(), (2048, 2048));
    assert_eq!(atlas.cell_dimensions(), (16, 32));
    assert_eq!(atlas.capacity(), 64);
    assert_eq!(atlas.allocated_count(), 0);
    assert_eq!(atlas.generation(), 0);
}

#[test]
fn test_allocate_single_region() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let glyph = GlyphId(65); // 'A'

    let region = atlas.allocate_region(glyph).unwrap();

    assert_eq!(region.glyph_id, glyph);
    assert_eq!(region.x, 0); // First cell (0, 0)
    assert_eq!(region.y, 0);
    assert_eq!(region.width, 16);
    assert_eq!(region.height, 32);
    assert_eq!(atlas.allocated_count(), 1);
    assert_eq!(atlas.generation(), 1);
}

#[test]
fn test_lookup_allocated_region() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let glyph = GlyphId(65);

    atlas.allocate_region(glyph).unwrap();

    let found = atlas.lookup_region(glyph).unwrap();
    assert_eq!(found.glyph_id, glyph);
    assert_eq!(found.x, 0);
    assert_eq!(found.y, 0);
}

#[test]
fn test_lookup_missing_region() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let glyph = GlyphId(65);

    assert!(atlas.lookup_region(glyph).is_none());
}

#[test]
fn test_allocate_duplicate_glyph() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let glyph = GlyphId(65);

    atlas.allocate_region(glyph).unwrap();

    let result = atlas.allocate_region(glyph);
    assert_eq!(result, Err(RenderError::AlreadyAllocated));
}

#[test]
fn test_allocate_multiple_regions() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    for i in 0..10 {
        let glyph = GlyphId(65 + i);
        let region = atlas.allocate_region(glyph).unwrap();

        // Verify grid layout (8x8)
        let expected_x = ((i % 8) * 16) as u16;
        let expected_y = ((i / 8) * 32) as u16;

        assert_eq!(region.x, expected_x);
        assert_eq!(region.y, expected_y);
    }

    assert_eq!(atlas.allocated_count(), 10);
    assert_eq!(atlas.generation(), 10);
}

#[test]
fn test_atlas_full() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    // Allocate all 64 regions
    for i in 0..64 {
        let glyph = GlyphId(i);
        atlas.allocate_region(glyph).unwrap();
    }

    // Next allocation should fail
    let result = atlas.allocate_region(GlyphId(100));
    assert_eq!(result, Err(RenderError::AtlasFull));
    assert_eq!(atlas.allocated_count(), 64);
}

#[test]
fn test_evict_lru() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    let glyph1 = GlyphId(65);
    let glyph2 = GlyphId(66);

    atlas.allocate_region(glyph1).unwrap();
    atlas.allocate_region(glyph2).unwrap();

    // Evict first allocated (FIFO)
    let evicted = atlas.evict_lru().unwrap();
    assert_eq!(evicted, glyph1);
    assert_eq!(atlas.allocated_count(), 1);

    // Verify glyph1 no longer in atlas
    assert!(atlas.lookup_region(glyph1).is_none());
    assert!(atlas.lookup_region(glyph2).is_some());
}

// ============================================================================
// T28 Q8-Q14: Property Tests
// ============================================================================

#[test]
fn test_allocation_uniqueness() {
    use std::collections::HashSet;

    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let mut allocated_regions = HashSet::new();

    // Allocate 32 glyphs
    for i in 0..32 {
        let glyph = GlyphId(i);
        let region = atlas.allocate_region(glyph).unwrap();

        // Verify unique coordinates
        let coord = (region.x, region.y);
        assert!(
            allocated_regions.insert(coord),
            "Duplicate region allocation at ({}, {})",
            region.x,
            region.y
        );
    }

    assert_eq!(atlas.allocated_count(), 32);
}

#[test]
fn test_bitmap_consistency() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    // Allocate some regions
    for i in 0..10 {
        atlas.allocate_region(GlyphId(i)).unwrap();
    }

    // Count should match allocated regions
    // Note: We can't directly access the bitmap, but we verify via allocated_count
    assert_eq!(atlas.allocated_count(), 10);
}

#[test]
fn test_generation_monotonicity() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let mut last_gen = 0;

    for i in 0..20 {
        atlas.allocate_region(GlyphId(i)).unwrap();

        let gen = atlas.generation();
        assert!(
            gen > last_gen,
            "Generation not monotonic: {} <= {}",
            gen,
            last_gen
        );
        last_gen = gen;
    }

    // Evictions should also increment generation
    atlas.evict_lru().unwrap();
    let gen_after_evict = atlas.generation();
    assert!(gen_after_evict > last_gen);
}

#[test]
fn test_allocation_count_consistency() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    // Allocate 10
    for i in 0..10 {
        atlas.allocate_region(GlyphId(i)).unwrap();
    }
    assert_eq!(atlas.allocated_count(), 10);

    // Evict 5
    for _ in 0..5 {
        atlas.evict_lru().unwrap();
    }
    assert_eq!(atlas.allocated_count(), 5);

    // Allocate 3 more
    for i in 10..13 {
        atlas.allocate_region(GlyphId(i)).unwrap();
    }
    assert_eq!(atlas.allocated_count(), 8);
}

// ============================================================================
// T28 Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_subpixel_offsets() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);
    let glyph = GlyphId(65);

    atlas.allocate_region(glyph).unwrap();

    // Set subpixel offsets
    atlas.set_subpixel_offset(glyph, 100, 200, 300).unwrap();

    // Verify offsets
    let (r, g, b) = atlas.get_subpixel_offset(glyph).unwrap();
    assert_eq!(r, 100);
    assert_eq!(g, 200);
    assert_eq!(b, 300);
}

#[test]
fn test_allocate_lookup_evict_cycle() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    // Allocate
    let glyph = GlyphId(65);
    let region1 = atlas.allocate_region(glyph).unwrap();
    assert_eq!(atlas.allocated_count(), 1);

    // Lookup
    let region2 = atlas.lookup_region(glyph).unwrap();
    assert_eq!(region1, region2);

    // Evict
    let evicted = atlas.evict_lru().unwrap();
    assert_eq!(evicted, glyph);
    assert_eq!(atlas.allocated_count(), 0);

    // Lookup after eviction
    assert!(atlas.lookup_region(glyph).is_none());

    // Re-allocate (should reuse slot)
    let region3 = atlas.allocate_region(glyph).unwrap();
    assert_eq!(region3.glyph_id, glyph);
    assert_eq!(atlas.allocated_count(), 1);
}

#[test]
fn test_full_atlas_eviction_reallocation() {
    let atlas = TerminalAtlasCapsule::new(2048, 2048, 16, 32);

    // Fill atlas completely
    for i in 0..64 {
        atlas.allocate_region(GlyphId(i)).unwrap();
    }
    assert_eq!(atlas.allocated_count(), 64);

    // Try to allocate new glyph (should fail)
    let result = atlas.allocate_region(GlyphId(100));
    assert_eq!(result, Err(RenderError::AtlasFull));

    // Evict one
    atlas.evict_lru().unwrap();
    assert_eq!(atlas.allocated_count(), 63);

    // Now allocation should succeed
    let region = atlas.allocate_region(GlyphId(100)).unwrap();
    assert_eq!(region.glyph_id, GlyphId(100));
    assert_eq!(atlas.allocated_count(), 64);
}

#[test]
fn test_concurrent_allocation() {
    use std::sync::Arc;
    use std::thread;

    let atlas: Arc<TerminalAtlasCapsule> = Arc::new(TerminalAtlasCapsule::new(2048, 2048, 16, 32));
    let mut handles = vec![];

    // Spawn 8 threads, each allocating 8 glyphs
    for t in 0..8 {
        let atlas_clone: Arc<TerminalAtlasCapsule> = Arc::clone(&atlas);
        let handle = thread::spawn(move || {
            for i in 0..8 {
                let glyph_id = t * 8 + i;
                let result = atlas_clone.allocate_region(GlyphId(glyph_id as u32));
                assert!(result.is_ok(), "Thread {} failed to allocate glyph {}", t, glyph_id);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all 64 regions allocated
    assert_eq!(atlas.allocated_count(), 64);
    assert_eq!(atlas.generation(), 64);
}

// ============================================================================
// Compile-time verification tests
// ============================================================================

#[test]
fn test_size_and_alignment() {
    assert_eq!(core::mem::size_of::<TerminalAtlasCapsule>(), 512);
    assert_eq!(core::mem::align_of::<TerminalAtlasCapsule>(), 64);
}

#[test]
fn test_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<TerminalAtlasCapsule>();
    assert_sync::<TerminalAtlasCapsule>();
}
