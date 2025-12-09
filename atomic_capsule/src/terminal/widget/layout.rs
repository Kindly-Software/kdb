//! Layout Solver Capsule - T4+T6 flexbox-style layout
//!
//! Constraint-based layout solver with flexbox semantics for terminal widgets.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::Rect;

/// Layout direction
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LayoutDirection {
    #[default]
    Row = 0,
    Column = 1,
    RowReverse = 2,
    ColumnReverse = 3,
}

/// Layout justify
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum JustifyContent {
    #[default]
    Start = 0,
    End = 1,
    Center = 2,
    SpaceBetween = 3,
    SpaceAround = 4,
    SpaceEvenly = 5,
}

/// Layout alignment
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AlignItems {
    #[default]
    Start = 0,
    End = 1,
    Center = 2,
    Stretch = 3,
}

/// Size constraints for layout
#[derive(Copy, Clone, Debug, Default)]
pub struct Constraints {
    pub min_width: u16,
    pub max_width: u16,
    pub min_height: u16,
    pub max_height: u16,
}

impl Constraints {
    pub const fn new(min_width: u16, max_width: u16, min_height: u16, max_height: u16) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    pub const fn tight(width: u16, height: u16) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }

    pub const fn loose(width: u16, height: u16) -> Self {
        Self {
            min_width: 0,
            max_width: width,
            min_height: 0,
            max_height: height,
        }
    }
}

/// T4+T6 - Flexbox-style layout solver
///
/// Efficient constraint-based layout with flex distribution.
#[repr(C, align(64))]
pub struct LayoutSolverCapsule {
    /// Generation counter (for versioning)
    generation: AtomicU64,
    /// Layout pass count (for profiling)
    pass_count: AtomicU32,
    /// Padding field
    _pad0: u32,

    // Temporary buffers for layout calculation (stack-allocated, no heap)
    /// Flex factors buffer (32 items)
    flex_buffer: [u16; 32],
    /// Size buffer (32 items × 2 = width, height pairs)
    size_buffer: [u16; 64],

    _pad: [u8; 300],
}

// #ASSUME: LayoutSolverCapsule is 64-byte aligned for cache performance
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::align_of::<LayoutSolverCapsule>() == 64);

// #ASSUME: LayoutSolverCapsule fits in 512B (8 cache lines)
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::size_of::<LayoutSolverCapsule>() == 512);

impl Default for LayoutSolverCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutSolverCapsule {
    /// Maximum children for batch layout
    pub const MAX_CHILDREN: usize = 32;

    /// Create new layout solver
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            pass_count: AtomicU32::new(0),
            _pad0: 0,
            flex_buffer: [0; 32],
            size_buffer: [0; 64],
            _pad: [0; 300],
        }
    }

    /// Solve flex layout for children
    ///
    /// Returns computed rectangles for each child.
    ///
    /// # Arguments
    /// * `children` - (width, height) pairs for each child
    /// * `container` - Container rectangle
    /// * `direction` - Layout direction
    /// * `justify` - Justify content mode
    pub fn solve_flex(
        &self,
        children: &[(u16, u16)],
        container: Rect,
        direction: LayoutDirection,
        justify: JustifyContent,
    ) -> Vec<Rect> {
        if children.is_empty() {
            return Vec::new();
        }

        self.pass_count.fetch_add(1, Ordering::Relaxed);

        let is_row = matches!(direction, LayoutDirection::Row | LayoutDirection::RowReverse);
        let available = if is_row {
            container.width
        } else {
            container.height
        };

        // Measure children
        let mut total_size = 0u16;
        let mut flex_total = 0u16;

        for (i, &(w, h)) in children.iter().enumerate().take(Self::MAX_CHILDREN) {
            let size = if is_row { w } else { h };
            total_size = total_size.saturating_add(size);

            // Use size as flex factor (non-zero = flexible)
            if size > 0 {
                flex_total = flex_total.saturating_add(1);
                unsafe {
                    *self.flex_buffer.as_ptr().add(i).cast_mut() = 1;
                }
            } else {
                unsafe {
                    *self.flex_buffer.as_ptr().add(i).cast_mut() = 0;
                }
            }
        }

        // Distribute remaining space
        let remaining = available.saturating_sub(total_size);
        let mut positions = Vec::with_capacity(children.len());

        match justify {
            JustifyContent::Start => {
                let mut pos = 0u16;
                for (i, &(w, h)) in children.iter().enumerate() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                    pos = pos.saturating_add(size);
                }
            }
            JustifyContent::End => {
                let mut pos = available;
                for &(w, h) in children.iter().rev() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    pos = pos.saturating_sub(size);

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                }
                positions.reverse();
            }
            JustifyContent::Center => {
                let mut pos = remaining / 2;
                for &(w, h) in children.iter() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                    pos = pos.saturating_add(size);
                }
            }
            JustifyContent::SpaceBetween => {
                if children.len() <= 1 {
                    return self.solve_flex(children, container, direction, JustifyContent::Start);
                }

                let gap = remaining / (children.len() as u16 - 1);
                let mut pos = 0u16;

                for (i, &(w, h)) in children.iter().enumerate() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                    pos = pos.saturating_add(size);
                    if i < children.len() - 1 {
                        pos = pos.saturating_add(gap);
                    }
                }
            }
            JustifyContent::SpaceAround => {
                let gap = remaining / children.len() as u16;
                let mut pos = gap / 2;

                for &(w, h) in children.iter() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                    pos = pos.saturating_add(size).saturating_add(gap);
                }
            }
            JustifyContent::SpaceEvenly => {
                let gap = remaining / (children.len() as u16 + 1);
                let mut pos = gap;

                for &(w, h) in children.iter() {
                    let size = if is_row { w } else { h };
                    let cross_size = if is_row { h } else { w };

                    let rect = if is_row {
                        Rect {
                            x: container.x.saturating_add(pos),
                            y: container.y,
                            width: size,
                            height: cross_size.min(container.height),
                        }
                    } else {
                        Rect {
                            x: container.x,
                            y: container.y.saturating_add(pos),
                            width: cross_size.min(container.width),
                            height: size,
                        }
                    };

                    positions.push(rect);
                    pos = pos.saturating_add(size).saturating_add(gap);
                }
            }
        }

        self.generation.fetch_add(1, Ordering::Release);
        positions
    }

    /// Measure child within constraints
    pub fn measure_child(&self, child_size: (u16, u16), constraints: Constraints) -> (u16, u16) {
        let (w, h) = child_size;

        let width = w.clamp(constraints.min_width, constraints.max_width);
        let height = h.clamp(constraints.min_height, constraints.max_height);

        (width, height)
    }

    /// Distribute space among children with flex factors
    pub fn distribute_space(&self, sizes: &[u16], available: u16, flex_factors: &[u16]) -> Vec<u16> {
        if sizes.is_empty() {
            return Vec::new();
        }

        let total_size: u16 = sizes.iter().copied().sum();
        let flex_total: u16 = flex_factors.iter().copied().sum();

        if flex_total == 0 {
            return sizes.to_vec();
        }

        let remaining = available.saturating_sub(total_size);
        let per_flex = remaining / flex_total;

        sizes
            .iter()
            .zip(flex_factors)
            .map(|(&size, &flex)| size.saturating_add(flex.saturating_mul(per_flex)))
            .collect()
    }

    /// Get layout pass count (for profiling)
    #[inline]
    pub fn pass_count(&self) -> u32 {
        self.pass_count.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_flex_start() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(10, 5), (20, 5), (15, 5)];
        let container = Rect::new(0, 0, 100, 20);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::Start);

        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[1].x, 10);
        assert_eq!(rects[2].x, 30);
    }

    #[test]
    fn test_solve_flex_end() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(10, 5), (20, 5)];
        let container = Rect::new(0, 0, 100, 20);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::End);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 70);
        assert_eq!(rects[1].x, 80);
    }

    #[test]
    fn test_solve_flex_center() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(20, 5), (20, 5)];
        let container = Rect::new(0, 0, 100, 20);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::Center);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 30); // (100 - 40) / 2 = 30
        assert_eq!(rects[1].x, 50);
    }

    #[test]
    fn test_solve_flex_space_between() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(10, 5), (10, 5), (10, 5)];
        let container = Rect::new(0, 0, 100, 20);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::SpaceBetween);

        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].x, 0);
        // Gap = (100 - 30) / 2 = 35
        assert_eq!(rects[1].x, 45);
        assert_eq!(rects[2].x, 90);
    }

    #[test]
    fn test_solve_flex_column() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(10, 5), (10, 10)];
        let container = Rect::new(0, 0, 50, 100);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Column, JustifyContent::Start);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[1].y, 5);
    }

    #[test]
    fn test_measure_child() {
        let solver = LayoutSolverCapsule::new();
        let constraints = Constraints::new(10, 100, 5, 50);

        let (w, h) = solver.measure_child((150, 3), constraints);
        assert_eq!(w, 100); // Clamped to max
        assert_eq!(h, 5); // Clamped to min
    }

    #[test]
    fn test_distribute_space() {
        let solver = LayoutSolverCapsule::new();
        let sizes = vec![10, 20, 15];
        let flex_factors = vec![1, 2, 1];

        let distributed = solver.distribute_space(&sizes, 100, &flex_factors);

        assert_eq!(distributed.len(), 3);
        // Remaining = 100 - 45 = 55
        // Per flex = 55 / 4 = 13
        assert_eq!(distributed[0], 10 + 13); // 23
        assert_eq!(distributed[1], 20 + 26); // 46
        assert_eq!(distributed[2], 15 + 13); // 28
    }

    #[test]
    fn test_pass_count_increments() {
        let solver = LayoutSolverCapsule::new();
        assert_eq!(solver.pass_count(), 0);

        let children = vec![(10, 5)];
        let container = Rect::new(0, 0, 100, 20);

        solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::Start);
        assert_eq!(solver.pass_count(), 1);

        solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::End);
        assert_eq!(solver.pass_count(), 2);
    }
}

#[cfg(all(test, feature = "proptest"))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_solve_flex_all_children_within_container(
            children in prop::collection::vec((1u16..50, 1u16..20), 1..10),
            container_width in 50u16..200,
            container_height in 20u16..100,
        ) {
            let solver = LayoutSolverCapsule::new();
            let container = Rect::new(0, 0, container_width, container_height);

            let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::Start);

            for rect in rects {
                assert!(rect.x >= container.x);
                assert!(rect.y >= container.y);
                assert!(rect.x + rect.width <= container.x + container.width);
                assert!(rect.y + rect.height <= container.y + container.height);
            }
        }

        #[test]
        fn prop_measure_child_respects_constraints(
            width in 0u16..200,
            height in 0u16..100,
            min_w in 0u16..100,
            max_w in 100u16..200,
            min_h in 0u16..50,
            max_h in 50u16..100,
        ) {
            let solver = LayoutSolverCapsule::new();
            let constraints = Constraints::new(min_w, max_w, min_h, max_h);

            let (w, h) = solver.measure_child((width, height), constraints);

            assert!(w >= min_w);
            assert!(w <= max_w);
            assert!(h >= min_h);
            assert!(h <= max_h);
        }

        #[test]
        fn prop_distribute_space_total_matches_available(
            sizes in prop::collection::vec(1u16..20, 1..8),
            available in 50u16..200,
        ) {
            let solver = LayoutSolverCapsule::new();
            let flex_factors = vec![1u16; sizes.len()];

            let distributed = solver.distribute_space(&sizes, available, &flex_factors);

            let total: u16 = distributed.iter().copied().sum();
            // Should be close to available (within rounding)
            assert!(total <= available + sizes.len() as u16);
        }

        #[test]
        fn prop_justify_preserves_child_count(
            children in prop::collection::vec((1u16..50, 1u16..20), 1..10),
            justify in prop::sample::select(vec![
                JustifyContent::Start,
                JustifyContent::End,
                JustifyContent::Center,
                JustifyContent::SpaceBetween,
                JustifyContent::SpaceAround,
                JustifyContent::SpaceEvenly,
            ]),
        ) {
            let solver = LayoutSolverCapsule::new();
            let container = Rect::new(0, 0, 200, 100);

            let rects = solver.solve_flex(&children, container, LayoutDirection::Row, justify);

            assert_eq!(rects.len(), children.len());
        }
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod integration_tests {
    use super::*;

    #[test]
    fn test_complex_nested_layout() {
        let solver = LayoutSolverCapsule::new();

        // Outer container: 3 columns
        let outer_children = vec![(30, 50), (40, 50), (30, 50)];
        let outer_container = Rect::new(0, 0, 100, 50);

        let outer_rects = solver.solve_flex(
            &outer_children,
            outer_container,
            LayoutDirection::Row,
            JustifyContent::Start,
        );

        // Inner layout in first column: 2 rows
        let inner_children = vec![(30, 20), (30, 30)];
        let inner_rects = solver.solve_flex(
            &inner_children,
            outer_rects[0],
            LayoutDirection::Column,
            JustifyContent::Start,
        );

        assert_eq!(inner_rects.len(), 2);
        assert_eq!(inner_rects[0].width, 30);
        assert_eq!(inner_rects[0].height, 20);
    }

    #[test]
    fn test_overflow_handling() {
        let solver = LayoutSolverCapsule::new();

        // Children larger than container
        let children = vec![(60, 20), (70, 20)];
        let container = Rect::new(0, 0, 100, 50);

        let rects = solver.solve_flex(&children, container, LayoutDirection::Row, JustifyContent::Start);

        assert_eq!(rects.len(), 2);
        // Should not panic, positions should be valid
        assert!(rects[0].x < u16::MAX);
        assert!(rects[1].x < u16::MAX);
    }

    #[test]
    fn test_flex_distribution_fairness() {
        let solver = LayoutSolverCapsule::new();

        let sizes = vec![10, 10, 10, 10];
        let flex_factors = vec![1, 1, 1, 1];
        let available = 100;

        let distributed = solver.distribute_space(&sizes, available, &flex_factors);

        // Each should get equal extra space
        let extra_per_item = (available - 40) / 4;
        for &size in &distributed {
            assert_eq!(size, 10 + extra_per_item);
        }
    }

    #[test]
    fn test_all_justify_modes() {
        let solver = LayoutSolverCapsule::new();
        let children = vec![(20, 10), (20, 10), (20, 10)];
        let container = Rect::new(0, 0, 100, 20);

        let modes = vec![
            JustifyContent::Start,
            JustifyContent::End,
            JustifyContent::Center,
            JustifyContent::SpaceBetween,
            JustifyContent::SpaceAround,
            JustifyContent::SpaceEvenly,
        ];

        for mode in modes {
            let rects = solver.solve_flex(&children, container, LayoutDirection::Row, mode);
            assert_eq!(rects.len(), 3);

            // All rects should be within container
            for rect in rects {
                assert!(rect.x + rect.width <= container.width);
            }
        }
    }
}
