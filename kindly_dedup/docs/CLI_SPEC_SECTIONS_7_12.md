# kindly_dedup CLI Experience & API Design Specification
## Sections 7-12: Technical Implementation & Complete Reference

**Version**: 2.0 (Sections 7-12)  
**Date**: 2025-11-10  
**Status**: PRODUCTION-READY SPECIFICATION  
**Sections 1-6**: Complete (brand identity, CLI UX, public API, license, Q34 audit)  
**Sections 7-12**: This document (technical architecture → appendices)

---

## Table of Contents

- [Section 7: Technical Architecture](#section-7-technical-architecture)
- [Section 8: UCE34 Framework Analysis (Q1-Q34)](#section-8-uce34-framework-analysis-q1-q34)
- [Section 9: Implementation Plan](#section-9-implementation-plan)
- [Section 10: Edge Cases & Error Scenarios](#section-10-edge-cases--error-scenarios)
- [Section 11: Testing Strategy (T28)](#section-11-testing-strategy-t28)
- [Section 12: Appendices](#section-12-appendices)

---

# Section 7: Technical Architecture

## 7.1 Module Structure (Detailed File Tree)

```
kindly_dedup/
├─ src/
│  ├─ lib.rs                      # Public API exports
│  ├─ api/
│  │  ├─ mod.rs                   # Public DedupClient API
│  │  ├─ client.rs                # DedupClient implementation
│  │  ├─ error.rs                 # Public error types (ApiError)
│  │  └─ types.rs                 # Public types (Document, Cluster, Config)
│  │
│  ├─ cli/
│  │  ├─ mod.rs                   # CLI entry point (main TUI loop)
│  │  ├─ welcome.rs               # Welcome screen (pulsing hearts)
│  │  ├─ main_menu.rs             # Main menu (7 options)
│  │  ├─ file_selection.rs        # File browser + manual entry
│  │  ├─ config_ui.rs             # Configuration UI (sliders, checkboxes)
│  │  ├─ progress.rs              # Progress rendering (real-time metrics)
│  │  ├─ results.rs               # Results summary (achievements, export)
│  │  ├─ license_ui.rs            # License status display
│  │  ├─ help.rs                  # Help screens (keyboard shortcuts)
│  │  └─ input.rs                 # Keyboard input handling
│  │
│  ├─ core/
│  │  ├─ mod.rs                   # Core dedup engine (proprietary)
│  │  ├─ minhash.rs               # MinHash signature computation
│  │  ├─ lsh.rs                   # LSH bucketing
│  │  ├─ union_find.rs            # Union-Find clustering
│  │  ├─ tokenize.rs              # Tokenization
│  │  ├─ parallel.rs              # Parallel processing (atomic_capsule)
│  │  └─ bloom.rs                 # Bloom pre-filter
│  │
│  ├─ license/
│  │  ├─ mod.rs                   # CryptoLicenseCapsule wrapper
│  │  ├─ crypto_license.rs        # License verification (Ed25519)
│  │  ├─ tier_enforcement.rs      # Tier limits enforcement
│  │  └─ trial.rs                 # Trial mode (7-day, 100K docs)
│  │
│  ├─ audit/
│  │  ├─ mod.rs                   # Q34 audit trail
│  │  ├─ audit_logger.rs          # Hash-chained logger (Blake3)
│  │  ├─ compliance_report.rs     # SOX/SOC2/GDPR/HIPAA reports
│  │  └─ verification.rs          # Audit trail verification
│  │
│  ├─ animation/
│  │  ├─ mod.rs                   # Animation engine
│  │  ├─ frame_scheduler.rs       # 8-60 FPS scheduler
│  │  ├─ pulsing_heart.rs         # Purple heart brightness cycling
│  │  ├─ progress_bar.rs          # Progress bar renderer
│  │  ├─ spinner.rs               # Rotating emoji spinner
│  │  └─ celebration.rs           # Sparkle animation
│  │
│  ├─ state/
│  │  ├─ mod.rs                   # Lockfree state capsules
│  │  ├─ menu_state.rs            # MenuStateCapsule (T1 Atomic)
│  │  ├─ progress_tracker.rs      # ProgressTrackerCapsule (T1 Atomic)
│  │  ├─ animation_state.rs       # AnimationStateCapsule (T1 Atomic)
│  │  └─ license_state.rs         # LicenseStateCapsule (T1 Atomic)
│  │
│  └─ utils/
│     ├─ mod.rs                   # Utility modules
│     ├─ terminal.rs              # Colors, emojis (EXISTING, enhanced)
│     ├─ box_drawing.rs           # Unicode box drawing characters
│     ├─ cursor.rs                # Cursor management (save/restore)
│     └─ error_messages.rs        # Friendly error message templates
│
├─ benches/                       # B32 benchmarks
├─ tests/                         # T28 tests
└─ docs/
   ├─ CLI_SPEC_SECTIONS_1_6.md    # Sections 1-6 (complete)
   ├─ CLI_SPEC_SECTIONS_7_12.md   # This document
   └─ IMPLEMENTATION_GUIDE.md     # Developer implementation guide
```

**Module Dependencies**:
- `api/` → `core/` (dedup engine)
- `cli/` → `api/` (public API only, no direct core access)
- `cli/` → `state/` (lockfree atomic capsules)
- `cli/` → `animation/` (rendering engine)
- `cli/` → `license/` (tier enforcement)
- `cli/` → `audit/` (Q34 logging)
- `animation/` → `state/` (AnimationStateCapsule)
- `license/` → `state/` (LicenseStateCapsule)

**CRITICAL**: CLI never directly accesses `core/`. All dedup operations go through `api/` (DedupClient). This enforces proprietary IP protection.

---

## 7.2 State Management (Lockfree Atomic Capsules)

All CLI state uses computational capsules from atomic_capsule (T1 Atomic tier):

### 7.2.1 MenuStateCapsule (64 bytes, cache-aligned)

```rust
use atomic_capsule_derive::ComputationalCapsule;
use atomic_capsule::patterns::DualAtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct MenuStateCapsule {
    // Primary atomic: selected_index (16 bits) + animation_frame (16 bits) + flags (32 bits)
    primary: AtomicU64,
    
    // Secondary atomic: timestamp (64 bits)
    secondary: AtomicU64,
    
    // Generation counter (ABA prevention)
    generation: AtomicU64,
    
    // Padding to 64 bytes
    _padding: [u8; 40],
}

impl MenuStateCapsule {
    pub fn new() -> Self { /* ... */ }
    
    // Lockfree read (Acquire)
    pub fn selected_index(&self) -> u16 { /* ... */ }
    pub fn animation_frame(&self) -> u16 { /* ... */ }
    pub fn is_animating(&self) -> bool { /* ... */ }
    
    // Lockfree write (Release)
    pub fn set_selected_index(&self, index: u16) { /* ... */ }
    pub fn increment_animation_frame(&self) { /* ... */ }
    pub fn toggle_animation(&self) { /* ... */ }
    
    // Coordinated read (DualAtomicU64 pattern)
    pub fn read_snapshot(&self) -> (u16, u16, bool) { /* ... */ }
}
```

**Performance**: <5ns read, <15ns write (T1 Atomic baseline)

**Verification**: `#[derive(ComputationalCapsule)]` ensures compile-time alignment/size correctness

---

### 7.2.2 ProgressTrackerCapsule (128 bytes, cache-aligned)

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct ProgressTrackerCapsule {
    // Documents processed (64 bits)
    docs_processed: AtomicU64,
    
    // Duplicates found (64 bits)
    duplicates_found: AtomicU64,
    
    // Throughput (docs/sec, Q16.16 fixed-point, 32 bits)
    throughput_q16: AtomicU32,
    
    // Phase (0=init, 1=minhash, 2=lsh, 3=clustering, 4=done)
    phase: AtomicU8,
    
    // Flags (8 bits: paused, cancelled, error)
    flags: AtomicU8,
    
    // Timestamp (nanoseconds since start, 64 bits)
    timestamp_ns: AtomicU64,
    
    // Padding to 128 bytes
    _padding: [u8; 46],
}

impl ProgressTrackerCapsule {
    // Lockfree increment (Relaxed for throughput, no synchronization needed)
    pub fn increment_docs(&self) { /* ... */ }
    pub fn increment_duplicates(&self) { /* ... */ }
    
    // Lockfree read (Relaxed for metrics)
    pub fn docs_processed(&self) -> u64 { /* ... */ }
    pub fn duplicates_found(&self) -> u64 { /* ... */ }
    pub fn throughput_docs_per_sec(&self) -> f64 { /* Q16.16 → f64 */ }
    
    // Lockfree write (Release for phase transitions)
    pub fn set_phase(&self, phase: Phase) { /* ... */ }
    pub fn set_error(&self) { /* ... */ }
    pub fn set_paused(&self, paused: bool) { /* ... */ }
    
    // Coordinated snapshot (all metrics at once, Acquire)
    pub fn snapshot(&self) -> ProgressSnapshot { /* ... */ }
}
```

**Performance**: <5ns increment (Relaxed), <10ns snapshot (Acquire)

**Threading**: 16 concurrent writers (1 per thread), 1 reader (UI thread), zero contention

---

### 7.2.3 AnimationStateCapsule (64 bytes, cache-aligned)

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct AnimationStateCapsule {
    // Frame counter (64 bits, wraps at u64::MAX)
    frame_count: AtomicU64,
    
    // Brightness level (Q8.8 fixed-point, 0.0-1.0, 16 bits)
    brightness_q8: AtomicU16,
    
    // FPS (frames per second, 8 bits, 8-60 FPS)
    fps: AtomicU8,
    
    // Flags (8 bits: enabled, pulsing, celebrating)
    flags: AtomicU8,
    
    // Timestamp of last frame (nanoseconds, 64 bits)
    last_frame_ns: AtomicU64,
    
    // Padding to 64 bytes
    _padding: [u8; 44],
}

impl AnimationStateCapsule {
    // Lockfree read
    pub fn frame_count(&self) -> u64 { /* ... */ }
    pub fn brightness(&self) -> f32 { /* Q8.8 → f32, 0.0-1.0 */ }
    pub fn fps(&self) -> u8 { /* ... */ }
    pub fn is_enabled(&self) -> bool { /* ... */ }
    
    // Lockfree write
    pub fn increment_frame(&self) { /* ... */ }
    pub fn set_brightness(&self, brightness: f32) { /* f32 → Q8.8 */ }
    pub fn set_fps(&self, fps: u8) { /* Clamp 8-60 */ }
    pub fn toggle_enabled(&self) { /* ... */ }
    
    // Coordinated update (frame + brightness + timestamp)
    pub fn update_animation(&self, dt_ns: u64) -> bool { /* true if frame rendered */ }
}
```

**Performance**: <5ns read, <10ns update

**Algorithm**: Brightness cycling (pulsing heart)
```rust
// Brightness cycles 0.4 → 1.0 → 0.4 over 2 seconds (sinusoidal)
fn compute_brightness(frame: u64, fps: u8) -> f32 {
    let t = (frame % (fps as u64 * 2)) as f32 / (fps as f32 * 2.0); // 0.0-1.0 over 2s
    let brightness = 0.4 + 0.6 * (t * std::f32::consts::TAU).sin().abs(); // 0.4-1.0
    brightness
}
```

---

### 7.2.4 LicenseStateCapsule (256 bytes, cache-aligned)

```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct LicenseStateCapsule {
    // Tier (0=Free, 1=Pro, 2=Enterprise, 3=Trial)
    tier: AtomicU8,
    
    // Features (64-bit bitmask: parallel, persistent, simd, etc.)
    features: AtomicU64,
    
    // Expiration timestamp (Unix seconds, 64 bits)
    expiration_unix: AtomicU64,
    
    // Document limit (64 bits)
    doc_limit: AtomicU64,
    
    // Thread limit (8 bits, 1-256)
    thread_limit: AtomicU8,
    
    // Validation status (8 bits: valid, expired, invalid, trial)
    status: AtomicU8,
    
    // Last validation timestamp (nanoseconds, 64 bits)
    last_validated_ns: AtomicU64,
    
    // Hardware ID hash (Blake3, 256 bits = 32 bytes)
    hardware_id_hash: [AtomicU8; 32],
    
    // Padding to 256 bytes
    _padding: [u8; 130],
}

impl LicenseStateCapsule {
    // Lockfree read (Acquire for validation)
    pub fn tier(&self) -> LicenseTier { /* ... */ }
    pub fn is_expired(&self) -> bool { /* ... */ }
    pub fn doc_limit(&self) -> u64 { /* ... */ }
    pub fn thread_limit(&self) -> u8 { /* ... */ }
    pub fn has_feature(&self, feature: Feature) -> bool { /* ... */ }
    
    // Lockfree write (Release for validation)
    pub fn set_tier(&self, tier: LicenseTier) { /* ... */ }
    pub fn set_expiration(&self, expiration: SystemTime) { /* ... */ }
    pub fn set_status(&self, status: ValidationStatus) { /* ... */ }
    
    // Coordinated validation (all fields, Acquire)
    pub fn validate(&self) -> Result<(), LicenseError> { /* ... */ }
}
```

**Performance**: <10ns read, <20ns write, <500μs Ed25519 signature verification (cached)

**Security**: Ed25519 public key signature verification, hardware ID binding (TPM 2.0 or CPU serial)

---

## 7.3 Animation Engine Design

### 7.3.1 Frame Scheduler (8-60 FPS)

```rust
pub struct FrameScheduler {
    state: Arc<AnimationStateCapsule>,
    target_fps: u8, // 8-60 FPS
}

impl FrameScheduler {
    pub fn new(target_fps: u8) -> Self {
        assert!((8..=60).contains(&target_fps), "FPS must be 8-60");
        // ...
    }
    
    /// Returns true if a frame should be rendered
    pub fn should_render(&self) -> bool {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let last_frame_ns = self.state.last_frame_ns();
        let frame_duration_ns = 1_000_000_000 / self.target_fps as u64;
        
        if now_ns - last_frame_ns >= frame_duration_ns {
            self.state.update_animation(now_ns - last_frame_ns);
            true
        } else {
            false
        }
    }
    
    /// Sleep until next frame (non-blocking alternative)
    pub fn sleep_until_next_frame(&self) {
        let now_ns = /* ... */;
        let last_frame_ns = self.state.last_frame_ns();
        let frame_duration_ns = 1_000_000_000 / self.target_fps as u64;
        let elapsed = now_ns - last_frame_ns;
        
        if elapsed < frame_duration_ns {
            let sleep_ns = frame_duration_ns - elapsed;
            std::thread::sleep(std::time::Duration::from_nanos(sleep_ns));
        }
    }
}
```

**Performance**: <100ns per check, <16ms frame time @ 60 FPS

**Threading**: Single animation thread, reads ProgressTrackerCapsule (lockfree), writes to screen

---

### 7.3.2 Pulsing Purple Heart (Brightness Cycling Algorithm)

```rust
pub struct PulsingHeart {
    state: Arc<AnimationStateCapsule>,
}

impl PulsingHeart {
    /// Render pulsing purple heart at current brightness
    pub fn render(&self) -> String {
        use crate::utils::terminal::{Color, colorize_with_style, Style};
        
        let brightness = self.state.brightness(); // 0.4-1.0
        let emoji = "💜"; // Purple heart
        
        // Choose color based on brightness
        let color = if brightness > 0.8 {
            Color::ByzantinePurple  // Full brightness
        } else if brightness > 0.6 {
            Color::RoyalPurple      // Medium-high
        } else {
            Color::DeepPurple       // Medium-low
        };
        
        // Apply bold style at peak brightness
        if brightness > 0.9 {
            colorize_with_style(emoji, color, Style::Bold)
        } else {
            colorize(emoji, color)
        }
    }
    
    /// Update brightness based on frame count
    pub fn update(&self) {
        let frame = self.state.frame_count();
        let fps = self.state.fps();
        
        // Sinusoidal brightness: 0.4 → 1.0 → 0.4 over 2 seconds
        let t = (frame % (fps as u64 * 2)) as f32 / (fps as f32 * 2.0);
        let brightness = 0.4 + 0.6 * (t * std::f32::consts::TAU).sin().abs();
        
        self.state.set_brightness(brightness);
    }
}
```

**Performance**: <1ms render (ANSI codes + emoji), <10ns brightness update

**Visual Effect**:
- 2-second cycle: fade in (0.4 → 1.0, 1 sec) → fade out (1.0 → 0.4, 1 sec)
- Color transitions: DeepPurple → RoyalPurple → ByzantinePurple → RoyalPurple → DeepPurple
- Bold style applied at peak brightness (>0.9)

---

### 7.3.3 Progress Bar Renderer (Smooth Updates)

```rust
pub struct ProgressBarRenderer {
    progress: Arc<ProgressTrackerCapsule>,
    width: u16, // Terminal width (auto-detected)
}

impl ProgressBarRenderer {
    pub fn render(&self) -> String {
        let snapshot = self.progress.snapshot();
        let percent = if snapshot.total > 0 {
            (snapshot.processed as f64 / snapshot.total as f64 * 100.0) as u8
        } else {
            0
        };
        
        // Progress bar: [████████████          ] 60% (600K/1M docs)
        let bar_width = self.width.saturating_sub(40); // Leave space for text
        let filled = (bar_width as f64 * percent as f64 / 100.0) as u16;
        let empty = bar_width - filled;
        
        format!(
            "[{}{}] {}% ({}/{})",
            "█".repeat(filled as usize).byzantine_purple(),
            " ".repeat(empty as usize),
            percent,
            format_number(snapshot.processed),
            format_number(snapshot.total)
        )
    }
}
```

**Performance**: <2ms render (string allocation + formatting)

**Features**:
- Auto-scales to terminal width
- Byzantine purple filled portion
- Real-time metrics (docs processed, duplicates found)
- Smooth updates (60 FPS max, batched renders)

---

### 7.3.4 Spinner Patterns (Rotating Emojis)

```rust
pub struct Spinner {
    state: Arc<AnimationStateCapsule>,
    pattern: &'static [&'static str],
}

impl Spinner {
    pub const ROCKET: &'static [&'static str] = &["🚀", "🛸", "🌙", "✨"];
    pub const HEARTS: &'static [&'static str] = &["💜", "💛", "💜", "💛"];
    pub const LOADING: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    
    pub fn new(pattern: &'static [&'static str]) -> Self {
        Self {
            state: Arc::new(AnimationStateCapsule::new()),
            pattern,
        }
    }
    
    pub fn render(&self) -> String {
        let frame = self.state.frame_count();
        let index = (frame % self.pattern.len() as u64) as usize;
        self.pattern[index].to_string()
    }
}
```

**Performance**: <50ns render (array lookup)

**Patterns**:
- `ROCKET`: 🚀 → 🛸 → 🌙 → ✨ (4 frames, 0.5s cycle @ 8 FPS)
- `HEARTS`: 💜 → 💛 → 💜 → 💛 (alternating brand colors)
- `LOADING`: Braille spinner (10 frames, smooth rotation)

---

### 7.3.5 Celebration Effects (Sparkle Animation)

```rust
pub struct CelebrationEffect {
    state: Arc<AnimationStateCapsule>,
    duration_frames: u64, // Total frames (e.g., 60 @ 60 FPS = 1 second)
}

impl CelebrationEffect {
    pub fn new(duration_sec: u8, fps: u8) -> Self {
        Self {
            state: Arc::new(AnimationStateCapsule::new()),
            duration_frames: duration_sec as u64 * fps as u64,
        }
    }
    
    pub fn render(&self) -> Option<String> {
        let frame = self.state.frame_count();
        
        if frame >= self.duration_frames {
            return None; // Animation complete
        }
        
        // Sparkle pattern: ✨💜🎉💛✨ (cycles through emojis)
        let pattern = ["✨", "💜", "🎉", "💛", "✨", "🏆", "💎", "👑"];
        let index = (frame % pattern.len() as u64) as usize;
        
        Some(format!(
            "{} {} {} {} {}",
            pattern[index],
            pattern[(index + 1) % pattern.len()],
            pattern[(index + 2) % pattern.len()],
            pattern[(index + 3) % pattern.len()],
            pattern[(index + 4) % pattern.len()]
        ))
    }
}
```

**Performance**: <100ns render

**Visual Effect**:
- 1-second sparkle animation on completion
- Cycles through 8 celebration emojis (✨💜🎉💛✨🏆💎👑)
- Auto-stops after duration

---

## 7.4 Terminal Compatibility Layer

### 7.4.1 Capability Detection

```rust
pub struct TerminalCapabilities {
    pub rgb_colors: bool,        // 24-bit RGB support
    pub emojis: bool,             // Unicode emoji support
    pub box_drawing: bool,        // Unicode box characters
    pub cursor_control: bool,     // Cursor save/restore
    pub resize_events: bool,      // Terminal resize detection
    pub width: u16,               // Terminal width (columns)
    pub height: u16,              // Terminal height (rows)
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        use std::io::IsTerminal;
        
        let is_tty = std::io::stdout().is_terminal();
        
        // Detect RGB color support (check COLORTERM env var)
        let rgb_colors = is_tty && std::env::var("COLORTERM")
            .map(|v| v == "truecolor" || v == "24bit")
            .unwrap_or(false);
        
        // Emojis require UTF-8 + modern terminal
        let emojis = is_tty && std::env::var("TERM")
            .map(|v| !v.contains("linux") && !v.contains("vt100"))
            .unwrap_or(false);
        
        // Box drawing: most terminals support Unicode
        let box_drawing = is_tty;
        
        // Cursor control: ANSI escape codes
        let cursor_control = is_tty;
        
        // Resize events: crossterm provides this
        let resize_events = is_tty;
        
        // Terminal size
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        
        Self {
            rgb_colors,
            emojis,
            box_drawing,
            cursor_control,
            resize_events,
            width,
            height,
        }
    }
}
```

**Performance**: <10ms initialization (one-time), <1ns cached reads

**Fallback Strategy**:
- No RGB colors → Use 16 ANSI colors (BrightMagenta instead of ByzantinePurple)
- No emojis → Use ASCII symbols (♥ instead of 💜, * instead of ✨)
- No box drawing → Use ASCII characters (+ - | instead of ┌─┐)
- No cursor control → Disable animations, use static output

---

### 7.4.2 Fallback Strategies

```rust
pub struct FallbackRenderer {
    caps: TerminalCapabilities,
}

impl FallbackRenderer {
    /// Render purple heart (emoji or ASCII fallback)
    pub fn purple_heart(&self) -> String {
        if self.caps.emojis {
            "💜".to_string()
        } else if self.caps.rgb_colors {
            "♥".bright_magenta().to_string()  // 16-color fallback
        } else {
            "♥".to_string()  // Plain ASCII
        }
    }
    
    /// Render box border (Unicode or ASCII fallback)
    pub fn box_border(&self, width: u16) -> (String, String, String) {
        if self.caps.box_drawing {
            // Unicode box drawing
            (
                format!("┌{}┐", "─".repeat(width as usize - 2)),
                format!("│{}│", " ".repeat(width as usize - 2)),
                format!("└{}┘", "─".repeat(width as usize - 2)),
            )
        } else {
            // ASCII fallback
            (
                format!("+{}+", "-".repeat(width as usize - 2)),
                format!("|{}|", " ".repeat(width as usize - 2)),
                format!("+{}+", "-".repeat(width as usize - 2)),
            )
        }
    }
    
    /// Render color (RGB or 16-color fallback)
    pub fn colorize(&self, text: &str, color: Color) -> String {
        if self.caps.rgb_colors {
            colorize(text, color)
        } else {
            // 16-color fallback
            match color {
                Color::ByzantinePurple => text.bright_magenta(),
                Color::ByzantineGold => text.bright_yellow(),
                _ => text.to_string(),
            }
        }
    }
}
```

**Fallback Matrix**:

| Terminal         | RGB Colors | Emojis | Box Drawing | Notes                     |
|------------------|------------|--------|-------------|---------------------------|
| iTerm2           | ✅         | ✅     | ✅          | Full support              |
| Windows Terminal | ✅         | ✅     | ✅          | Full support              |
| VS Code          | ✅         | ✅     | ✅          | Full support              |
| Alacritty        | ✅         | ✅     | ✅          | Full support              |
| xterm            | ⚠️         | ⚠️     | ✅          | Limited emoji support     |
| cmd.exe (Win10+) | ⚠️         | ❌     | ⚠️          | ASCII fallback            |
| Linux console    | ❌         | ❌     | ⚠️          | ASCII-only fallback       |

---

### 7.4.3 Terminal Size Detection + Resize Handling

```rust
pub struct TerminalSizeManager {
    current_size: Arc<Mutex<(u16, u16)>>, // (width, height)
}

impl TerminalSizeManager {
    pub fn new() -> Self {
        let size = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            current_size: Arc::new(Mutex::new(size)),
        }
    }
    
    /// Detect resize events (blocking, run in separate thread)
    pub fn watch_resize(&self) {
        use crossterm::event::{Event, read};
        
        loop {
            if let Ok(Event::Resize(width, height)) = read() {
                *self.current_size.lock().unwrap() = (width, height);
                // Trigger re-render (send event to UI thread)
            }
        }
    }
    
    pub fn size(&self) -> (u16, u16) {
        *self.current_size.lock().unwrap()
    }
}
```

**Performance**: <1ms resize detection, <10ns size read

**Re-render Strategy**:
- On resize: Clear screen, re-render all UI components
- Debounce: 100ms delay to avoid flickering during continuous resize

---

### 7.4.4 Cursor Management (Save/Restore Position)

```rust
pub mod cursor {
    use crossterm::cursor::{Hide, Show, MoveTo, SavePosition, RestorePosition};
    use crossterm::execute;
    use std::io::{stdout, Write};
    
    /// Save cursor position
    pub fn save() -> std::io::Result<()> {
        execute!(stdout(), SavePosition)
    }
    
    /// Restore cursor position
    pub fn restore() -> std::io::Result<()> {
        execute!(stdout(), RestorePosition)
    }
    
    /// Hide cursor (for animations)
    pub fn hide() -> std::io::Result<()> {
        execute!(stdout(), Hide)
    }
    
    /// Show cursor (after animations)
    pub fn show() -> std::io::Result<()> {
        execute!(stdout(), Show)
    }
    
    /// Move cursor to (x, y)
    pub fn move_to(x: u16, y: u16) -> std::io::Result<()> {
        execute!(stdout(), MoveTo(x, y))
    }
}
```

**Usage Example**:
```rust
cursor::hide()?;
cursor::save()?;

// Render animation frames...
render_animation();

cursor::restore()?;
cursor::show()?;
```

---

### 7.4.5 Clear Screen Without Flicker

```rust
pub mod screen {
    use crossterm::terminal::{Clear, ClearType};
    use crossterm::execute;
    use std::io::stdout;
    
    /// Clear entire screen (flicker-free)
    pub fn clear() -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::All))
    }
    
    /// Clear from cursor to end of screen
    pub fn clear_down() -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::FromCursorDown))
    }
    
    /// Clear current line
    pub fn clear_line() -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::CurrentLine))
    }
    
    /// Alternative screen (for full-screen TUI)
    pub fn enter_alt_screen() -> std::io::Result<()> {
        use crossterm::terminal::EnterAlternateScreen;
        execute!(stdout(), EnterAlternateScreen)
    }
    
    pub fn exit_alt_screen() -> std::io::Result<()> {
        use crossterm::terminal::LeaveAlternateScreen;
        execute!(stdout(), LeaveAlternateScreen)
    }
}
```

**Flicker Prevention**:
- Use alternate screen buffer (preserves terminal history)
- Double buffering: render to String, then write once
- Only update changed regions (partial re-render)

---

## 7.5 Error Handling Strategy

### 7.5.1 Error Taxonomy (9 Categories, 50+ Specific Errors)

```rust
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("File error: {0}")]
    File(#[from] FileError),
    
    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),
    
    #[error("License error: {0}")]
    License(#[from] LicenseError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),
    
    #[error("Audit error: {0}")]
    Audit(#[from] AuditError),
    
    #[error("Terminal error: {0}")]
    Terminal(#[from] TerminalError),
    
    #[error("User input error: {0}")]
    UserInput(#[from] UserInputError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
}

// Category-specific errors (10 errors each, see Section 10)
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File not found: {path}")]
    NotFound { path: PathBuf },
    
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },
    
    // ... (see Section 10 for all 50+ errors)
}
```

**Error Context**: All errors include:
- User-facing message (friendly, with emoji)
- Technical details (for logs)
- Recovery suggestion (actionable steps)
- Related documentation link

---

### 7.5.2 Recovery Strategies

```rust
pub enum RecoveryStrategy {
    Retry { max_attempts: u8, backoff_ms: u64 },
    Fallback { alternative: Box<dyn Fn() -> Result<(), CliError>> },
    Degrade { reduced_functionality: String },
    Cancel { cleanup: Box<dyn Fn()> },
}

impl CliError {
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            CliError::File(FileError::NotFound { .. }) => {
                RecoveryStrategy::Fallback {
                    alternative: Box::new(|| {
                        // Prompt user for correct path
                        Ok(())
                    }),
                }
            },
            CliError::Memory(MemoryError::OutOfMemory { .. }) => {
                RecoveryStrategy::Degrade {
                    reduced_functionality: "Enable persistent mode (saves to disk)".to_string(),
                }
            },
            CliError::License(LicenseError::Expired { .. }) => {
                RecoveryStrategy::Degrade {
                    reduced_functionality: "Degrade to Free tier (100K doc limit)".to_string(),
                }
            },
            _ => RecoveryStrategy::Cancel {
                cleanup: Box::new(|| {
                    // Save progress, graceful shutdown
                }),
            },
        }
    }
}
```

---

### 7.5.3 User-Friendly Message Templates

```rust
pub struct ErrorMessageTemplate {
    emoji: &'static str,
    title: String,
    description: String,
    suggestion: String,
    link: Option<String>,
}

impl CliError {
    pub fn friendly_message(&self) -> ErrorMessageTemplate {
        match self {
            CliError::File(FileError::NotFound { path }) => {
                ErrorMessageTemplate {
                    emoji: "📁",
                    title: "File not found".to_string(),
                    description: format!(
                        "We couldn't find the file at:\n  {}",
                        path.display()
                    ),
                    suggestion: format!(
                        "Try:\n  • Check the file path is correct\n  • Use the file browser (option 2)\n  • Did you mean: {}?",
                        fuzzy_match_path(path)  // Fuzzy path suggestion
                    ),
                    link: Some("https://docs.kindly.software/cli/file-not-found".to_string()),
                }
            },
            // ... (see Section 10 for all 50+ templates)
        }
    }
}
```

**Template Rendering**:
```rust
fn render_error(err: &CliError) {
    let msg = err.friendly_message();
    
    println!("{} {}", msg.emoji, msg.title.bold().red());
    println!();
    println!("{}", msg.description);
    println!();
    println!("{}", "💡 Suggestion:".bold().byzantine_purple());
    println!("{}", msg.suggestion);
    
    if let Some(link) = msg.link {
        println!();
        println!("{} {}", "📚 Learn more:".dim(), link.underline());
    }
}
```

**Example Output**:
```
📁 File not found

We couldn't find the file at:
  /home/user/data/corpus.jsonl

💡 Suggestion:
Try:
  • Check the file path is correct
  • Use the file browser (option 2)
  • Did you mean: /home/user/data/corpus2.jsonl?

📚 Learn more: https://docs.kindly.software/cli/file-not-found
```

---

### 7.5.4 Suggestion Engine (Fuzzy Path Matching)

```rust
use std::path::{Path, PathBuf};

pub fn fuzzy_match_path(target: &Path) -> Option<PathBuf> {
    let parent = target.parent()?;
    let target_name = target.file_name()?.to_str()?;
    
    // Read directory, find closest match (Levenshtein distance)
    let entries = std::fs::read_dir(parent).ok()?;
    let mut best_match: Option<(PathBuf, usize)> = None;
    
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let entry_str = entry_name.to_str()?;
        let distance = levenshtein_distance(target_name, entry_str);
        
        if distance < 5 {  // Close enough
            if let Some((_, best_dist)) = best_match {
                if distance < best_dist {
                    best_match = Some((entry.path(), distance));
                }
            } else {
                best_match = Some((entry.path(), distance));
            }
        }
    }
    
    best_match.map(|(path, _)| path)
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    // Dynamic programming implementation (standard algorithm)
    // ...
}
```

**Examples**:
- User types: `/data/corpus.jsonl` → Not found → Suggest: `/data/corpus2.jsonl` (1 char diff)
- User types: `/data/crpus.jsonl` → Not found → Suggest: `/data/corpus.jsonl` (typo fix)

---

(Continue in next file due to length...)
