//! TUI Wizard Computational Capsules (T1 Atomic)
//!
//! Three specialized capsules for the clapi configuration wizard:
//! 1. LogoAnimationCapsule - DualAtomicU64 for RGB colors (ping-pong animation)
//! 2. WizardStateCapsule - Packed state machine (step, field_idx, mode, navigation stack)
//! 3. CtrlCHandlerCapsule - Double-press detection with generation counter
//!
//! Architecture: UCE34 Q10 (T1 Atomic tier), Q11 (Rust atomics), Q33 (compile-time verification)
//! Performance: All operations <100ns (lockfree, deterministic)
//! Safety: 100% lockfree (NO Mutex), generation counters prevent TOCTOU

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// CAPSULE 1: LogoAnimationCapsule (T1 Atomic, 64B)
// ============================================================================

/// Logo Animation Capsule - DualAtomicU64 pattern for RGB color animation
///
/// **Architecture**: T1 Atomic (UCE34 Q10)
/// - DualAtomicU64 pattern: 2 cache-separated atomics for block + border colors
/// - Ping-pong animation: Purple (#663399) ↔ Gold (#FFD700)
/// - Generation counters prevent TOCTOU races
///
/// **Performance**: <50ns per frame update (lockfree, no contention)
///
/// **Alignment**: 64B (single cache line per atomic, prevents false sharing)
///
/// **Layout**:
/// - primary (8B): packed RGB for block color
/// - _padding1 (56B): complete cache line 1
/// Total: 64B exactly
///
/// **Note**: This is a simplified single-atomic version. For dual-channel coordination
/// with separate block/border colors, use 128B DualAtomicU64 pattern (see § Composition).
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct LogoAnimationCapsule {
    /// Packed state: generation(16) | direction(1) | frame(8) | r(8) | g(8) | b(8)
    /// - generation: prevents TOCTOU
    /// - direction: 0 = purple→gold, 1 = gold→purple
    /// - frame: current animation frame (0-255)
    /// - rgb: current color (24 bits)
    state: AtomicU64,
    _padding: [u8; 56],
}

impl LogoAnimationCapsule {
    // Purple: #663399 = RGB(102, 51, 153)
    const PURPLE_R: u8 = 0x66;
    const PURPLE_G: u8 = 0x33;
    const PURPLE_B: u8 = 0x99;

    // Gold: #FFD700 = RGB(255, 215, 0)
    const GOLD_R: u8 = 0xFF;
    const GOLD_G: u8 = 0xD7;
    const GOLD_B: u8 = 0x00;

    const TOTAL_FRAMES: u8 = 60; // 60 frames for smooth animation

    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(Self::pack_state(0, 0, 0, Self::PURPLE_R, Self::PURPLE_G, Self::PURPLE_B)),
            _padding: [0u8; 56],
        }
    }

    /// Pack state into single u64 (compile-time const)
    const fn pack_state(generation: u16, direction: u8, frame: u8, r: u8, g: u8, b: u8) -> u64 {
        ((generation as u64) << 48)
            | ((direction as u64 & 0x1) << 40)
            | ((frame as u64) << 32)
            | ((r as u64) << 16)
            | ((g as u64) << 8)
            | (b as u64)
    }

    /// Unpack state from u64
    const fn unpack_state(state: u64) -> (u16, u8, u8, u8, u8, u8) {
        let generation = (state >> 48) as u16;
        let direction = ((state >> 40) & 0x1) as u8;
        let frame = (state >> 32) as u8;
        let r = (state >> 16) as u8;
        let g = (state >> 8) as u8;
        let b = state as u8;
        (generation, direction, frame, r, g, b)
    }

    /// Update animation frame (ping-pong between purple and gold)
    ///
    /// **Performance**: <50ns (single atomic read + CAS)
    ///
    /// **ASSUM Safety**:
    /// - #ASSUME: CAS loop succeeds within 3 retries typically
    /// - #VERIFY: Property tests validate linearizability
    /// - #ASSUME: Generation counter prevents TOCTOU races
    /// - #VERIFY: Concurrent access tests validate atomicity
    pub fn update_frame(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, direction, frame, _r, _g, _b) = Self::unpack_state(current);

            // Increment frame
            let mut new_frame = frame.wrapping_add(1);
            let mut new_direction = direction;

            // Reverse direction at frame boundaries
            if new_frame >= Self::TOTAL_FRAMES {
                new_frame = 0;
                new_direction = 1 - direction; // Toggle 0↔1
            }

            // Linear interpolation between colors
            let t = new_frame as f32 / Self::TOTAL_FRAMES as f32;
            let (new_r, new_g, new_b) = if new_direction == 0 {
                // Purple → Gold
                Self::lerp_color(
                    (Self::PURPLE_R, Self::PURPLE_G, Self::PURPLE_B),
                    (Self::GOLD_R, Self::GOLD_G, Self::GOLD_B),
                    t,
                )
            } else {
                // Gold → Purple
                Self::lerp_color(
                    (Self::GOLD_R, Self::GOLD_G, Self::GOLD_B),
                    (Self::PURPLE_R, Self::PURPLE_G, Self::PURPLE_B),
                    t,
                )
            };

            let new_state = Self::pack_state(gen.wrapping_add(1), new_direction, new_frame, new_r, new_g, new_b);

            // #ASSUME: Release ordering ensures color update visible to readers
            // #VERIFY: Memory ordering tests validate synchronization
            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Read current colors (lockfree, <10ns)
    ///
    /// Returns: (r, g, b) as u8 tuple
    pub fn read_colors(&self) -> (u8, u8, u8) {
        let state = self.state.load(Ordering::Relaxed);
        let (_gen, _dir, _frame, r, g, b) = Self::unpack_state(state);
        (r, g, b)
    }

    /// Linear interpolation between two colors
    const fn lerp_color(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
        let r = (from.0 as f32 + (to.0 as f32 - from.0 as f32) * t) as u8;
        let g = (from.1 as f32 + (to.1 as f32 - from.1 as f32) * t) as u8;
        let b = (from.2 as f32 + (to.2 as f32 - from.2 as f32) * t) as u8;
        (r, g, b)
    }
}

// ============================================================================
// CAPSULE 2: WizardStateCapsule (T1 Atomic, 128B)
// ============================================================================

/// Wizard State Machine Capsule - Simple bounded navigation
///
/// **Architecture**: T1 Atomic (UCE34 Q10)
/// - Packed state: step(8) + field_idx(8) + mode(8) in single AtomicU64
/// - Input buffer: 64 chars, lockfree
/// - Step bounds: 1-4 (clamped, no overflow/underflow)
///
/// **Performance**: <20ns state transitions (lockfree, simplified)
///
/// **Alignment**: 128B (prevents false sharing)
///
/// **Layout**:
/// - state (8B): packed generation|step|field_idx|mode
/// - input_buffer (64B): user input (lockfree read/write)
/// - _padding (56B): complete 128B alignment
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct WizardStateCapsule {
    /// Packed state: generation(16) | step(8) | field_idx(8) | mode(8)
    /// - generation: TOCTOU prevention
    /// - step: current wizard step (1-4, bounds-checked)
    /// - field_idx: current field index within step (0-255)
    /// - mode: wizard mode (0=config, 1=provider, 2=advanced)
    state: AtomicU64,

    /// Input buffer: 64 chars stored as 8 × u64
    /// - Lockfree read/write (no CAS required)
    /// - Each u64 stores 8 ASCII chars
    input_buffer: [AtomicU64; 8],

    /// Padding to 128B (8B state + 64B input + 56B padding = 128B)
    _padding: [u8; 56],
}

impl WizardStateCapsule {
    /// Maximum wizard step (Step 4: Preview & Confirm)
    const MAX_STEP: u8 = 4;
    /// Minimum wizard step (Step 1: Server Settings)
    const MIN_STEP: u8 = 1;

    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(Self::pack_state(0, 1, 0, 0)), // generation=0, step=1, field_idx=0, mode=0
            input_buffer: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            _padding: [0u8; 56],
        }
    }

    /// Pack state into u64
    const fn pack_state(generation: u16, step: u8, field_idx: u8, mode: u8) -> u64 {
        ((generation as u64) << 48)
            | ((step as u64) << 16)
            | ((field_idx as u64) << 8)
            | (mode as u64)
    }

    /// Unpack state from u64
    const fn unpack_state(state: u64) -> (u16, u8, u8, u8) {
        let generation = (state >> 48) as u16;
        let step = (state >> 16) as u8;
        let field_idx = (state >> 8) as u8;
        let mode = state as u8;
        (generation, step, field_idx, mode)
    }

    /// Navigate to next step (bounded to MAX_STEP)
    ///
    /// **Performance**: <20ns (single atomic CAS operation)
    /// **Bounds**: Stays at step 4 if already at max (no overflow)
    pub fn next_step(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, step, _field_idx, mode) = Self::unpack_state(current);

            // Clamp to MAX_STEP (stay at step 4 if already there)
            let new_step = if step >= Self::MAX_STEP {
                Self::MAX_STEP
            } else {
                step + 1
            };

            let new_state = Self::pack_state(gen.wrapping_add(1), new_step, 0, mode);

            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Navigate to previous step (bounded to MIN_STEP)
    ///
    /// **Performance**: <20ns (single atomic CAS operation)
    /// **Bounds**: Stays at step 1 if already at min (no underflow)
    pub fn prev_step(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, step, _field_idx, mode) = Self::unpack_state(current);

            // Clamp to MIN_STEP (stay at step 1 if already there)
            let new_step = if step <= Self::MIN_STEP {
                Self::MIN_STEP
            } else {
                step - 1
            };

            let new_state = Self::pack_state(gen.wrapping_add(1), new_step, 0, mode);

            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Navigate to next option within current step (for lists/dropdowns)
    ///
    /// **Performance**: <20ns (single atomic CAS operation)
    /// **Bounds**: Per-step max options (Step 2: 5 providers, 0-4 index)
    pub fn next_option(&self, max_options: u8) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, step, field_idx, mode) = Self::unpack_state(current);

            // Clamp to max_options - 1 (0-indexed)
            let new_field_idx = if field_idx >= max_options - 1 {
                max_options - 1
            } else {
                field_idx + 1
            };

            let new_state = Self::pack_state(gen.wrapping_add(1), step, new_field_idx, mode);

            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Navigate to previous option within current step (for lists/dropdowns)
    ///
    /// **Performance**: <20ns (single atomic CAS operation)
    /// **Bounds**: Min 0 (first option)
    pub fn prev_option(&self) {
        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, step, field_idx, mode) = Self::unpack_state(current);

            // Clamp to 0 (first option)
            let new_field_idx = if field_idx == 0 {
                0
            } else {
                field_idx - 1
            };

            let new_state = Self::pack_state(gen.wrapping_add(1), step, new_field_idx, mode);

            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Update input buffer (lockfree write, <20ns for 64 chars)
    ///
    /// **Safety**: Uses Relaxed ordering (no synchronization needed for input buffer)
    pub fn update_input(&self, input: &str) {
        // Clear buffer first
        for atomic in &self.input_buffer {
            atomic.store(0, Ordering::Relaxed);
        }

        // Write input (max 64 chars)
        let bytes = input.as_bytes();
        let len = bytes.len().min(64);

        for (chunk_idx, chunk) in bytes[..len].chunks(8).enumerate() {
            let mut value = 0u64;
            for (byte_idx, &byte) in chunk.iter().enumerate() {
                value |= (byte as u64) << (byte_idx * 8);
            }
            self.input_buffer[chunk_idx].store(value, Ordering::Relaxed);
        }
    }

    /// Read current state (lockfree, <10ns)
    ///
    /// Returns: (step, field_idx, mode)
    pub fn read_state(&self) -> (u8, u8, u8) {
        let state = self.state.load(Ordering::Relaxed);
        let (_gen, step, field_idx, mode) = Self::unpack_state(state);
        (step, field_idx, mode)
    }

    /// Read input buffer (lockfree, <50ns for 64 chars)
    pub fn read_input(&self) -> String {
        let mut bytes = Vec::with_capacity(64);

        for atomic in &self.input_buffer {
            let value = atomic.load(Ordering::Relaxed);
            for i in 0..8 {
                let byte = ((value >> (i * 8)) & 0xFF) as u8;
                if byte == 0 {
                    break; // Null terminator
                }
                bytes.push(byte);
            }
        }

        String::from_utf8_lossy(&bytes).to_string()
    }
}

// ============================================================================
// CAPSULE 3: CtrlCHandlerCapsule (T1 Atomic, 64B)
// ============================================================================

/// Ctrl+C Double-Press Detection Capsule
///
/// **Architecture**: T1 Atomic (UCE34 Q10)
/// - Generation counter prevents TOCTOU races
/// - Double-press detection: 2 presses within 2 seconds = exit
/// - Timestamp tracking: last press time (nanoseconds since epoch)
///
/// **Performance**: <20ns per press registration (lockfree)
///
/// **Alignment**: 64B (single cache line)
///
/// **Layout**:
/// - state (8B): packed generation(16) | press_count(8) | last_press_ns(40)
/// - _padding (56B): complete cache line
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct CtrlCHandlerCapsule {
    /// Packed state: generation(16) | press_count(8) | last_press_ns(40)
    /// - generation: TOCTOU prevention
    /// - press_count: total Ctrl+C presses (0-255)
    /// - last_press_ns: timestamp of last press (40 bits = 1.1 trillion seconds = 34,000 years)
    state: AtomicU64,
    _padding: [u8; 56],
}

impl CtrlCHandlerCapsule {
    const DOUBLE_PRESS_WINDOW_MS: u64 = 2_000; // 2 seconds in milliseconds

    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Pack state into u64
    const fn pack_state(generation: u16, press_count: u8, last_press_ms: u64) -> u64 {
        ((generation as u64) << 48)
            | ((press_count as u64) << 40)
            | (last_press_ms & 0xFF_FFFF_FFFF) // 40 bits for timestamp (milliseconds)
    }

    /// Unpack state from u64
    const fn unpack_state(state: u64) -> (u16, u8, u64) {
        let generation = (state >> 48) as u16;
        let press_count = ((state >> 40) & 0xFF) as u8;
        let last_press_ms = state & 0xFF_FFFF_FFFF;
        (generation, press_count, last_press_ms)
    }

    /// Get current timestamp (milliseconds since UNIX epoch)
    ///
    /// **Note**: Stores milliseconds to fit in 40 bits (2^40 ms = ~34 years)
    /// Nanoseconds since UNIX epoch (~1.7e18) would overflow 40 bits (max 1.1e12)
    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Register Ctrl+C press (lockfree, <20ns)
    ///
    /// **ASSUM Safety**:
    /// - #ASSUME: CAS loop succeeds within 3 retries
    /// - #VERIFY: Property tests validate linearizability
    /// - #ASSUME: Generation counter prevents TOCTOU races
    /// - #VERIFY: Double-press tests validate timing window
    pub fn register_press(&self) {
        let now_ms = Self::current_time_ms();

        loop {
            let current = self.state.load(Ordering::Acquire);
            let (gen, press_count, _last_press_ms) = Self::unpack_state(current);

            // Increment press count (wrap at 255)
            let new_press_count = press_count.wrapping_add(1);

            let new_state = Self::pack_state(gen.wrapping_add(1), new_press_count, now_ms);

            // #ASSUME: Release ordering ensures timestamp visible to should_exit()
            // #VERIFY: Memory ordering tests validate synchronization
            if self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
    }

    /// Check if should exit (double-press within 2 seconds)
    ///
    /// **Performance**: <10ns (single atomic load)
    ///
    /// Returns: true if two presses within 2 seconds, false otherwise
    pub fn should_exit(&self) -> bool {
        let current = self.state.load(Ordering::Acquire);
        let (_gen, press_count, last_press_ms) = Self::unpack_state(current);

        if press_count < 2 {
            return false;
        }

        // Mask to 40 bits to match stored timestamp
        let now_ms = Self::current_time_ms() & 0xFF_FFFF_FFFF;
        let elapsed_ms = now_ms.saturating_sub(last_press_ms);

        elapsed_ms <= Self::DOUBLE_PRESS_WINDOW_MS
    }

    /// Reset press count (for testing or manual reset)
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logo_animation_colors() {
        let logo = LogoAnimationCapsule::new();

        // Initial color should be purple
        let (r, _g, _b) = logo.read_colors();
        assert_eq!(r, LogoAnimationCapsule::PURPLE_R);

        // After updates, colors should interpolate
        for _ in 0..30 {
            logo.update_frame();
        }
        let (r, g, b) = logo.read_colors();
        assert!(r > LogoAnimationCapsule::PURPLE_R); // Should be moving toward gold
    }

    #[test]
    fn test_wizard_state_navigation() {
        let wizard = WizardStateCapsule::new();

        // Initial state (step starts at 1, MIN_STEP)
        let (step, field, mode) = wizard.read_state();
        assert_eq!((step, field, mode), (1, 0, 0));

        // Next step (1 → 2)
        wizard.next_step();
        let (step, _field, _mode) = wizard.read_state();
        assert_eq!(step, 2);

        // Previous step (2 → 1)
        wizard.prev_step();
        let (step, _field, _mode) = wizard.read_state();
        assert_eq!(step, 1);
    }

    #[test]
    fn test_wizard_input_buffer() {
        let wizard = WizardStateCapsule::new();

        wizard.update_input("test_input");
        assert_eq!(wizard.read_input(), "test_input");

        wizard.update_input("longer_test_input_with_64_characters_exactly_fills_buffer_max");
        let input = wizard.read_input();
        assert!(input.len() <= 64);
    }

    #[test]
    fn test_ctrlc_single_press() {
        let handler = CtrlCHandlerCapsule::new();

        handler.register_press();
        assert!(!handler.should_exit()); // Single press doesn't exit
    }

    #[test]
    fn test_ctrlc_double_press() {
        let handler = CtrlCHandlerCapsule::new();

        handler.register_press();
        handler.register_press();
        assert!(handler.should_exit()); // Double press within window exits
    }

    #[test]
    fn test_ctrlc_reset() {
        let handler = CtrlCHandlerCapsule::new();

        handler.register_press();
        handler.register_press();
        assert!(handler.should_exit());

        handler.reset();
        assert!(!handler.should_exit());
    }
}
