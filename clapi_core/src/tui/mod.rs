//! TUI (Terminal User Interface) - Live Metrics Dashboard with Computational Capsule Architecture
//!
//! # Purpose
//! Interactive terminal dashboard for clapi metrics with:
//! - **100% lockfree architecture** - No Mutex/RwLock in hot paths
//! - **Real-time updates** - Background HTTP polling with atomic state
//! - **Byzantine Purple theme** - Byzantine Purple (#663399) + Gold (#FFD700) branding
//! - **Sub-5ms rendering** - Ratatui with optimized layouts
//!
//! # UCE34 Framework
//! - **Q1-Q9**: Terminal UI for metrics display (real-time data presentation)
//! - **Q10**: Tier 1 (Atomic) - DashboardContentCapsule for lockfree metrics cache
//! - **Q11-Q28**: Ratatui rendering, HTTP polling, atomic state updates
//! - **Q31**: Simplicity - Clean layouts, minimal dependencies (ratatui, crossterm)
//! - **Q33**: Validation - Compile-time capsule verification
//! - **Q34**: N/A (read-only dashboard, no state modification)
//!
//! # Design Principles
//! - **Lockfree Mandate**: All metrics cached in atomic capsule
//! - **Progressive Disclosure**: Simple overview, detailed tables
//! - **Visual Feedback**: Color-coded status, trend indicators
//! - **Responsive Layout**: Terminal width-aware rendering
//! - **Graceful Degradation**: Handle network errors, missing metrics
//!
//! # Performance Targets
//! - **HTTP polling**: <50ms (local endpoint)
//! - **Atomic updates**: <10ns per field
//! - **Rendering**: <5ms (full screen refresh with ratatui)
//! - **Memory**: <20MB (stateless polling + capsule cache)
//!
//! # Module Structure
//! - `app.rs` - TUI application state and event loop
//! - `content.rs` - DashboardContentCapsule and rendering logic
//! - `input.rs` - CommandInputCapsule for readline-style editing with history
//!
//! # Controls
//! - `?`: Toggle help overlay (keyboard shortcuts guide)
//! - `/`: Open command palette with fuzzy search
//! - `q`: Quit dashboard
//! - `Esc`: Close palette or quit (if palette not visible)
//! - `p`: Pause updates
//! - `r`: Resume updates
//! - `Ctrl+C`: Force quit (immediate)
//! - `Ctrl+R`: Force refresh display

pub mod app;
pub mod content;
pub mod input;
pub mod colors;
pub mod state;
pub mod palette;     // Command palette (/ trigger, fuzzy search, lockfree)
pub mod layout;      // Layout rendering for ratatui
pub mod dispatcher;  // Command dispatcher (execution engine)
pub mod server_control;  // Server lifecycle management (start/stop/restart)
pub mod polling;     // Background HTTP polling with exponential backoff
pub mod progress;    // Progress indicator (spinner animation for async commands)
pub mod help;        // Help overlay (? trigger, keyboard shortcuts guide)
pub mod persistence; // History persistence capsule (atomic file I/O)
pub mod output;      // Command output capsule (ring buffer for TUI display)
pub mod tabs;        // Tab rendering (providers, budgets)
pub mod tab_renderers;  // Tab-specific rendering functions
pub mod logo_animation;  // Logo animation capsule (Byzantine Purple ↔ Gold ping-pong)

pub use app::TuiApp;
pub use content::DashboardContentCapsule;
pub use input::{CommandInputCapsule, CommandHistory, InputHandler};
pub use palette::{CommandPalette, CommandPaletteCapsule};
pub use dispatcher::{CommandDispatcher, CommandDispatcherCapsule, ExecutionState, ExecutionStats};
pub use server_control::{ProcessState, ServerController, ServerProcessCapsule};
pub use polling::{MetricsPoller, MetricsPollingCapsule, PollingStats};
pub use progress::ProgressIndicatorCapsule;
pub use persistence::{HistoryPersistenceCapsule, HistoryPersistenceManager};
pub use output::CommandOutputCapsule;
pub use state::{
    CommandHistoryEntry, ServerStatusCapsule, ServerStatusSnapshot, TuiStateCapsule,
    TuiStateSnapshot,
};
pub use help::{HelpOverlayCapsule, render_help_overlay};
pub use tabs::{DashboardTab, TabStateCapsule};
pub use tab_renderers::{
    render_overview_tab, render_providers_tab, render_budgets_tab,
    render_performance_tab, render_cost_tab, render_tab_indicator,
};
pub use logo_animation::{LogoAnimationCapsule, spawn_logo_animator};
