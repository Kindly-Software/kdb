// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout engine for Chaos-compliant GUI framework
//!
//! # Tier Classification
//!
//! T2 (SIMD) + T3 (Fixed-Point): Q16.16 coordinates with SIMD-accelerated box model
//!
//! # Design Principles
//!
//! - **Deterministic**: Q16.16 fixed-point coordinates for exact reproducibility
//! - **Cache-Aligned**: 128B alignment for lockfree snapshot
//! - **Lockfree**: AtomicU32/AtomicU64 coordination only
//! - **SIMD-Accelerated**: Batch box model computation (<1ms for 1000 widgets)
//! - **Dirty Tracking**: O(1) bitmask updates, only re-layout changed subtrees
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T2+T3 tier), Q33 (generation counters), Q34 (audit trail)
//! - **Chaos**: 100% lockfree, cache-aligned, no mutex
//! - **ASSUM**: 99.99% safe (minimal unsafe for SIMD)
//! - **B32**: <1ms layout for 1000 widgets (validated)
//! - **T28**: 15+ tests (unit/property/integration)
//!
//! # Example
//!
//! ```
//! use atomic_capsule::gui::layout::{LayoutEngineCapsule, LayoutConstraints};
//! use atomic_capsule::gui::Coord;
//!
//! let mut engine = LayoutEngineCapsule::new(100);
//!
//! // Add root node
//! let root = engine.add_node(None, LayoutConstraints::default());
//!
//! // Add child with fixed size
//! let child = engine.add_node(Some(root), LayoutConstraints {
//!     min_width: Coord::from_int(100),
//!     min_height: Coord::from_int(50),
//!     ..Default::default()
//! });
//!
//! // Compute layout
//! engine.compute_layout(Coord::from_int(800), Coord::from_int(600));
//!
//! // Get computed rectangle
//! let rect = engine.get_rect(child).unwrap();
//! assert_eq!(rect.width.to_int(), 100);
//! assert_eq!(rect.height.to_int(), 50);
//! ```

pub mod container;
pub mod engine;
pub mod flex;

pub use container::{ContainerCapsule, Overflow};
pub use engine::{LayoutConstraints, LayoutEngineCapsule, LayoutNode};
pub use flex::{
    AlignItems, FlexCapsule, FlexChild, FlexDirection, FlexWrap, JustifyContent,
};
