/// Global application state capsule
#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    /// Current route
    pub current_route: String,

    /// Is mobile view
    pub is_mobile: bool,

    /// Is dark mode enabled
    pub is_dark_mode: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_route: "/".to_string(),
            is_mobile: false,
            is_dark_mode: false,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_route(&mut self, route: String) {
        self.current_route = route;
    }

    pub fn toggle_dark_mode(&mut self) {
        self.is_dark_mode = !self.is_dark_mode;
    }

    pub fn set_mobile(&mut self, is_mobile: bool) {
        self.is_mobile = is_mobile;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.current_route, "/");
        assert!(!state.is_mobile);
        assert!(!state.is_dark_mode);
    }

    #[test]
    fn test_toggle_dark_mode() {
        let mut state = AppState::default();
        assert!(!state.is_dark_mode);
        state.toggle_dark_mode();
        assert!(state.is_dark_mode);
        state.toggle_dark_mode();
        assert!(!state.is_dark_mode);
    }
}
