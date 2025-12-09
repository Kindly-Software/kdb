//! Responsive layout utilities
//!
//! Plain Leptos hooks (no capsules, trade secret protected)

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Window;

/// Responsive breakpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    /// Extra small: 0-639px (mobile portrait)
    Xs,
    /// Small: 640-767px (mobile landscape)
    Sm,
    /// Medium: 768-1023px (tablet)
    Md,
    /// Large: 1024-1279px (desktop)
    Lg,
    /// Extra large: 1280px+ (wide desktop)
    Xl,
}

impl Breakpoint {
    /// Get breakpoint from width
    pub fn from_width(width: u32) -> Self {
        match width {
            0..=639 => Self::Xs,
            640..=767 => Self::Sm,
            768..=1023 => Self::Md,
            1024..=1279 => Self::Lg,
            _ => Self::Xl,
        }
    }

    /// Check if mobile
    pub fn is_mobile(&self) -> bool {
        matches!(self, Self::Xs | Self::Sm)
    }
}

/// Get window object (WASM only)
fn window() -> Window {
    web_sys::window().expect("no window")
}

/// Get current viewport width
pub fn viewport_width() -> u32 {
    window()
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1024.0) as u32
}

/// Get current viewport height
pub fn viewport_height() -> u32 {
    window()
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(768.0) as u32
}

/// Use responsive breakpoint (reactive)
pub fn use_breakpoint() -> Signal<Breakpoint> {
    let (breakpoint, set_breakpoint) = signal(Breakpoint::from_width(viewport_width()));

    // Update on window resize
    Effect::new(move |_| {
        let window = window();
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
            set_breakpoint.set(Breakpoint::from_width(viewport_width()));
        }) as Box<dyn FnMut(_)>);

        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .expect("failed to add resize listener");

        closure.forget(); // Keep listener active
    });

    breakpoint.into()
}

/// Use scroll position (reactive)
pub fn use_scroll_y() -> Signal<u32> {
    let (scroll_y, set_scroll_y) = signal(0u32);

    Effect::new(move |_| {
        let window = window();
        let window_clone = window.clone();
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
            let y = window_clone.scroll_y().unwrap_or(0.0) as u32;
            set_scroll_y.set(y);
        }) as Box<dyn FnMut(_)>);

        window
            .add_event_listener_with_callback("scroll", closure.as_ref().unchecked_ref())
            .expect("failed to add scroll listener");

        closure.forget(); // Keep listener active
    });

    scroll_y.into()
}

/// Get navbar blur level based on scroll position
pub fn navbar_blur_from_scroll(scroll_y: u32) -> u8 {
    match scroll_y {
        0..=50 => 0,       // No blur
        51..=200 => 1,     // Light blur
        201..=500 => 2,    // Medium blur
        _ => 3,            // Full blur
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_from_width() {
        assert_eq!(Breakpoint::from_width(320), Breakpoint::Xs);
        assert_eq!(Breakpoint::from_width(640), Breakpoint::Sm);
        assert_eq!(Breakpoint::from_width(768), Breakpoint::Md);
        assert_eq!(Breakpoint::from_width(1024), Breakpoint::Lg);
        assert_eq!(Breakpoint::from_width(1920), Breakpoint::Xl);
    }

    #[test]
    fn test_is_mobile() {
        assert!(Breakpoint::Xs.is_mobile());
        assert!(Breakpoint::Sm.is_mobile());
        assert!(!Breakpoint::Md.is_mobile());
        assert!(!Breakpoint::Lg.is_mobile());
        assert!(!Breakpoint::Xl.is_mobile());
    }

    #[test]
    fn test_navbar_blur() {
        assert_eq!(navbar_blur_from_scroll(0), 0);
        assert_eq!(navbar_blur_from_scroll(100), 1);
        assert_eq!(navbar_blur_from_scroll(300), 2);
        assert_eq!(navbar_blur_from_scroll(600), 3);
    }
}
