//! Noise texture for glassmorphic effect
//! Renders random white dots to simulate frosted glass grain

use iced::widget::canvas::{self, Cache, Canvas, Frame, Path};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Noise texture canvas renderer
///
/// Architecture:
/// - 1000 random white dots at 2% opacity
/// - Seeded RNG for consistency (seed: 42)
/// - Covers full bounds to simulate frosted glass grain
/// - Stateless rendering (no cache needed, <1ms per frame)
#[derive(Debug, Clone, Copy)]
pub struct NoiseTexture {
    seed: u64,
}

impl NoiseTexture {
    /// Create new noise texture with given seed
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Render noise texture to canvas
    pub fn view(self) -> Element<'static, crate::gui::messages::Message> {
        Canvas::new(self).width(Length::Fill).height(Length::Fill).into()
    }
}

impl Default for NoiseTexture {
    fn default() -> Self {
        Self::new(42)
    }
}

impl<Message> canvas::Program<Message> for NoiseTexture {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        // Generate deterministic noise pattern (no cache, <1ms)
        let mut frame = Frame::new(renderer, bounds.size());
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed);

        // Draw 1000 random white dots (2% opacity)
        for _ in 0..1000 {
            let x = rng.gen_range(0.0..bounds.width);
            let y = rng.gen_range(0.0..bounds.height);

            // 1px white dot at 2% opacity
            let dot = Path::rectangle(Point::new(x, y), Size::new(1.0, 1.0));
            frame.fill(&dot, Color::from_rgba(1.0, 1.0, 1.0, 0.02));
        }

        vec![frame.into_geometry()]
    }
}
