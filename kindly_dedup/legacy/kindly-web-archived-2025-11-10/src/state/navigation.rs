/// Navigation state capsule
#[derive(Clone, Debug, PartialEq)]
pub struct NavigationState {
    /// Is mobile menu open
    pub is_mobile_menu_open: bool,

    /// Current section (for scrollspy)
    pub current_section: Option<String>,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            is_mobile_menu_open: false,
            current_section: None,
        }
    }
}

impl NavigationState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_mobile_menu(&mut self) {
        self.is_mobile_menu_open = !self.is_mobile_menu_open;
    }

    pub fn close_mobile_menu(&mut self) {
        self.is_mobile_menu_open = false;
    }

    pub fn set_current_section(&mut self, section: String) {
        self.current_section = Some(section);
    }

    pub fn clear_current_section(&mut self) {
        self.current_section = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_state_default() {
        let state = NavigationState::default();
        assert!(!state.is_mobile_menu_open);
        assert_eq!(state.current_section, None);
    }

    #[test]
    fn test_toggle_mobile_menu() {
        let mut state = NavigationState::default();
        state.toggle_mobile_menu();
        assert!(state.is_mobile_menu_open);
        state.toggle_mobile_menu();
        assert!(!state.is_mobile_menu_open);
    }

    #[test]
    fn test_set_current_section() {
        let mut state = NavigationState::default();
        state.set_current_section("hero".to_string());
        assert_eq!(state.current_section, Some("hero".to_string()));
        state.clear_current_section();
        assert_eq!(state.current_section, None);
    }
}
