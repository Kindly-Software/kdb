/// Scroll state capsule for tracking scroll position
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollState {
    /// Current scroll Y position
    pub scroll_y: f64,

    /// Is scrolled past threshold (for navbar styling)
    pub is_scrolled: bool,

    /// Scroll direction
    pub direction: ScrollDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    None,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            scroll_y: 0.0,
            is_scrolled: false,
            direction: ScrollDirection::None,
        }
    }
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, new_scroll_y: f64, threshold: f64) {
        // Update direction
        self.direction = if new_scroll_y > self.scroll_y {
            ScrollDirection::Down
        } else if new_scroll_y < self.scroll_y {
            ScrollDirection::Up
        } else {
            ScrollDirection::None
        };

        // Update scroll position
        self.scroll_y = new_scroll_y;

        // Update scrolled flag
        self.is_scrolled = new_scroll_y > threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_default() {
        let state = ScrollState::default();
        assert_eq!(state.scroll_y, 0.0);
        assert!(!state.is_scrolled);
        assert_eq!(state.direction, ScrollDirection::None);
    }

    #[test]
    fn test_scroll_state_update() {
        let mut state = ScrollState::default();

        // Scroll down past threshold
        state.update(150.0, 100.0);
        assert_eq!(state.scroll_y, 150.0);
        assert!(state.is_scrolled);
        assert_eq!(state.direction, ScrollDirection::Down);

        // Scroll up
        state.update(50.0, 100.0);
        assert_eq!(state.scroll_y, 50.0);
        assert!(!state.is_scrolled);
        assert_eq!(state.direction, ScrollDirection::Up);
    }
}
