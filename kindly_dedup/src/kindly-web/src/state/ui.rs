/// UI state capsule for managing interactive elements
#[derive(Clone, Debug, PartialEq)]
pub struct UiState {
    /// Active modal (if any)
    pub active_modal: Option<String>,

    /// Is loading
    pub is_loading: bool,

    /// Toast message (if any)
    pub toast: Option<Toast>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub message: String,
    pub variant: ToastVariant,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastVariant {
    Info,
    Success,
    Warning,
    Error,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_modal: None,
            is_loading: false,
            toast: None,
        }
    }
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_modal(&mut self, modal_id: String) {
        self.active_modal = Some(modal_id);
    }

    pub fn close_modal(&mut self) {
        self.active_modal = None;
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    pub fn show_toast(&mut self, message: String, variant: ToastVariant, duration_ms: u32) {
        self.toast = Some(Toast {
            message,
            variant,
            duration_ms,
        });
    }

    pub fn clear_toast(&mut self) {
        self.toast = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_state_default() {
        let state = UiState::default();
        assert_eq!(state.active_modal, None);
        assert!(!state.is_loading);
        assert_eq!(state.toast, None);
    }

    #[test]
    fn test_modal_management() {
        let mut state = UiState::default();
        state.open_modal("test-modal".to_string());
        assert_eq!(state.active_modal, Some("test-modal".to_string()));
        state.close_modal();
        assert_eq!(state.active_modal, None);
    }

    #[test]
    fn test_toast_management() {
        let mut state = UiState::default();
        state.show_toast("Test".to_string(), ToastVariant::Success, 3000);
        assert!(state.toast.is_some());
        if let Some(toast) = &state.toast {
            assert_eq!(toast.message, "Test");
            assert_eq!(toast.variant, ToastVariant::Success);
        }
        state.clear_toast();
        assert_eq!(state.toast, None);
    }
}
