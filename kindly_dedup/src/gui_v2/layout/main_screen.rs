//! Main screen layout for kindly_dedup GUI v2
//!
//! **Architecture**:
//! - Header (fixed top, 80px)
//! - Scrollable content area:
//!   - File input card (glassmorphic)
//!   - Settings card
//!   - Action button (centered, gold)
//!   - Progress card (when processing)
//!   - Results card (when complete)
//!   - Feature badges (3-column grid)
//! - Footer (fixed bottom, 40px)
//! - Max-width: 900px, centered
//!
//! **Framework Compliance**:
//! - **UCE34**: T3 Fixed-Point tier (Q16.16 layout calculations)
//! - **Chaos**: Deterministic layout (same input → same output)
//! - **ASSUM**: Overflow checks on viewport bounds
//! - **T28**: 10+ tests (responsive, bounds, determinism)

use super::Rect;

/// Main screen layout configuration
#[derive(Debug, Clone, Copy)]
pub struct MainScreenLayout {
    /// Viewport width (pixels)
    viewport_width: u16,
    /// Viewport height (pixels)
    viewport_height: u16,
    /// Scroll offset (Q16.16 fixed-point)
    scroll_offset: i32,
}

/// Layout regions for main screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainScreenRegions {
    /// Header region (fixed top, 80px)
    pub header: Rect,
    /// Content container (scrollable, max 900px wide, centered)
    pub content_container: Rect,
    /// File input card
    pub file_input_card: Rect,
    /// Settings card
    pub settings_card: Rect,
    /// Action button (centered)
    pub action_button: Rect,
    /// Progress card (when processing)
    pub progress_card: Rect,
    /// Results card (when complete)
    pub results_card: Rect,
    /// Feature badges grid (3 columns)
    pub feature_badges: [Rect; 6],
    /// Footer region (fixed bottom, 40px)
    pub footer: Rect,
}

impl MainScreenLayout {
    /// Header height in pixels
    pub const HEADER_HEIGHT: u16 = 80;
    /// Footer height in pixels
    pub const FOOTER_HEIGHT: u16 = 40;
    /// Content max width in pixels
    pub const CONTENT_MAX_WIDTH: u16 = 900;
    /// Content min width in pixels
    pub const CONTENT_MIN_WIDTH: u16 = 600;
    /// Card spacing in pixels
    pub const CARD_SPACING: u16 = 20;
    /// Card height in pixels
    pub const CARD_HEIGHT: u16 = 120;
    /// Action button width in pixels
    pub const ACTION_BUTTON_WIDTH: u16 = 200;
    /// Action button height in pixels
    pub const ACTION_BUTTON_HEIGHT: u16 = 50;
    /// Feature badge width in pixels
    pub const BADGE_WIDTH: u16 = 280;
    /// Feature badge height in pixels
    pub const BADGE_HEIGHT: u16 = 80;

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
            scroll_offset: 0,
        }
    }

    /// Set scroll offset (Q16.16 fixed-point)
    #[inline]
    pub const fn with_scroll(mut self, scroll_offset: i32) -> Self {
        self.scroll_offset = scroll_offset;
        self
    }

    /// Calculate all layout regions
    ///
    /// **Deterministic**: Same input → same output (Q16.16 fixed-point)
    pub fn calculate_regions(&self) -> MainScreenRegions {
        // Fixed header (top)
        let header = Rect::new(0, 0, self.viewport_width, Self::HEADER_HEIGHT);

        // Fixed footer (bottom)
        let footer_y = self.viewport_height.saturating_sub(Self::FOOTER_HEIGHT);
        let footer = Rect::new(0, footer_y, self.viewport_width, Self::FOOTER_HEIGHT);

        // Content area (between header and footer)
        let content_height = self
            .viewport_height
            .saturating_sub(Self::HEADER_HEIGHT)
            .saturating_sub(Self::FOOTER_HEIGHT);

        // Content width (clamped to 600-900px, centered)
        let content_width = self.viewport_width.clamp(Self::CONTENT_MIN_WIDTH, Self::CONTENT_MAX_WIDTH);
        let content_x = (self.viewport_width.saturating_sub(content_width)) / 2;

        // Content container (scrollable)
        let content_container = Rect::new(content_x, Self::HEADER_HEIGHT, content_width, content_height);

        // Apply scroll offset to content Y positions
        let scroll_offset = self.scroll_offset >> 16; // Convert Q16.16 to pixels
        let mut y_pos = Self::HEADER_HEIGHT.saturating_add_signed(-(scroll_offset as i16));

        // File input card
        y_pos = y_pos.saturating_add(Self::CARD_SPACING);
        let file_input_card = Rect::new(content_x, y_pos, content_width, Self::CARD_HEIGHT);

        // Settings card
        y_pos = y_pos.saturating_add(Self::CARD_HEIGHT).saturating_add(Self::CARD_SPACING);
        let settings_card = Rect::new(content_x, y_pos, content_width, Self::CARD_HEIGHT);

        // Action button (centered horizontally)
        y_pos = y_pos.saturating_add(Self::CARD_HEIGHT).saturating_add(Self::CARD_SPACING);
        let button_x = content_x + (content_width - Self::ACTION_BUTTON_WIDTH) / 2;
        let action_button = Rect::new(button_x, y_pos, Self::ACTION_BUTTON_WIDTH, Self::ACTION_BUTTON_HEIGHT);

        // Progress card
        y_pos = y_pos
            .saturating_add(Self::ACTION_BUTTON_HEIGHT)
            .saturating_add(Self::CARD_SPACING);
        let progress_card = Rect::new(content_x, y_pos, content_width, Self::CARD_HEIGHT);

        // Results card
        y_pos = y_pos.saturating_add(Self::CARD_HEIGHT).saturating_add(Self::CARD_SPACING);
        let results_card = Rect::new(content_x, y_pos, content_width, Self::CARD_HEIGHT * 2);

        // Feature badges (3 columns, 2 rows)
        y_pos = y_pos
            .saturating_add(Self::CARD_HEIGHT * 2)
            .saturating_add(Self::CARD_SPACING);
        // Use saturating_sub to prevent underflow on narrow viewports
        let total_badge_width = Self::BADGE_WIDTH.saturating_mul(3);
        let badge_spacing = content_width.saturating_sub(total_badge_width) / 4;
        let mut feature_badges = [Rect::new(0, 0, 0, 0); 6];

        for row in 0..2 {
            for col in 0..3 {
                let badge_x = content_x + badge_spacing + (Self::BADGE_WIDTH + badge_spacing) * col;
                let badge_y = y_pos + (Self::BADGE_HEIGHT + Self::CARD_SPACING) * row;
                feature_badges[row as usize * 3 + col as usize] =
                    Rect::new(badge_x, badge_y, Self::BADGE_WIDTH, Self::BADGE_HEIGHT);
            }
        }

        MainScreenRegions {
            header,
            content_container,
            file_input_card,
            settings_card,
            action_button,
            progress_card,
            results_card,
            feature_badges,
            footer,
        }
    }

    /// Get content scroll height (total scrollable height)
    pub const fn scroll_height(&self) -> u16 {
        // Sum all card heights + spacing
        Self::CARD_SPACING + // Top spacing
        Self::CARD_HEIGHT + Self::CARD_SPACING + // File input
        Self::CARD_HEIGHT + Self::CARD_SPACING + // Settings
        Self::ACTION_BUTTON_HEIGHT + Self::CARD_SPACING + // Action button
        Self::CARD_HEIGHT + Self::CARD_SPACING + // Progress
        Self::CARD_HEIGHT * 2 + Self::CARD_SPACING + // Results
        Self::BADGE_HEIGHT * 2 + Self::CARD_SPACING + // Badges (2 rows)
        Self::CARD_SPACING // Bottom spacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_screen_layout_new() {
        let layout = MainScreenLayout::new(1920, 1080);
        assert_eq!(layout.viewport_width, 1920);
        assert_eq!(layout.viewport_height, 1080);
        assert_eq!(layout.scroll_offset, 0);
    }

    #[test]
    fn test_main_screen_layout_with_scroll() {
        let layout = MainScreenLayout::new(1920, 1080).with_scroll(100 << 16);
        assert_eq!(layout.scroll_offset, 100 << 16);
    }

    #[test]
    fn test_calculate_regions_desktop() {
        let layout = MainScreenLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        // Header: full width, 80px tall
        assert_eq!(regions.header.to_pixels(), (0, 0, 1920, 80));

        // Footer: full width, 40px tall, at bottom
        assert_eq!(regions.footer.to_pixels(), (0, 1040, 1920, 40));

        // Content: max 900px wide, centered
        let (cx, _, cw, _) = regions.content_container.to_pixels();
        assert_eq!(cw, 900); // Max width
        assert_eq!(cx, (1920 - 900) / 2); // Centered

        // File input card: first card in content
        let (_, fy, _, _) = regions.file_input_card.to_pixels();
        assert_eq!(fy, 80 + MainScreenLayout::CARD_SPACING);
    }

    #[test]
    fn test_calculate_regions_mobile() {
        let layout = MainScreenLayout::new(600, 800);
        let regions = layout.calculate_regions();

        // Content: min 600px wide (viewport width)
        let (cx, _, cw, _) = regions.content_container.to_pixels();
        assert_eq!(cw, 600); // Min width
        assert_eq!(cx, 0); // No centering (full width)
    }

    #[test]
    fn test_calculate_regions_with_scroll() {
        let layout = MainScreenLayout::new(1920, 1080).with_scroll(50 << 16); // 50px scroll
        let regions = layout.calculate_regions();

        // File input card should move up by scroll offset
        let (_, fy, _, _) = regions.file_input_card.to_pixels();
        assert_eq!(fy, 80 + MainScreenLayout::CARD_SPACING - 50);
    }

    #[test]
    fn test_action_button_centered() {
        let layout = MainScreenLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        // Action button should be centered horizontally
        let (bx, _, bw, _) = regions.action_button.to_pixels();
        let content_x = (1920 - 900) / 2;
        let expected_x = content_x + (900 - MainScreenLayout::ACTION_BUTTON_WIDTH) / 2;
        assert_eq!(bx, expected_x);
        assert_eq!(bw, MainScreenLayout::ACTION_BUTTON_WIDTH);
    }

    #[test]
    fn test_feature_badges_grid() {
        let layout = MainScreenLayout::new(1920, 1080);
        let regions = layout.calculate_regions();

        // Should have 6 badges (3 columns × 2 rows)
        assert_eq!(regions.feature_badges.len(), 6);

        // All badges should have same size
        for badge in &regions.feature_badges {
            let (_, _, w, h) = badge.to_pixels();
            assert_eq!(w, MainScreenLayout::BADGE_WIDTH);
            assert_eq!(h, MainScreenLayout::BADGE_HEIGHT);
        }

        // Verify row alignment
        let (_, y0, _, _) = regions.feature_badges[0].to_pixels();
        let (_, y1, _, _) = regions.feature_badges[1].to_pixels();
        let (_, y2, _, _) = regions.feature_badges[2].to_pixels();
        assert_eq!(y0, y1); // Row 1
        assert_eq!(y1, y2);

        let (_, y3, _, _) = regions.feature_badges[3].to_pixels();
        let (_, y4, _, _) = regions.feature_badges[4].to_pixels();
        let (_, y5, _, _) = regions.feature_badges[5].to_pixels();
        assert_eq!(y3, y4); // Row 2
        assert_eq!(y4, y5);

        // Rows should be different
        assert_ne!(y0, y3);
    }

    #[test]
    fn test_scroll_height() {
        let layout = MainScreenLayout::new(1920, 1080);
        let scroll_height = layout.scroll_height();

        // Should be sum of all cards + spacing
        let expected = MainScreenLayout::CARD_SPACING + // Top
            MainScreenLayout::CARD_HEIGHT + MainScreenLayout::CARD_SPACING + // File input
            MainScreenLayout::CARD_HEIGHT + MainScreenLayout::CARD_SPACING + // Settings
            MainScreenLayout::ACTION_BUTTON_HEIGHT + MainScreenLayout::CARD_SPACING + // Button
            MainScreenLayout::CARD_HEIGHT + MainScreenLayout::CARD_SPACING + // Progress
            MainScreenLayout::CARD_HEIGHT * 2 + MainScreenLayout::CARD_SPACING + // Results
            MainScreenLayout::BADGE_HEIGHT * 2 + MainScreenLayout::CARD_SPACING + // Badges
            MainScreenLayout::CARD_SPACING; // Bottom

        assert_eq!(scroll_height, expected);
    }

    #[test]
    fn test_determinism() {
        // Same input → same output
        let layout1 = MainScreenLayout::new(1920, 1080);
        let regions1 = layout1.calculate_regions();

        let layout2 = MainScreenLayout::new(1920, 1080);
        let regions2 = layout2.calculate_regions();

        assert_eq!(regions1, regions2);
    }

    #[test]
    fn test_responsive_width_clamping() {
        // Test min width (600px)
        let layout_min = MainScreenLayout::new(500, 800);
        let regions_min = layout_min.calculate_regions();
        let (_, _, w_min, _) = regions_min.content_container.to_pixels();
        assert_eq!(w_min, 600);

        // Test max width (900px)
        let layout_max = MainScreenLayout::new(2000, 1080);
        let regions_max = layout_max.calculate_regions();
        let (_, _, w_max, _) = regions_max.content_container.to_pixels();
        assert_eq!(w_max, 900);

        // Test in-between (700px)
        let layout_mid = MainScreenLayout::new(700, 800);
        let regions_mid = layout_mid.calculate_regions();
        let (_, _, w_mid, _) = regions_mid.content_container.to_pixels();
        assert_eq!(w_mid, 700);
    }
}
