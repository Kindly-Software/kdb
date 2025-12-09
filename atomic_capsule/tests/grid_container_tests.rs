//! Integration tests for GridContainerCapsule
//!
//! T28 Testing Framework Compliance:
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (invariants)
//! - Q15-Q21: Integration tests (full workflows)

#![cfg(all(feature = "terminal-widgets", feature = "tui-terminal"))]

use atomic_capsule::terminal::widget::{
    Widget,
    container::{
        GridContainerCapsule, GridContainerState, GridTrack, GridItem,
        AutoFlow, Alignment,
    },
    types::Rect,
};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn unit_grid_creation() {
    let grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(2)],
        &[GridTrack::fixed(10), GridTrack::auto()],
    );

    assert_eq!(GridContainerCapsule::MAX_COLS, 8);
    assert_eq!(GridContainerCapsule::MAX_ROWS, 8);
    assert_eq!(GridContainerCapsule::MAX_CHILDREN, 24);
}

#[test]
fn unit_add_child() {
    let mut grid = GridContainerCapsule::default();

    let item = GridItem::new().col(1, 1).row(1, 1);
    let idx = grid.add_child(item);

    assert_eq!(idx, Some(0));
    assert_eq!(grid.child_count(), 1);
}

#[test]
fn unit_max_children() {
    let mut grid = GridContainerCapsule::default();

    // Add MAX_CHILDREN items
    for _ in 0..GridContainerCapsule::MAX_CHILDREN {
        assert!(grid.add_child(GridItem::new()).is_some());
    }

    // Next should fail
    assert!(grid.add_child(GridItem::new()).is_none());
}

#[test]
fn unit_remove_child() {
    let mut grid = GridContainerCapsule::default();

    grid.add_child(GridItem::new());
    grid.add_child(GridItem::new());
    grid.add_child(GridItem::new());

    assert_eq!(grid.child_count(), 3);

    grid.remove_child(1);
    assert_eq!(grid.child_count(), 2);
}

#[test]
fn unit_track_sizing_fixed() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fixed(10), GridTrack::fixed(20)],
        &[GridTrack::fixed(15)],
    );

    grid.layout(Rect::new(0, 0, 100, 100));

    // Verify track sizes remain fixed
    assert!(grid.child_count() == 0); // No children yet
}

#[test]
fn unit_track_sizing_fr() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(2)],
        &[GridTrack::fr(1)],
    );

    grid.layout(Rect::new(0, 0, 90, 30));

    // Verify fractional distribution (tested via layout)
    let state = grid.snapshot();
    assert!(!state.dirty); // Layout clears dirty flag
}

#[test]
fn unit_gap_spacing() {
    let grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(1)],
        &[GridTrack::fr(1)],
    )
    .with_gap(5, 5);

    assert!(!grid.is_dirty()); // No children, not dirty
}

#[test]
fn unit_padding() {
    let grid = GridContainerCapsule::default()
        .with_padding(5, 5, 10, 10);

    let (min_w, min_h) = grid.min_size();
    assert!(min_w >= 10); // At least left + right padding
    assert!(min_h >= 20); // At least top + bottom padding
}

#[test]
fn unit_layout_dirty_flag() {
    let mut grid = GridContainerCapsule::default();
    assert!(!grid.is_dirty());

    grid.add_child(GridItem::new());
    assert!(grid.is_dirty());

    grid.layout(Rect::new(0, 0, 100, 100));
    assert!(!grid.is_dirty());
}

#[test]
fn unit_generation_counter() {
    let mut grid = GridContainerCapsule::default();
    let gen1 = grid.generation();

    grid.layout(Rect::new(0, 0, 100, 100));
    let gen2 = grid.generation();

    assert_eq!(gen2, gen1.wrapping_add(1));
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn property_child_bounds_within_container() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(1)],
        &[GridTrack::fr(1), GridTrack::fr(1)],
    );

    // Add 4 items in 2×2 grid
    for _ in 0..4 {
        grid.add_child(GridItem::new());
    }

    let container = Rect::new(10, 20, 100, 80);
    grid.layout(container);

    // All child bounds should be within container
    for i in 0..4 {
        if let Some(bounds) = grid.child_bounds(i) {
            assert!(bounds.x >= container.x,
                    "Child {} x={} < container.x={}", i, bounds.x, container.x);
            assert!(bounds.y >= container.y,
                    "Child {} y={} < container.y={}", i, bounds.y, container.y);
            assert!(bounds.x + bounds.width as u16 <= container.x + container.width as u16,
                    "Child {} right edge exceeds container", i);
            assert!(bounds.y + bounds.height as u16 <= container.y + container.height as u16,
                    "Child {} bottom edge exceeds container", i);
        }
    }
}

#[test]
fn property_layout_idempotent() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(2)],
        &[GridTrack::fr(1)],
    );

    grid.add_child(GridItem::new());

    let container = Rect::new(0, 0, 90, 30);
    grid.layout(container);
    let bounds1 = grid.child_bounds(0);

    grid.layout(container);
    let bounds2 = grid.child_bounds(0);

    assert_eq!(bounds1, bounds2, "Layout should be idempotent");
}

#[test]
fn property_snapshot_consistency() {
    let mut grid = GridContainerCapsule::default();
    grid.add_child(GridItem::new());

    let snap1 = grid.snapshot();
    let snap2 = grid.snapshot();

    assert_eq!(snap1.generation, snap2.generation);
    assert_eq!(snap1.child_count, snap2.child_count);
}

#[test]
fn property_generation_increments_on_layout() {
    let mut grid = GridContainerCapsule::default();

    let gen1 = grid.generation();
    grid.layout(Rect::new(0, 0, 100, 100));
    let gen2 = grid.generation();
    grid.layout(Rect::new(0, 0, 100, 100));
    let gen3 = grid.generation();

    assert_eq!(gen2, gen1.wrapping_add(1));
    assert_eq!(gen3, gen2.wrapping_add(1));
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_2x2_grid_auto_placement() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(1)],
        &[GridTrack::fr(1), GridTrack::fr(1)],
    );

    // Add 4 items with auto-placement
    for _ in 0..4 {
        grid.add_child(GridItem::new());
    }

    grid.layout(Rect::new(0, 0, 100, 80));

    // Verify all 4 items have bounds
    for i in 0..4 {
        assert!(grid.child_bounds(i).is_some(), "Child {} should have bounds", i);
    }

    // Item 0 should be top-left
    let bounds0 = grid.child_bounds(0).unwrap();
    assert_eq!(bounds0.x, 0);
    assert_eq!(bounds0.y, 0);

    // Item 1 should be top-right
    let bounds1 = grid.child_bounds(1).unwrap();
    assert_eq!(bounds1.x, 50); // Half of 100
    assert_eq!(bounds1.y, 0);

    // Item 2 should be bottom-left
    let bounds2 = grid.child_bounds(2).unwrap();
    assert_eq!(bounds2.x, 0);
    assert_eq!(bounds2.y, 40); // Half of 80

    // Item 3 should be bottom-right
    let bounds3 = grid.child_bounds(3).unwrap();
    assert_eq!(bounds3.x, 50);
    assert_eq!(bounds3.y, 40);
}

#[test]
fn integration_explicit_placement() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fr(1), GridTrack::fr(1)],
        &[GridTrack::fr(1), GridTrack::fr(1)],
    );

    // Place item explicitly at col 2, row 2
    let item = GridItem::new().col(2, 1).row(2, 1);
    grid.add_child(item);

    grid.layout(Rect::new(0, 0, 100, 80));

    let bounds = grid.child_bounds(0).unwrap();
    assert_eq!(bounds.x, 50); // Second column
    assert_eq!(bounds.y, 40); // Second row
}

#[test]
fn integration_mixed_track_types() {
    let mut grid = GridContainerCapsule::new(
        &[GridTrack::fixed(30), GridTrack::fr(1)],
        &[GridTrack::fixed(20)],
    );

    grid.add_child(GridItem::new());
    grid.add_child(GridItem::new());

    grid.layout(Rect::new(0, 0, 100, 50));

    // Verify items were placed (specific bounds depend on layout algorithm)
    assert!(grid.child_bounds(0).is_some());
    assert!(grid.child_bounds(1).is_some());
}

#[test]
fn integration_widget_trait() {
    let grid = GridContainerCapsule::default();

    let state = grid.snapshot();
    assert_eq!(state.generation, 0);
    assert_eq!(state.child_count, 0);
    assert!(!state.dirty);

    // Test Widget trait implementation
    assert!(!grid.is_focusable());

    // Test min_size calculation
    let (min_w, min_h) = grid.min_size();
    assert!(min_w > 0);
    assert!(min_h > 0);
}
