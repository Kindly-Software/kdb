//! Shimmer progress bar with animated glow effect
//! Provides visual feedback that processing is active (not frozen)

use crate::gui::theme::colors::*;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme};

/// Progress bar with animated shimmer/glow effect
///
/// Architecture:
/// - 3 layers: background rail (dark purple), filled bar (purple→gold gradient), shimmer highlight (moving gold glow)
/// - Shimmer is 10% of bar width, moves left→right in 2-second loop
/// - Uses Canvas for custom rendering (iced 0.10 limitation - no built-in gradient animation)
pub struct ShimmerProgress {
    progress: f32,
    shimmer_offset: f32,
}

impl ShimmerProgress {
    /// Create new shimmer progress bar
    ///
    /// # Arguments
    /// * `progress` - Progress fraction (0.0 to 1.0)
    /// * `shimmer_offset` - Shimmer animation offset (0.0 to 1.0, loops)
    pub fn new(progress: f32, shimmer_offset: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            shimmer_offset: shimmer_offset % 1.0,
        }
    }

    /// Render the shimmer progress bar
    pub fn view(&self) -> Element<'static, crate::gui::messages::Message> {
        Canvas::new(ShimmerRenderer {
            progress: self.progress,
            shimmer_offset: self.shimmer_offset,
        })
        .width(Length::Fill)
        .height(Length::Fixed(24.0))
        .into()
    }
}

/// Canvas renderer for shimmer effect
struct ShimmerRenderer {
    progress: f32,
    shimmer_offset: f32,
}

impl<Message> canvas::Program<Message> for ShimmerRenderer {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Layer 1: Background rail (dark purple at 30% opacity)
        let rail_path = Path::rectangle(Point::ORIGIN, Size::new(bounds.width, 24.0));
        frame.fill(&rail_path, with_alpha(PURPLE_DEEP, 0.3));

        // Layer 2: Filled bar (purple→gold gradient based on progress)
        if self.progress > 0.0 {
            let filled_width = bounds.width * self.progress;
            let filled_path = Path::rectangle(Point::ORIGIN, Size::new(filled_width, 24.0));

            // Gradient color based on progress
            let bar_color = lerp_color(PURPLE_ROYAL, GOLD_BRIGHT, self.progress);
            frame.fill(&filled_path, bar_color);
        }

        // Layer 3: Shimmer highlight (moving gold glow, 10% bar width)
        if self.progress > 0.1 {
            // Only show shimmer if progress > 10%
            let filled_width = bounds.width * self.progress;
            let shimmer_width = bounds.width * 0.1; // 10% of total bar width

            // Calculate shimmer position within filled portion
            // shimmer_offset: 0.0 → left edge, 1.0 → right edge of filled bar
            let shimmer_x = (filled_width - shimmer_width) * self.shimmer_offset;

            // Only draw shimmer if it's within the filled portion
            if shimmer_x + shimmer_width <= filled_width {
                let shimmer_path = Path::rectangle(Point::new(shimmer_x, 0.0), Size::new(shimmer_width, 24.0));

                // Gold glow at 60% opacity
                frame.fill(&shimmer_path, with_alpha(GOLD_LIGHT, 0.6));
            }
        }

        // Optional: Add subtle border for depth
        let border_path = Path::rectangle(Point::ORIGIN, Size::new(bounds.width, 24.0));
        frame.stroke(
            &border_path,
            Stroke::default()
                .with_width(1.0)
                .with_color(with_alpha(PURPLE_ROYAL, 0.5)),
        );

        vec![frame.into_geometry()]
    }
}
