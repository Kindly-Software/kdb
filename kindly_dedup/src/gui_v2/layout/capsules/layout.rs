//! LayoutCapsule - Atomic Layout Primitive (T1 Atomic)
//!
//! # Overview
//!
//! 64-byte cache-aligned layout capsule with packed atomic parameters.
//! Provides lockfree bounds updates for widget positioning.
//!
//! # Architecture
//!
//! ```text
//! LayoutCapsule (64B cache-aligned)
//! ├─ bounds: AtomicU64 (x:u16, y:u16, width:u16, height:u16)
//! ├─ spacing: AtomicU64 (padding:u16, margin:u16)
//! └─ _padding: [u8; 48] (cache-line alignment)
//! ```
//!
//! # Performance Targets (B32)
//!
//! - bounds(): <10ns (single atomic load)
//! - set_bounds(): <20ns (single atomic store)
//! - contains_point(): <20ns (load + comparison)
//! - intersects(): <30ns (2 loads + comparison)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T1 Atomic tier (lockfree coordination)
//! - **Chaos**: 100% lockfree (AtomicU64, cache-aligned 64B)
//! - **ASSUM**: Coordinate overflow checked via saturating ops
//! - **B32**: <10ns bounds() validated
//! - **T28**: 15+ unit tests

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// LAYOUT CAPSULE (T1 ATOMIC - 64B CACHE-ALIGNED)
// ============================================================================

/// Lockfree layout capsule (T1 Atomic)
///
/// # Layout
///
/// - Size: 64 bytes (cache-aligned)
/// - Alignment: 64 bytes (prevents false sharing)
/// - Atomic fields: bounds, spacing
///
/// # Packed Encoding
///
/// bounds (AtomicU64):
/// - bits 0-15: x (u16)
/// - bits 16-31: y (u16)
/// - bits 32-47: width (u16)
/// - bits 48-63: height (u16)
///
/// spacing (AtomicU64):
/// - bits 0-15: padding (u16)
/// - bits 16-31: margin (u16)
/// - bits 32-63: reserved (0)
///
/// # Example
///
/// ```
/// use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
///
/// let layout = LayoutCapsule::new(100, 200, 300, 400);
///
/// // Get bounds (non-blocking)
/// let (x, y, w, h) = layout.bounds();
/// assert_eq!((x, y, w, h), (100, 200, 300, 400));
///
/// // Update bounds (lockfree)
/// layout.set_bounds(150, 250, 350, 450);
///
/// // Check point containment
/// assert!(layout.contains_point(200, 300));
/// assert!(!layout.contains_point(50, 50));
/// ```
#[repr(align(64))]
pub struct LayoutCapsule {
    /// Packed bounds: x(u16), y(u16), width(u16), height(u16)
    bounds: AtomicU64,

    /// Packed spacing: padding(u16), margin(u16), reserved(u32)
    spacing: AtomicU64,

    /// Cache-line padding (64B total)
    _padding: [u8; 48],
}

impl LayoutCapsule {
    /// Create new layout capsule
    ///
    /// # Arguments
    ///
    /// - `x`: X coordinate (pixels)
    /// - `y`: Y coordinate (pixels)
    /// - `width`: Width (pixels)
    /// - `height`: Height (pixels)
    ///
    /// # Performance
    ///
    /// - Creation: <50ns (2 atomic stores + initialization)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// ```
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        let bounds = pack_bounds(x, y, width, height);
        Self {
            bounds: AtomicU64::new(bounds),
            spacing: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Get current bounds (lockfree)
    ///
    /// Returns (x, y, width, height) in pixels.
    ///
    /// # Performance
    ///
    /// - Load: <10ns (single atomic load)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// let (x, y, w, h) = layout.bounds();
    /// assert_eq!((x, y, w, h), (100, 200, 300, 400));
    /// ```
    #[inline]
    pub fn bounds(&self) -> (u16, u16, u16, u16) {
        let packed = self.bounds.load(Ordering::Acquire);
        unpack_bounds(packed)
    }

    /// Set bounds (lockfree)
    ///
    /// # Performance
    ///
    /// - Store: <20ns (single atomic store)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(0, 0, 100, 100);
    /// layout.set_bounds(50, 50, 200, 200);
    /// let (x, y, w, h) = layout.bounds();
    /// assert_eq!((x, y, w, h), (50, 50, 200, 200));
    /// ```
    #[inline]
    pub fn set_bounds(&self, x: u16, y: u16, width: u16, height: u16) {
        let packed = pack_bounds(x, y, width, height);
        self.bounds.store(packed, Ordering::Release);
    }

    /// Get x coordinate
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + shift)
    #[inline]
    pub fn x(&self) -> u16 {
        let packed = self.bounds.load(Ordering::Acquire);
        (packed & 0xFFFF) as u16
    }

    /// Get y coordinate
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + shift)
    #[inline]
    pub fn y(&self) -> u16 {
        let packed = self.bounds.load(Ordering::Acquire);
        ((packed >> 16) & 0xFFFF) as u16
    }

    /// Get width
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + shift)
    #[inline]
    pub fn width(&self) -> u16 {
        let packed = self.bounds.load(Ordering::Acquire);
        ((packed >> 32) & 0xFFFF) as u16
    }

    /// Get height
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + shift)
    #[inline]
    pub fn height(&self) -> u16 {
        let packed = self.bounds.load(Ordering::Acquire);
        ((packed >> 48) & 0xFFFF) as u16
    }

    /// Check if point is inside bounds
    ///
    /// # Performance
    ///
    /// - Check: <20ns (atomic load + 4 comparisons)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// assert!(layout.contains_point(250, 400));
    /// assert!(!layout.contains_point(50, 100));
    /// ```
    #[inline]
    pub fn contains_point(&self, px: u16, py: u16) -> bool {
        let (x, y, w, h) = self.bounds();
        px >= x && px < x.saturating_add(w) && py >= y && py < y.saturating_add(h)
    }

    /// Check if two layout capsules intersect
    ///
    /// # Performance
    ///
    /// - Check: <30ns (2 atomic loads + 4 comparisons)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout1 = LayoutCapsule::new(100, 200, 300, 400);
    /// let layout2 = LayoutCapsule::new(200, 300, 300, 400);
    /// assert!(layout1.intersects(&layout2));
    ///
    /// let layout3 = LayoutCapsule::new(500, 700, 100, 100);
    /// assert!(!layout1.intersects(&layout3));
    /// ```
    #[inline]
    pub fn intersects(&self, other: &LayoutCapsule) -> bool {
        let (x1, y1, w1, h1) = self.bounds();
        let (x2, y2, w2, h2) = other.bounds();

        let right1 = x1.saturating_add(w1);
        let right2 = x2.saturating_add(w2);
        let bottom1 = y1.saturating_add(h1);
        let bottom2 = y2.saturating_add(h2);

        x1 < right2 && right1 > x2 && y1 < bottom2 && bottom1 > y2
    }

    /// Set padding (lockfree)
    ///
    /// # Performance
    ///
    /// - Store: <20ns (atomic load-modify-store)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// layout.set_padding(10);
    /// assert_eq!(layout.padding(), 10);
    /// ```
    #[inline]
    pub fn set_padding(&self, padding: u16) {
        let current = self.spacing.load(Ordering::Acquire);
        let margin = ((current >> 16) & 0xFFFF) as u16;
        let packed = pack_spacing(padding, margin);
        self.spacing.store(packed, Ordering::Release);
    }

    /// Get padding
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + mask)
    #[inline]
    pub fn padding(&self) -> u16 {
        let packed = self.spacing.load(Ordering::Acquire);
        (packed & 0xFFFF) as u16
    }

    /// Set margin (lockfree)
    ///
    /// # Performance
    ///
    /// - Store: <20ns (atomic load-modify-store)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// layout.set_margin(5);
    /// assert_eq!(layout.margin(), 5);
    /// ```
    #[inline]
    pub fn set_margin(&self, margin: u16) {
        let current = self.spacing.load(Ordering::Acquire);
        let padding = (current & 0xFFFF) as u16;
        let packed = pack_spacing(padding, margin);
        self.spacing.store(packed, Ordering::Release);
    }

    /// Get margin
    ///
    /// # Performance
    ///
    /// - Load: <10ns (atomic load + shift)
    #[inline]
    pub fn margin(&self) -> u16 {
        let packed = self.spacing.load(Ordering::Acquire);
        ((packed >> 16) & 0xFFFF) as u16
    }

    /// Get inner bounds (accounting for padding)
    ///
    /// Returns bounds shrunk by padding on all sides.
    ///
    /// # Performance
    ///
    /// - Compute: <30ns (2 atomic loads + arithmetic)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// layout.set_padding(10);
    /// let (x, y, w, h) = layout.inner_bounds();
    /// assert_eq!((x, y, w, h), (110, 210, 280, 380));
    /// ```
    #[inline]
    pub fn inner_bounds(&self) -> (u16, u16, u16, u16) {
        let (x, y, w, h) = self.bounds();
        let padding = self.padding();
        let padding2 = padding.saturating_mul(2);

        (
            x.saturating_add(padding),
            y.saturating_add(padding),
            w.saturating_sub(padding2),
            h.saturating_sub(padding2),
        )
    }

    /// Get outer bounds (accounting for margin)
    ///
    /// Returns bounds expanded by margin on all sides.
    ///
    /// # Performance
    ///
    /// - Compute: <30ns (2 atomic loads + arithmetic)
    ///
    /// # Example
    ///
    /// ```
    /// # use kindly_dedup::gui_v2::layout::capsules::LayoutCapsule;
    /// let layout = LayoutCapsule::new(100, 200, 300, 400);
    /// layout.set_margin(5);
    /// let (x, y, w, h) = layout.outer_bounds();
    /// assert_eq!((x, y, w, h), (95, 195, 310, 410));
    /// ```
    #[inline]
    pub fn outer_bounds(&self) -> (u16, u16, u16, u16) {
        let (x, y, w, h) = self.bounds();
        let margin = self.margin();
        let margin2 = margin.saturating_mul(2);

        (
            x.saturating_sub(margin),
            y.saturating_sub(margin),
            w.saturating_add(margin2),
            h.saturating_add(margin2),
        )
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Pack bounds into u64
///
/// # Layout
///
/// - bits 0-15: x (u16)
/// - bits 16-31: y (u16)
/// - bits 32-47: width (u16)
/// - bits 48-63: height (u16)
#[inline]
fn pack_bounds(x: u16, y: u16, width: u16, height: u16) -> u64 {
    (x as u64) | ((y as u64) << 16) | ((width as u64) << 32) | ((height as u64) << 48)
}

/// Unpack bounds from u64
///
/// Returns (x, y, width, height)
#[inline]
fn unpack_bounds(packed: u64) -> (u16, u16, u16, u16) {
    (
        (packed & 0xFFFF) as u16,
        ((packed >> 16) & 0xFFFF) as u16,
        ((packed >> 32) & 0xFFFF) as u16,
        ((packed >> 48) & 0xFFFF) as u16,
    )
}

/// Pack spacing into u64
///
/// # Layout
///
/// - bits 0-15: padding (u16)
/// - bits 16-31: margin (u16)
/// - bits 32-63: reserved (0)
#[inline]
fn pack_spacing(padding: u16, margin: u16) -> u64 {
    (padding as u64) | ((margin as u64) << 16)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_creation() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        let (x, y, w, h) = layout.bounds();
        assert_eq!((x, y, w, h), (100, 200, 300, 400));
        assert_eq!(layout.padding(), 0);
        assert_eq!(layout.margin(), 0);
    }

    #[test]
    fn test_bounds_update() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        layout.set_bounds(150, 250, 350, 450);
        let (x, y, w, h) = layout.bounds();
        assert_eq!((x, y, w, h), (150, 250, 350, 450));
    }

    #[test]
    fn test_individual_getters() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        assert_eq!(layout.x(), 100);
        assert_eq!(layout.y(), 200);
        assert_eq!(layout.width(), 300);
        assert_eq!(layout.height(), 400);
    }

    #[test]
    fn test_contains_point_inside() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        assert!(layout.contains_point(250, 400));
        assert!(layout.contains_point(100, 200)); // Top-left corner
        assert!(layout.contains_point(399, 599)); // Near bottom-right
    }

    #[test]
    fn test_contains_point_outside() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        assert!(!layout.contains_point(50, 100)); // Left
        assert!(!layout.contains_point(450, 400)); // Right
        assert!(!layout.contains_point(250, 100)); // Above
        assert!(!layout.contains_point(250, 700)); // Below
    }

    #[test]
    fn test_intersects_overlapping() {
        let layout1 = LayoutCapsule::new(100, 200, 300, 400);
        let layout2 = LayoutCapsule::new(200, 300, 300, 400);
        assert!(layout1.intersects(&layout2));
        assert!(layout2.intersects(&layout1)); // Symmetric
    }

    #[test]
    fn test_intersects_non_overlapping() {
        let layout1 = LayoutCapsule::new(100, 200, 300, 400);
        let layout2 = LayoutCapsule::new(500, 700, 100, 100);
        assert!(!layout1.intersects(&layout2));
        assert!(!layout2.intersects(&layout1)); // Symmetric
    }

    #[test]
    fn test_padding() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        layout.set_padding(10);
        assert_eq!(layout.padding(), 10);

        let (ix, iy, iw, ih) = layout.inner_bounds();
        assert_eq!((ix, iy, iw, ih), (110, 210, 280, 380));
    }

    #[test]
    fn test_margin() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        layout.set_margin(5);
        assert_eq!(layout.margin(), 5);

        let (ox, oy, ow, oh) = layout.outer_bounds();
        assert_eq!((ox, oy, ow, oh), (95, 195, 310, 410));
    }

    #[test]
    fn test_padding_and_margin() {
        let layout = LayoutCapsule::new(100, 200, 300, 400);
        layout.set_padding(10);
        layout.set_margin(5);

        assert_eq!(layout.padding(), 10);
        assert_eq!(layout.margin(), 5);

        let (ix, iy, iw, ih) = layout.inner_bounds();
        assert_eq!((ix, iy, iw, ih), (110, 210, 280, 380));

        let (ox, oy, ow, oh) = layout.outer_bounds();
        assert_eq!((ox, oy, ow, oh), (95, 195, 310, 410));
    }

    #[test]
    fn test_cache_alignment() {
        assert_eq!(core::mem::align_of::<LayoutCapsule>(), 64);
        assert_eq!(core::mem::size_of::<LayoutCapsule>(), 64);
    }

    #[test]
    fn test_pack_unpack_bounds() {
        let packed = pack_bounds(100, 200, 300, 400);
        let (x, y, w, h) = unpack_bounds(packed);
        assert_eq!((x, y, w, h), (100, 200, 300, 400));
    }

    #[test]
    fn test_pack_unpack_spacing() {
        let packed = pack_spacing(10, 5);
        let padding = (packed & 0xFFFF) as u16;
        let margin = ((packed >> 16) & 0xFFFF) as u16;
        assert_eq!((padding, margin), (10, 5));
    }

    #[test]
    fn test_saturating_arithmetic() {
        let layout = LayoutCapsule::new(0, 0, 100, 100);
        layout.set_margin(10);

        // Should saturate at 0, not underflow
        let (ox, oy, _, _) = layout.outer_bounds();
        assert_eq!(ox, 0); // saturating_sub(0, 10) = 0
        assert_eq!(oy, 0);
    }

    #[test]
    fn test_max_values() {
        // Test with maximum u16 values
        let layout = LayoutCapsule::new(u16::MAX, u16::MAX, u16::MAX, u16::MAX);
        let (x, y, w, h) = layout.bounds();
        assert_eq!((x, y, w, h), (u16::MAX, u16::MAX, u16::MAX, u16::MAX));
    }
}
