//! ThemeCapsule - Tier 1 Atomic (64B)
//!
//! Purpose: Theme color management with dark mode support
//! Memory Layout:
//!   [0]     color_index: AtomicU8 (primary color index 0-15)
//!   [1]     accent_index: AtomicU8 (accent color index 0-15)
//!   [2]     dark_mode: AtomicU8 (0=light, 1=dark)
//!   [3-63]  _padding: [u8; 61] (cache alignment)

use super::error::{CapsuleError, CapsuleResult};
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU8, Ordering};

/// Tier 1 Atomic: Theme capsule (64B cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct ThemeCapsule {
    /// Primary color index (0-15)
    color_index: AtomicU8,
    /// Accent color index (0-15)
    accent_index: AtomicU8,
    /// Dark mode (0=light, 1=dark)
    dark_mode: AtomicU8,
    /// Padding to 64 bytes (cache line alignment)
    _padding: [u8; 61],
}

impl ThemeCapsule {
    /// Create new theme capsule with default values
    ///
    /// # Returns
    /// ThemeCapsule with color_index=0, accent_index=0, dark_mode=false
    pub const fn new() -> Self {
        Self {
            color_index: AtomicU8::new(0),
            accent_index: AtomicU8::new(0),
            dark_mode: AtomicU8::new(0),
            _padding: [0u8; 61],
        }
    }

    /// Create theme with initial colors
    ///
    /// # Arguments
    /// * `color_index` - Primary color index (0-15)
    /// * `accent_index` - Accent color index (0-15)
    /// * `dark_mode` - Dark mode enabled
    ///
    /// # Returns
    /// ThemeCapsule or error if indices out of range
    pub fn with_colors(color_index: u8, accent_index: u8, dark_mode: bool) -> CapsuleResult<Self> {
        if color_index > 15 {
            return Err(CapsuleError::InvalidValue {
                message: format!("color_index {} exceeds 4-bit limit (0-15)", color_index),
            });
        }
        if accent_index > 15 {
            return Err(CapsuleError::InvalidValue {
                message: format!("accent_index {} exceeds 4-bit limit (0-15)", accent_index),
            });
        }

        Ok(Self {
            color_index: AtomicU8::new(color_index),
            accent_index: AtomicU8::new(accent_index),
            dark_mode: AtomicU8::new(dark_mode as u8),
            _padding: [0u8; 61],
        })
    }

    /// Set primary color index
    ///
    /// #ASSUME: Atomic store prevents race conditions
    /// #VERIFY: Index is 0-15 (4 bits)
    ///
    /// # Arguments
    /// * `index` - Color index (0-15)
    ///
    /// # Returns
    /// Ok or error if index > 15
    pub fn set_color_index(&self, index: u8) -> CapsuleResult<()> {
        if index > 15 {
            return Err(CapsuleError::InvalidValue {
                message: format!("color_index {} exceeds 4-bit limit (0-15)", index),
            });
        }

        // #ASSUME: Relaxed ordering safe (color_index is independent UI state)
        self.color_index.store(index, Ordering::Relaxed);
        Ok(())
    }

    /// Get primary color index
    ///
    /// #ASSUME: Relaxed load safe (color_index is independent)
    pub fn get_color_index(&self) -> u8 {
        self.color_index.load(Ordering::Relaxed)
    }

    /// Set accent color index
    ///
    /// #ASSUME: Atomic store prevents race conditions
    /// #VERIFY: Index is 0-15 (4 bits)
    ///
    /// # Arguments
    /// * `index` - Accent color index (0-15)
    ///
    /// # Returns
    /// Ok or error if index > 15
    pub fn set_accent_index(&self, index: u8) -> CapsuleResult<()> {
        if index > 15 {
            return Err(CapsuleError::InvalidValue {
                message: format!("accent_index {} exceeds 4-bit limit (0-15)", index),
            });
        }

        // #ASSUME: Relaxed ordering safe (accent_index is independent UI state)
        self.accent_index.store(index, Ordering::Relaxed);
        Ok(())
    }

    /// Get accent color index
    ///
    /// #ASSUME: Relaxed load safe (accent_index is independent)
    pub fn get_accent_index(&self) -> u8 {
        self.accent_index.load(Ordering::Relaxed)
    }

    /// Set both color indices atomically
    ///
    /// #ASSUME: Two separate atomic stores (order doesn't matter for UI)
    ///
    /// # Arguments
    /// * `color_index` - Primary color index (0-15)
    /// * `accent_index` - Accent color index (0-15)
    ///
    /// # Returns
    /// Ok or error if either index > 15
    pub fn set_colors(&self, color_index: u8, accent_index: u8) -> CapsuleResult<()> {
        self.set_color_index(color_index)?;
        self.set_accent_index(accent_index)?;
        Ok(())
    }

    /// Set dark mode
    ///
    /// #ASSUME: Atomic store prevents race conditions
    ///
    /// # Arguments
    /// * `enabled` - Dark mode enabled
    pub fn set_dark_mode(&self, enabled: bool) {
        // #ASSUME: Relaxed ordering safe (dark_mode is independent UI state)
        self.dark_mode.store(enabled as u8, Ordering::Relaxed);
    }

    /// Get dark mode state
    ///
    /// #ASSUME: Relaxed load safe (dark_mode is independent)
    pub fn get_dark_mode(&self) -> bool {
        self.dark_mode.load(Ordering::Relaxed) != 0
    }

    /// Toggle dark mode
    ///
    /// #ASSUME: CAS loop for atomic toggle
    ///
    /// # Returns
    /// New dark mode state after toggle
    pub fn toggle_dark_mode(&self) -> bool {
        let mut current = self.dark_mode.load(Ordering::Relaxed);
        loop {
            let new_value = if current == 0 { 1 } else { 0 };

            match self
                .dark_mode
                .compare_exchange_weak(current, new_value, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return new_value != 0,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get snapshot of all theme values
    ///
    /// #ASSUME: Three separate loads (order doesn't matter for UI snapshot)
    ///
    /// # Returns
    /// (color_index, accent_index, dark_mode)
    pub fn snapshot(&self) -> (u8, u8, bool) {
        let color = self.color_index.load(Ordering::Relaxed);
        let accent = self.accent_index.load(Ordering::Relaxed);
        let dark = self.dark_mode.load(Ordering::Relaxed) != 0;
        (color, accent, dark)
    }
}

impl Default for ThemeCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_alignment() {
        assert_eq!(std::mem::align_of::<ThemeCapsule>(), 64);
        assert_eq!(std::mem::size_of::<ThemeCapsule>(), 64);
    }

    #[test]
    fn test_color_index() {
        let theme = ThemeCapsule::new();
        assert_eq!(theme.get_color_index(), 0);

        theme.set_color_index(5).unwrap();
        assert_eq!(theme.get_color_index(), 5);
    }

    #[test]
    fn test_accent_index() {
        let theme = ThemeCapsule::new();
        assert_eq!(theme.get_accent_index(), 0);

        theme.set_accent_index(7).unwrap();
        assert_eq!(theme.get_accent_index(), 7);
    }

    #[test]
    fn test_set_colors() {
        let theme = ThemeCapsule::new();

        theme.set_colors(3, 9).unwrap();
        assert_eq!(theme.get_color_index(), 3);
        assert_eq!(theme.get_accent_index(), 9);
    }

    #[test]
    fn test_dark_mode() {
        let theme = ThemeCapsule::new();
        assert_eq!(theme.get_dark_mode(), false);

        theme.set_dark_mode(true);
        assert_eq!(theme.get_dark_mode(), true);

        assert_eq!(theme.toggle_dark_mode(), false);
        assert_eq!(theme.get_dark_mode(), false);
    }

    #[test]
    fn test_with_colors() {
        let theme = ThemeCapsule::with_colors(5, 10, true).unwrap();
        assert_eq!(theme.get_color_index(), 5);
        assert_eq!(theme.get_accent_index(), 10);
        assert_eq!(theme.get_dark_mode(), true);
    }

    #[test]
    fn test_snapshot() {
        let theme = ThemeCapsule::with_colors(2, 8, true).unwrap();
        let (color, accent, dark) = theme.snapshot();
        assert_eq!(color, 2);
        assert_eq!(accent, 8);
        assert_eq!(dark, true);
    }

    #[test]
    fn test_invalid_color_index() {
        let theme = ThemeCapsule::new();
        assert!(theme.set_color_index(16).is_err());
    }

    #[test]
    fn test_invalid_accent_index() {
        let theme = ThemeCapsule::new();
        assert!(theme.set_accent_index(20).is_err());
    }

    #[test]
    fn test_invalid_with_colors() {
        assert!(ThemeCapsule::with_colors(16, 0, false).is_err());
        assert!(ThemeCapsule::with_colors(0, 16, false).is_err());
    }
}
