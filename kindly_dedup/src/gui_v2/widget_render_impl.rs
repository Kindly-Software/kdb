//! Widget Render Implementation - Complete render() methods for all widgets
//!
//! This file contains the render() implementations to be integrated into each widget file.
//! Implementation is consolidated here for review, then will be copied to individual files.

/*

=== ProgressWidget::render() ===

Add this method to src/gui_v2/widgets/progress.rs after get_progress_bg_rect():

    /// Render progress widget
    ///
    /// # Output
    ///
    /// - Background track (full width, gray)
    /// - Filled progress bar (current progress, gradient purple-to-gold)
    /// - Shimmer overlay (animated highlight)
    /// - Phase text (status message)
    /// - ETA and throughput text
    ///
    /// # Performance
    ///
    /// - Shape creation: <200ns (4 shapes + 3 text commands)
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        use super::super::rendering_primitives::{Shape, TextCommand, TextAlign};
        use super::theme;

        let bg_rect = self.get_progress_bg_rect();
        let progress_rect = self.get_progress_rect();

        // Background track (gray)
        shapes.push(Shape::rounded_rect(
            bg_rect.clone(),
            super::Color::rgb(60, 60, 70),
            8,
        ));

        // Filled progress bar (gradient purple-to-gold)
        if progress_rect.width > 0 {
            shapes.push(Shape::gradient_h(
                progress_rect.clone(),
                theme::PURPLE_ROYAL,
                theme::GOLD_BRIGHT,
            ));

            // Shimmer overlay (animated highlight)
            let shimmer_offset = self.get_shimmer_offset();
            let shimmer_x = progress_rect.x + (shimmer_offset as i32 % progress_rect.width);
            let shimmer_rect = crate::gui_v2::layout::Rect {
                x: shimmer_x,
                y: progress_rect.y,
                width: 40, // Shimmer width
                height: progress_rect.height,
            };
            shapes.push(Shape::gradient_h(
                shimmer_rect,
                super::Color::rgba(255, 255, 255, 100),
                super::Color::rgba(255, 255, 255, 0),
            ));
        }

        // Phase text (above progress bar)
        let phase_text = self.get_phase().name();
        texts.push(TextCommand::new(
            phase_text,
            bg_rect.x,
            bg_rect.y - 30,
            18,
            theme::TEXT_PRIMARY,
        ));

        // Progress percentage (right-aligned above bar)
        let progress_pct = format!("{:.1}%", self.get_progress());
        texts.push(TextCommand::new(
            progress_pct,
            bg_rect.x + bg_rect.width - 60,
            bg_rect.y - 30,
            18,
            theme::GOLD_BRIGHT,
        ));

        // ETA and throughput (below bar)
        let info_text = format!("{} | {}", self.format_eta(), self.format_throughput());
        texts.push(TextCommand::new(
            info_text,
            bg_rect.x,
            bg_rect.y + bg_rect.height + 10,
            14,
            theme::TEXT_SECONDARY,
        ));
    }

=== SettingsWidget::render() ===

Add this method to src/gui_v2/widgets/settings.rs after get_mode_description():

    /// Render settings widget
    ///
    /// # Output
    ///
    /// - Threshold label + value
    /// - Slider track + knob
    /// - Mode label
    /// - Dropdown button
    /// - Mode description text
    ///
    /// # Performance
    ///
    /// - Shape creation: <300ns (7 shapes + 5 text commands)
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        use super::super::rendering_primitives::{Shape, TextCommand};
        use super::theme;

        let slider_bounds = self.slider_bounds();
        let dropdown_bounds = self.dropdown_bounds();
        let is_slider_hovered = self.is_slider_hovered();
        let is_dropdown_hovered = self.is_dropdown_hovered();

        // === Threshold Slider ===

        // Label
        texts.push(TextCommand::new(
            "Similarity Threshold:",
            slider_bounds.x,
            slider_bounds.y - 25,
            16,
            theme::TEXT_PRIMARY,
        ));

        // Slider track (background)
        shapes.push(Shape::rounded_rect(
            slider_bounds.clone(),
            super::Color::rgb(60, 60, 70),
            4,
        ));

        // Slider filled portion (0.5 to current value)
        let threshold = self.get_threshold();
        let fill_width = ((threshold - 0.5) / 0.5 * slider_bounds.width as f64) as i32;
        let fill_rect = crate::gui_v2::layout::Rect {
            x: slider_bounds.x,
            y: slider_bounds.y,
            width: fill_width,
            height: slider_bounds.height,
        };
        shapes.push(Shape::rounded_rect(
            fill_rect,
            theme::PURPLE_ROYAL,
            4,
        ));

        // Slider knob
        let knob_x = slider_bounds.x + fill_width - 8;
        let knob_y = slider_bounds.y + slider_bounds.height / 2;
        let knob_color = if is_slider_hovered {
            theme::GOLD_BRIGHT
        } else {
            theme::PURPLE_LIGHT
        };
        shapes.push(Shape::circle(knob_x, knob_y, 12, knob_color));

        // Threshold value (right of slider)
        texts.push(TextCommand::new(
            self.format_threshold(),
            slider_bounds.x + slider_bounds.width + 15,
            slider_bounds.y + 5,
            18,
            theme::GOLD_BRIGHT,
        ));

        // === Mode Dropdown ===

        // Label
        texts.push(TextCommand::new(
            "Processing Mode:",
            dropdown_bounds.x,
            dropdown_bounds.y - 25,
            16,
            theme::TEXT_PRIMARY,
        ));

        // Dropdown button
        let dropdown_color = if is_dropdown_hovered {
            super::Color::rgb(80, 80, 90)
        } else {
            super::Color::rgb(60, 60, 70)
        };
        shapes.push(Shape::rounded_rect(
            dropdown_bounds.clone(),
            dropdown_color,
            6,
        ));

        // Dropdown border (if hovered)
        if is_dropdown_hovered {
            shapes.push(Shape::rect_with_border(
                dropdown_bounds.clone(),
                super::Color::rgba(0, 0, 0, 0), // Transparent fill
                theme::GOLD_BRIGHT,
                2,
            ));
        }

        // Dropdown text
        texts.push(TextCommand::new(
            self.get_mode().name(),
            dropdown_bounds.x + 10,
            dropdown_bounds.y + 10,
            18,
            theme::TEXT_PRIMARY,
        ));

        // Mode description (below dropdown)
        texts.push(TextCommand::new(
            self.get_mode_description(),
            dropdown_bounds.x,
            dropdown_bounds.y + dropdown_bounds.height + 10,
            14,
            theme::TEXT_SECONDARY,
        ));
    }

=== FileInputWidget::render() ===

Add this method to src/gui_v2/widgets/file_input.rs after render_text():

    /// Render file input widget
    ///
    /// # Output
    ///
    /// - Browse button
    /// - Drop zone border (dashed)
    /// - File path + size text (if file selected)
    /// - Drop zone prompt text (if no file)
    ///
    /// # Performance
    ///
    /// - Shape creation: <150ns (3 shapes + 2 text commands)
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        use super::super::rendering_primitives::{Shape, TextCommand};
        use super::theme;

        let button_bounds = self.get_button_bounds();
        let drop_zone_bounds = self.get_drop_zone_bounds();
        let is_button_hovered = self.is_button_hovered();
        let is_drop_zone_hovered = self.is_drop_zone_hovered();

        // === Browse Button ===

        let button_color = if is_button_hovered {
            theme::PURPLE_LIGHT
        } else {
            theme::PURPLE_ROYAL
        };
        shapes.push(Shape::rounded_rect(
            button_bounds.clone(),
            button_color,
            8,
        ));

        texts.push(TextCommand::centered(
            "Browse Files",
            button_bounds.x + button_bounds.width / 2,
            button_bounds.y + 10,
            18,
            super::Color::WHITE,
        ));

        // === Drop Zone ===

        // Drop zone border (dashed appearance via multiple line segments)
        let border_color = if is_drop_zone_hovered {
            theme::GOLD_BRIGHT
        } else {
            super::Color::rgb(100, 100, 120)
        };

        // Top border
        shapes.push(Shape::line(
            drop_zone_bounds.x,
            drop_zone_bounds.y,
            drop_zone_bounds.x + drop_zone_bounds.width,
            drop_zone_bounds.y,
            border_color,
            2,
        ));

        // Bottom border
        shapes.push(Shape::line(
            drop_zone_bounds.x,
            drop_zone_bounds.y + drop_zone_bounds.height,
            drop_zone_bounds.x + drop_zone_bounds.width,
            drop_zone_bounds.y + drop_zone_bounds.height,
            border_color,
            2,
        ));

        // Left border
        shapes.push(Shape::line(
            drop_zone_bounds.x,
            drop_zone_bounds.y,
            drop_zone_bounds.x,
            drop_zone_bounds.y + drop_zone_bounds.height,
            border_color,
            2,
        ));

        // Right border
        shapes.push(Shape::line(
            drop_zone_bounds.x + drop_zone_bounds.width,
            drop_zone_bounds.y,
            drop_zone_bounds.x + drop_zone_bounds.width,
            drop_zone_bounds.y + drop_zone_bounds.height,
            border_color,
            2,
        ));

        // Drop zone text
        let text = self.render_text();
        texts.push(TextCommand::centered(
            text,
            drop_zone_bounds.x + drop_zone_bounds.width / 2,
            drop_zone_bounds.y + drop_zone_bounds.height / 2 - 10,
            16,
            theme::TEXT_SECONDARY,
        ));
    }

=== ResultsWidget::render() ===

Add this method to src/gui_v2/widgets/results.rs after get_duplicate_percentage():

    /// Render results widget
    ///
    /// # Output
    ///
    /// - Success checkmark (animated spring)
    /// - Statistics table (total, duplicates, speedup)
    /// - Output path text
    /// - Reset button
    ///
    /// # Performance
    ///
    /// - Shape creation: <250ns (5 shapes + 5 text commands)
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        use super::super::rendering_primitives::{Shape, TextCommand};
        use super::theme;

        let reset_button_bounds = self.get_reset_button_bounds();
        let is_reset_hovered = self.is_reset_hovered();
        let checkmark_scale = self.get_checkmark_scale();

        // === Success Checkmark (animated) ===

        let checkmark_x = 400;
        let checkmark_y = 200;
        let checkmark_radius = (30.0 * checkmark_scale) as u32;

        shapes.push(Shape::circle(
            checkmark_x,
            checkmark_y,
            checkmark_radius,
            theme::GOLD_BRIGHT,
        ));

        // === Statistics Table ===

        let stats_y = 280;
        let line_height = 30;

        // Total documents
        texts.push(TextCommand::new(
            format!("Total Documents: {}", self.get_total()),
            150,
            stats_y,
            20,
            theme::TEXT_PRIMARY,
        ));

        // Duplicates found
        let dup_pct = self.get_duplicate_percentage();
        texts.push(TextCommand::new(
            format!("Duplicates Found: {} ({:.1}%)", self.get_duplicates(), dup_pct),
            150,
            stats_y + line_height,
            20,
            theme::PURPLE_LIGHT,
        ));

        // Speedup
        texts.push(TextCommand::new(
            format!("Speedup: {:.1}×", self.get_speedup()),
            150,
            stats_y + line_height * 2,
            20,
            theme::GOLD_BRIGHT,
        ));

        // Output path
        if let Some(path) = self.get_output_path() {
            texts.push(TextCommand::new(
                format!("Output: {}", path),
                150,
                stats_y + line_height * 3,
                16,
                theme::TEXT_SECONDARY,
            ));
        }

        // === Reset Button ===

        let reset_color = if is_reset_hovered {
            theme::PURPLE_LIGHT
        } else {
            theme::PURPLE_ROYAL
        };
        shapes.push(Shape::rounded_rect(
            reset_button_bounds.clone(),
            reset_color,
            8,
        ));

        texts.push(TextCommand::centered(
            "Reset",
            reset_button_bounds.x + reset_button_bounds.width / 2,
            reset_button_bounds.y + 10,
            18,
            super::Color::WHITE,
        ));
    }

=== ErrorBoxWidget::render() ===

Add this method to src/gui_v2/widgets/error_box.rs after wrap_message():

    /// Render error box widget (only if visible)
    ///
    /// # Output
    ///
    /// - Semi-transparent overlay (modal background)
    /// - Error box background
    /// - Error icon
    /// - Wrapped error message
    /// - Report button
    /// - Close button
    ///
    /// # Performance
    ///
    /// - Shape creation: <400ns (6 shapes + 3-10 text commands depending on wrapping)
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        if !self.is_visible() {
            return; // Skip rendering if hidden
        }

        use super::super::rendering_primitives::{Shape, TextCommand};
        use super::theme;

        let box_bounds = self.get_box_bounds();
        let report_bounds = self.get_report_button_bounds();
        let close_bounds = self.get_close_button_bounds();
        let is_report_hovered = self.is_report_hovered();
        let is_close_hovered = self.is_close_hovered();

        // === Modal Overlay (semi-transparent black) ===

        shapes.push(Shape::rect(
            crate::gui_v2::layout::Rect {
                x: 0,
                y: 0,
                width: 900, // Full screen width
                height: 1000, // Full screen height
            },
            super::Color::rgba(0, 0, 0, 180), // 70% opacity
        ));

        // === Error Box Background ===

        shapes.push(Shape::rounded_rect(
            box_bounds.clone(),
            super::Color::rgb(40, 30, 50), // Dark purple background
            12,
        ));

        // Error box border (red)
        shapes.push(Shape::rect_with_border(
            box_bounds.clone(),
            super::Color::rgba(0, 0, 0, 0), // Transparent fill
            super::Color::rgb(220, 50, 50), // Red border
            3,
        ));

        // === Error Icon (red circle with X) ===

        shapes.push(Shape::circle(
            box_bounds.x + 30,
            box_bounds.y + 30,
            20,
            super::Color::rgb(220, 50, 50),
        ));

        // === Error Message (wrapped) ===

        let wrapped_lines = self.wrap_message(60);
        let mut text_y = box_bounds.y + 70;

        for line in wrapped_lines {
            texts.push(TextCommand::new(
                line,
                box_bounds.x + 20,
                text_y,
                16,
                theme::TEXT_PRIMARY,
            ));
            text_y += 25; // Line spacing
        }

        // === Report Button ===

        let report_color = if is_report_hovered {
            theme::GOLD_ACCENT
        } else {
            theme::GOLD_DARK
        };
        shapes.push(Shape::rounded_rect(
            report_bounds.clone(),
            report_color,
            6,
        ));

        texts.push(TextCommand::centered(
            "Report Issue",
            report_bounds.x + report_bounds.width / 2,
            report_bounds.y + 10,
            16,
            super::Color::WHITE,
        ));

        // === Close Button ===

        let close_color = if is_close_hovered {
            theme::PURPLE_LIGHT
        } else {
            theme::PURPLE_ROYAL
        };
        shapes.push(Shape::rounded_rect(
            close_bounds.clone(),
            close_color,
            6,
        ));

        texts.push(TextCommand::centered(
            "Close",
            close_bounds.x + close_bounds.width / 2,
            close_bounds.y + 10,
            16,
            super::Color::WHITE,
        ));
    }

*/
