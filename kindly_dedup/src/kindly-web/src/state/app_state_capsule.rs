//! AppStateCapsule - Tier 1 Atomic (64B)
//!
//! Purpose: Global application state with theme, dark mode, user tracking
//! Memory Layout:
//!   [0-7]   packed: AtomicU64 (theme:2b + dark_mode:1b + user_id:30b + generation:31b)
//!   [8-63]  _padding: [u8; 56] (cache alignment)

use super::error::{CapsuleError, CapsuleResult};
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tier 1 Atomic: Global app state capsule (64B cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct AppStateCapsule {
    /// Packed: theme(2b) + dark_mode(1b) + user_id(30b) + generation(31b)
    packed: AtomicU64,
    /// Padding to 64 bytes (cache line alignment)
    _padding: [u8; 56],
}

// Bit layout constants
const THEME_MASK: u64 = 0x3; // 2 bits
const DARK_MODE_MASK: u64 = 0x4; // 1 bit (bit 2)
const USER_ID_MASK: u64 = 0x3FFF_FFFC; // 30 bits (bits 3-32)
const GENERATION_MASK: u64 = 0xFFFF_FFFF_8000_0000; // 31 bits (bits 33-63)

const THEME_SHIFT: u32 = 0;
const DARK_MODE_SHIFT: u32 = 2;
const USER_ID_SHIFT: u32 = 3;
const GENERATION_SHIFT: u32 = 33;

impl AppStateCapsule {
    /// Create new app state capsule with default values
    ///
    /// # Returns
    /// AppStateCapsule with theme=0, dark_mode=false, user_id=0, generation=0
    pub const fn new() -> Self {
        Self {
            packed: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Create app state with initial user ID
    ///
    /// # Arguments
    /// * `user_id` - User ID (0-1,073,741,823, 30 bits)
    ///
    /// # Returns
    /// Initialized AppStateCapsule or error if user_id too large
    pub fn with_user_id(user_id: u32) -> CapsuleResult<Self> {
        if user_id >= (1 << 30) {
            return Err(CapsuleError::InvalidValue {
                message: format!("user_id {} exceeds 30-bit limit", user_id),
            });
        }

        let packed_value = (user_id as u64) << USER_ID_SHIFT;
        Ok(Self {
            packed: AtomicU64::new(packed_value),
            _padding: [0u8; 56],
        })
    }

    /// Set theme (0-3)
    ///
    /// #ASSUME: Atomic CAS prevents race conditions on theme updates
    /// #VERIFY: Only 2 bits used (0-3 range)
    ///
    /// # Arguments
    /// * `theme` - Theme index (0-3)
    ///
    /// # Returns
    /// Previous theme value or error if theme > 3
    pub fn set_theme(&self, theme: u8) -> CapsuleResult<u8> {
        if theme > 3 {
            return Err(CapsuleError::InvalidValue {
                message: format!("theme {} exceeds 2-bit limit (0-3)", theme),
            });
        }

        // #ASSUME: Relaxed ordering safe for theme (no cross-field dependencies)
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let old_theme = (current & THEME_MASK) as u8;
            let new_value = (current & !THEME_MASK) | (theme as u64);

            // #ASSUME: CAS with Relaxed prevents concurrent theme updates
            match self
                .packed
                .compare_exchange_weak(current, new_value, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Ok(old_theme),
                Err(actual) => current = actual, // Retry with updated value
            }
        }
    }

    /// Get current theme (0-3)
    ///
    /// #ASSUME: Relaxed load safe (theme is independent value)
    pub fn get_theme(&self) -> u8 {
        let packed = self.packed.load(Ordering::Relaxed);
        (packed & THEME_MASK) as u8
    }

    /// Set dark mode
    ///
    /// #ASSUME: Atomic CAS prevents race conditions
    /// #VERIFY: Single bit (boolean)
    ///
    /// # Arguments
    /// * `enabled` - Dark mode enabled
    ///
    /// # Returns
    /// Previous dark mode state
    pub fn set_dark_mode(&self, enabled: bool) -> bool {
        // #ASSUME: Relaxed ordering safe for dark_mode (UI state)
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let old_dark_mode = (current & DARK_MODE_MASK) != 0;
            let new_value = if enabled {
                current | DARK_MODE_MASK
            } else {
                current & !DARK_MODE_MASK
            };

            // #ASSUME: CAS with Relaxed prevents concurrent dark_mode updates
            match self
                .packed
                .compare_exchange_weak(current, new_value, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return old_dark_mode,
                Err(actual) => current = actual, // Retry
            }
        }
    }

    /// Toggle dark mode
    ///
    /// # Returns
    /// New dark mode state after toggle
    pub fn toggle_dark_mode(&self) -> bool {
        let old = self.set_dark_mode(!self.get_dark_mode());
        !old
    }

    /// Get dark mode state
    ///
    /// #ASSUME: Relaxed load safe (dark_mode is independent)
    pub fn get_dark_mode(&self) -> bool {
        let packed = self.packed.load(Ordering::Relaxed);
        (packed & DARK_MODE_MASK) != 0
    }

    /// Get user ID (30 bits)
    ///
    /// #ASSUME: Relaxed load safe (user_id is read-only after init)
    pub fn get_user_id(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        ((packed & USER_ID_MASK) >> USER_ID_SHIFT) as u32
    }

    /// Get generation counter (31 bits)
    ///
    /// #ASSUME: Relaxed load safe (generation for TOCTOU prevention only)
    pub fn generation(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32
    }

    /// Increment generation counter (for TOCTOU prevention)
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (generation is monotonic counter)
    /// #VERIFY: 31-bit wraparound acceptable (TOCTOU detection)
    fn _increment_generation(&self) {
        // Extract current generation, increment, pack back
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let gen = ((current & GENERATION_MASK) >> GENERATION_SHIFT) as u32;
            let new_gen = gen.wrapping_add(1) & 0x7FFF_FFFF; // 31-bit mask
            let new_value = (current & !GENERATION_MASK) | ((new_gen as u64) << GENERATION_SHIFT);

            match self
                .packed
                .compare_exchange_weak(current, new_value, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Load all fields atomically
    ///
    /// #ASSUME: Single 64-bit load is atomic (x86-64 guarantee)
    ///
    /// # Returns
    /// (theme, dark_mode, user_id, generation)
    pub fn snapshot(&self) -> (u8, bool, u32, u32) {
        let packed = self.packed.load(Ordering::Relaxed);
        let theme = (packed & THEME_MASK) as u8;
        let dark_mode = (packed & DARK_MODE_MASK) != 0;
        let user_id = ((packed & USER_ID_MASK) >> USER_ID_SHIFT) as u32;
        let generation = ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32;
        (theme, dark_mode, user_id, generation)
    }
}

impl Default for AppStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_alignment() {
        assert_eq!(std::mem::align_of::<AppStateCapsule>(), 64);
        assert_eq!(std::mem::size_of::<AppStateCapsule>(), 64);
    }

    #[test]
    fn test_theme_operations() {
        let state = AppStateCapsule::new();
        assert_eq!(state.get_theme(), 0);

        assert_eq!(state.set_theme(2).unwrap(), 0);
        assert_eq!(state.get_theme(), 2);

        assert_eq!(state.set_theme(3).unwrap(), 2);
        assert_eq!(state.get_theme(), 3);
    }

    #[test]
    fn test_dark_mode() {
        let state = AppStateCapsule::new();
        assert_eq!(state.get_dark_mode(), false);

        assert_eq!(state.set_dark_mode(true), false);
        assert_eq!(state.get_dark_mode(), true);

        assert_eq!(state.toggle_dark_mode(), false);
        assert_eq!(state.get_dark_mode(), false);
    }

    #[test]
    fn test_user_id() {
        let state = AppStateCapsule::with_user_id(123456).unwrap();
        assert_eq!(state.get_user_id(), 123456);
    }

    #[test]
    fn test_snapshot() {
        let state = AppStateCapsule::with_user_id(999).unwrap();
        state.set_theme(2).unwrap();
        state.set_dark_mode(true);

        let (theme, dark_mode, user_id, _gen) = state.snapshot();
        assert_eq!(theme, 2);
        assert_eq!(dark_mode, true);
        assert_eq!(user_id, 999);
    }

    #[test]
    fn test_invalid_theme() {
        let state = AppStateCapsule::new();
        assert!(state.set_theme(4).is_err());
    }

    #[test]
    fn test_invalid_user_id() {
        let result = AppStateCapsule::with_user_id(1 << 30);
        assert!(result.is_err());
    }
}
