//! Compliance modal layout for kindly_dedup GUI v2
//!
//! **Architecture**:
//! - Semi-transparent backdrop (full viewport)
//! - Centered modal card (600x400)
//! - Audit chain status (top section)
//! - Verify button (middle left)
//! - Export button (middle right)
//! - Close button (bottom right)
//!
//! **Framework Compliance**:
//! - **UCE34**: T3 Fixed-Point tier (Q16.16 layout calculations)
//! - **Chaos**: Deterministic layout (same input → same output)
//! - **ASSUM**: Overflow checks on center calculations
//! - **T28**: 5+ tests (centering, button positions, determinism)

use super::Rect;

/// Compliance modal layout configuration
#[derive(Debug, Clone, Copy)]
pub struct ComplianceModalLayout {
    /// Viewport width (pixels)
    viewport_width: u16,
    /// Viewport height (pixels)
    viewport_height: u16,
}

/// Layout regions for compliance modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComplianceModalRegions {
    /// Backdrop (full viewport, semi-transparent)
    pub backdrop: Rect,
    /// Modal card (600x400, centered)
    pub modal_card: Rect,
    /// Audit chain status section
    pub audit_status: Rect,
    /// Verify button
    pub verify_button: Rect,
    /// Export button
    pub export_button: Rect,
    /// Close button (X in top-right)
    pub close_button: Rect,
}

impl ComplianceModalLayout {
    /// Modal width in pixels
    pub const MODAL_WIDTH: u16 = 600;
    /// Modal height in pixels
    pub const MODAL_HEIGHT: u16 = 400;
    /// Modal padding in pixels
    pub const MODAL_PADDING: u16 = 20;
    /// Button width in pixels
    pub const BUTTON_WIDTH: u16 = 120;
    /// Button height in pixels
    pub const BUTTON_HEIGHT: u16 = 40;
    /// Button spacing in pixels
    pub const BUTTON_SPACING: u16 = 10;
    /// Close button size in pixels (square)
    pub const CLOSE_BUTTON_SIZE: u16 = 32;
    /// Audit status height in pixels
    pub const AUDIT_STATUS_HEIGHT: u16 = 200;

    /// Create new layout with viewport dimensions
    ///
    /// # Arguments
    /// - `viewport_width`: Viewport width in pixels
    /// - `viewport_height`: Viewport height in pixels
    #[inline]
    pub const fn new(viewport_width: u16, viewport_height: u16) -> Self {
        Self {
            viewport_width,
            viewport_height,
        }
    }

    /// Calculate all layout regions
    ///
    /// **Deterministic**: Same input → same output (Q16.16 fixed-point)
    pub fn calculate_regions(&self) -> ComplianceModalRegions {
        // Backdrop: full viewport
        let backdrop = Rect::new(0, 0, self.viewport_width, self.viewport_height);

        // Modal card: centered 600x400
        let modal_x = (self.viewport_width.saturating_sub(Self::MODAL_WIDTH)) / 2;
        let modal_y = (self.viewport_height.saturating_sub(Self::MODAL_HEIGHT)) / 2;
        let modal_card = Rect::new(modal_x, modal_y, Self::MODAL_WIDTH, Self::MODAL_HEIGHT);

        // Audit status: top section (inside modal padding)
        let audit_x = modal_x + Self::MODAL_PADDING;
        let audit_y = modal_y + Self::MODAL_PADDING;
        let audit_width = Self::MODAL_WIDTH - 2 * Self::MODAL_PADDING;
        let audit_status = Rect::new(audit_x, audit_y, audit_width, Self::AUDIT_STATUS_HEIGHT);

        // Buttons section: below audit status
        let buttons_y = audit_y + Self::AUDIT_STATUS_HEIGHT + Self::MODAL_PADDING;

        // Verify button (left)
        let verify_x = audit_x;
        let verify_button = Rect::new(verify_x, buttons_y, Self::BUTTON_WIDTH, Self::BUTTON_HEIGHT);

        // Export button (right of verify)
        let export_x = verify_x + Self::BUTTON_WIDTH + Self::BUTTON_SPACING;
        let export_button = Rect::new(export_x, buttons_y, Self::BUTTON_WIDTH, Self::BUTTON_HEIGHT);

        // Close button (X in top-right corner of modal)
        let close_x = modal_x + Self::MODAL_WIDTH - Self::CLOSE_BUTTON_SIZE - Self::MODAL_PADDING;
        let close_y = modal_y + Self::MODAL_PADDING;
        let close_button = Rect::new(close_x, close_y, Self::CLOSE_BUTTON_SIZE, Self::CLOSE_BUTTON_SIZE);

        ComplianceModalRegions {
            backdrop,
            modal_card,
            audit_status,
            verify_button,
            export_button,
            close_button,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_modal_layout_new() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        assert_eq!(layout.viewport_width, 1920);
        assert_eq!(layout.viewport_height, 1080);
    }

    #[test]
    fn test_calculate_regions_desktop() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        // Backdrop: full viewport
        assert_eq!(regions.backdrop.to_pixels(), (0, 0, 1920, 1080));

        // Modal: 600x400, centered
        let (mx, my, mw, mh) = regions.modal_card.to_pixels();
        assert_eq!(mw, ComplianceModalLayout::MODAL_WIDTH);
        assert_eq!(mh, ComplianceModalLayout::MODAL_HEIGHT);
        assert_eq!(mx, (1920 - 600) / 2);
        assert_eq!(my, (1080 - 400) / 2);
    }

    #[test]
    fn test_modal_centered() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        let (modal_x, modal_y, modal_w, modal_h) = regions.modal_card.to_pixels();

        // Check horizontal centering
        let left_margin = modal_x;
        let right_margin = 1920 - (modal_x + modal_w);
        assert_eq!(left_margin, right_margin);

        // Check vertical centering
        let top_margin = modal_y;
        let bottom_margin = 1080 - (modal_y + modal_h);
        assert_eq!(top_margin, bottom_margin);
    }

    #[test]
    fn test_audit_status_position() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        let (modal_x, modal_y, _, _) = regions.modal_card.to_pixels();
        let (audit_x, audit_y, audit_w, audit_h) = regions.audit_status.to_pixels();

        // Should be inside modal with padding
        assert_eq!(audit_x, modal_x + ComplianceModalLayout::MODAL_PADDING);
        assert_eq!(audit_y, modal_y + ComplianceModalLayout::MODAL_PADDING);
        assert_eq!(
            audit_w,
            ComplianceModalLayout::MODAL_WIDTH - 2 * ComplianceModalLayout::MODAL_PADDING
        );
        assert_eq!(audit_h, ComplianceModalLayout::AUDIT_STATUS_HEIGHT);
    }

    #[test]
    fn test_verify_button_position() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        let (audit_x, audit_y, _, audit_h) = regions.audit_status.to_pixels();
        let (verify_x, verify_y, verify_w, verify_h) = regions.verify_button.to_pixels();

        // Should be below audit status, left-aligned
        assert_eq!(verify_x, audit_x);
        assert_eq!(
            verify_y,
            audit_y + audit_h + ComplianceModalLayout::MODAL_PADDING
        );
        assert_eq!(verify_w, ComplianceModalLayout::BUTTON_WIDTH);
        assert_eq!(verify_h, ComplianceModalLayout::BUTTON_HEIGHT);
    }

    #[test]
    fn test_export_button_position() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        let (verify_x, verify_y, verify_w, _) = regions.verify_button.to_pixels();
        let (export_x, export_y, export_w, export_h) = regions.export_button.to_pixels();

        // Should be to the right of verify button
        assert_eq!(
            export_x,
            verify_x + verify_w + ComplianceModalLayout::BUTTON_SPACING
        );
        assert_eq!(export_y, verify_y); // Same Y position
        assert_eq!(export_w, ComplianceModalLayout::BUTTON_WIDTH);
        assert_eq!(export_h, ComplianceModalLayout::BUTTON_HEIGHT);
    }

    #[test]
    fn test_close_button_position() {
        let layout = ComplianceModalLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        let (modal_x, modal_y, modal_w, _) = regions.modal_card.to_pixels();
        let (close_x, close_y, close_w, close_h) = regions.close_button.to_pixels();

        // Should be in top-right corner of modal
        assert_eq!(
            close_x,
            modal_x + modal_w - ComplianceModalLayout::CLOSE_BUTTON_SIZE - ComplianceModalLayout::MODAL_PADDING
        );
        assert_eq!(close_y, modal_y + ComplianceModalLayout::MODAL_PADDING);
        assert_eq!(close_w, ComplianceModalLayout::CLOSE_BUTTON_SIZE);
        assert_eq!(close_h, ComplianceModalLayout::CLOSE_BUTTON_SIZE);
    }

    #[test]
    fn test_determinism() {
        // Same input → same output
        let layout1 = ComplianceModalLayout::new(1920, 1080);
        let regions1 = layout1.calculate_regions();

        let layout2 = ComplianceModalLayout::new(1920, 1080);
        let regions2 = layout2.calculate_regions();

        assert_eq!(regions1, regions2);
    }

    #[test]
    fn test_small_viewport() {
        // Test with small viewport (800x600)
        let layout = ComplianceModalLayout::new(800, 600);
        let regions = layout.calculate_regions();

        // Modal should still be 600x400, centered
        let (mx, my, mw, mh) = regions.modal_card.to_pixels();
        assert_eq!(mw, 600);
        assert_eq!(mh, 400);
        assert_eq!(mx, (800 - 600) / 2);
        assert_eq!(my, (600 - 400) / 2);
    }
}
