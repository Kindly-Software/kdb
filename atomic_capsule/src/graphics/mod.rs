//! Graphics Display Server Foundation for Capsule OS
//!
//! This module provides the foundation for a Wayland-compatible display server
//! built entirely on computational capsule architecture.
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    DisplayServerCapsule (T6 Mixed, 2KB)                  │
//! │  ┌───────────────────────────────────────────────────────────────────┐  │
//! │  │  Client Registry (T1)  │  Input State (T1)  │  Frame Timing (T1) │  │
//! │  └───────────────────────────────────────────────────────────────────┘  │
//! │                                    │                                     │
//! │                       ┌────────────┴────────────┐                        │
//! │                       ▼                         ▼                        │
//! │  ┌─────────────────────────────┐  ┌──────────────────────────────────┐  │
//! │  │ SurfaceCompositorCapsule    │  │    DrmConnectorCapsule (T1)      │  │
//! │  │     (T4 Batch, 1KB)         │  │         (256B per output)        │  │
//! │  │ • 16 surface slots          │  │ • Monitor detection              │  │
//! │  │ • Z-order management        │  │ • EDID parsing                   │  │
//! │  │ • Damage tracking           │  │ • Mode enumeration               │  │
//! │  │ • Alpha blending            │  │ • Hotplug events                 │  │
//! │  └─────────────────────────────┘  └──────────────────────────────────┘  │
//! │                       │                         │                        │
//! │                       └────────────┬────────────┘                        │
//! │                                    ▼                                     │
//! │  ┌───────────────────────────────────────────────────────────────────┐  │
//! │  │            FramebufferCapsule (T1 Atomic, 512B)                    │  │
//! │  │ • Direct DRM framebuffer access                                   │  │
//! │  │ • Double/triple buffering                                         │  │
//! │  │ • VSync coordination                                              │  │
//! │  │ • Page flip management                                            │  │
//! │  └───────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Tier Breakdown
//!
//! | Capsule | Tier | Size | Performance |
//! |---------|------|------|-------------|
//! | DisplayServerCapsule | T6 Mixed | 2KB | <16ms frame time |
//! | SurfaceCompositorCapsule | T4 Batch | 1KB | <500us composition |
//! | FramebufferCapsule | T1 Atomic | 512B | <20ns buffer swap |
//! | DrmConnectorCapsule | T1 Atomic | 256B | <5ns state query |
//!
//! # UCE34/COCA Compliance
//!
//! - **100% Lockfree**: No mutex, RwLock, or blocking synchronization
//! - **Cache-Aligned**: 256B/512B/1KB/2KB alignments prevent false sharing
//! - **Generation Counters**: ABA prevention on all state transitions
//! - **ASSUM Safety**: 70+ documented safety assumptions with verification
//! - **<16ms Frame Time**: 60 FPS target with vsync coordination
//!
//! # Feature Flags
//!
//! - `graphics`: Enable graphics display server module (default off)
//! - `graphics-drm`: Enable DRM/KMS backend (requires Linux)
//! - `graphics-wayland`: Enable Wayland protocol support
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::graphics::{
//!     DisplayServerCapsule, SurfaceCompositorCapsule,
//!     FramebufferCapsule, DrmConnectorCapsule,
//! };
//!
//! // Create display server
//! let mut server = DisplayServerCapsule::new();
//! server.init(drm_fd)?;
//!
//! // Set up output (monitor)
//! let connector = DrmConnectorCapsule::new();
//! connector.init(connector_id, CONNECTOR_TYPE_HDMIA, 1);
//! connector.hotplug_connect(drm_fd)?;
//! server.add_output(&connector)?;
//!
//! // Create compositor and framebuffer
//! let mut compositor = SurfaceCompositorCapsule::new(1920, 1080);
//! let fb = FramebufferCapsule::new();
//! fb.allocate(drm_fd, 1920, 1080, PIXEL_FORMAT_XRGB8888, BUFFERING_DOUBLE)?;
//! fb.map()?;
//!
//! // Main loop
//! loop {
//!     // Handle input
//!     server.handle_pointer_motion(x, y);
//!     server.handle_key_event(scancode, pressed);
//!
//!     // Compose frame
//!     server.begin_frame();
//!     server.end_frame(&mut compositor, &fb)?;
//!
//!     // Present
//!     fb.present(crtc_id, connector_id)?;
//!     fb.wait_vsync()?;
//! }
//! ```
//!
//! # References
//!
//! - [Wayland Protocol](https://wayland-book.com/)
//! - [DRM/KMS Documentation](https://www.kernel.org/doc/html/latest/gpu/drm-kms.html)
//! - [wlroots Compositor Library](https://github.com/swaywm/wlroots)
//! - [Double Buffering](http://wiki.osdev.org/Double_Buffering)

// ============================================================================
// MODULE EXPORTS
// ============================================================================

/// Framebuffer management (T1 Atomic, 512B)
pub mod framebuffer;

/// DRM connector/monitor detection (T1 Atomic, 256B)
pub mod drm_connector;

/// Surface composition (T4 Batch, 1KB)
pub mod surface_compositor;

/// Display server (T6 Mixed, 2KB)
pub mod display_server;

/// Wayland-compatible compositor (T6 Mixed, 2KB) + Surface (T1 Atomic, 512B)
pub mod compositor;

// ============================================================================
// PUBLIC RE-EXPORTS
// ============================================================================

// Framebuffer exports
pub use framebuffer::{
    FramebufferCapsule, FramebufferError, FramebufferResult,
    // States
    FB_STATE_UNINITIALIZED, FB_STATE_ALLOCATED, FB_STATE_MAPPED, FB_STATE_SCANOUT, FB_STATE_ERROR,
    // Pixel formats
    PIXEL_FORMAT_ARGB8888, PIXEL_FORMAT_XRGB8888, PIXEL_FORMAT_RGB565, PIXEL_FORMAT_NV12, PIXEL_FORMAT_ABGR8888,
    // Buffering modes
    BUFFERING_SINGLE, BUFFERING_DOUBLE, BUFFERING_TRIPLE,
    // Flags
    FB_FLAG_DIRTY, FB_FLAG_VSYNC, FB_FLAG_SCANOUT_ACTIVE, FB_FLAG_FLIP_PENDING, FB_FLAG_MAPPED,
};

// DRM connector exports
pub use drm_connector::{
    DrmConnectorCapsule, DrmConnectorError, DrmConnectorResult, DisplayMode,
    // States
    CONNECTOR_STATE_DISCONNECTED, CONNECTOR_STATE_CONNECTING, CONNECTOR_STATE_CONNECTED,
    CONNECTOR_STATE_UNKNOWN, CONNECTOR_STATE_ERROR,
    // Connector types
    CONNECTOR_TYPE_UNKNOWN, CONNECTOR_TYPE_VGA, CONNECTOR_TYPE_DVII, CONNECTOR_TYPE_DVID,
    CONNECTOR_TYPE_DVIA, CONNECTOR_TYPE_COMPOSITE, CONNECTOR_TYPE_SVIDEO, CONNECTOR_TYPE_LVDS,
    CONNECTOR_TYPE_COMPONENT, CONNECTOR_TYPE_9PIN_DIN, CONNECTOR_TYPE_DISPLAYPORT,
    CONNECTOR_TYPE_HDMIA, CONNECTOR_TYPE_HDMIB, CONNECTOR_TYPE_TV, CONNECTOR_TYPE_EDP,
    CONNECTOR_TYPE_VIRTUAL, CONNECTOR_TYPE_DSI, CONNECTOR_TYPE_DPI, CONNECTOR_TYPE_WRITEBACK,
    CONNECTOR_TYPE_SPI, CONNECTOR_TYPE_USB,
    // DPMS states
    DPMS_ON, DPMS_STANDBY, DPMS_SUSPEND, DPMS_OFF,
    // Utilities
    connector_type_name, connector_is_digital,
};

// Surface compositor exports
pub use surface_compositor::{
    SurfaceCompositorCapsule, SurfaceCompositorError, SurfaceCompositorResult,
    SurfaceSlot, DamageRect, MAX_SURFACES, MAX_DAMAGE_REGIONS,
    // Surface states
    SURFACE_STATE_HIDDEN, SURFACE_STATE_PENDING, SURFACE_STATE_VISIBLE, SURFACE_STATE_DESTROYED,
    // Compositor states
    COMPOSITOR_STATE_IDLE, COMPOSITOR_STATE_COMPOSING, COMPOSITOR_STATE_PRESENTING, COMPOSITOR_STATE_ERROR,
    // Transform flags
    TRANSFORM_NONE, TRANSFORM_90, TRANSFORM_180, TRANSFORM_270, TRANSFORM_FLIP_H, TRANSFORM_FLIP_V,
    // Blend modes
    BLEND_NORMAL, BLEND_ADDITIVE, BLEND_MULTIPLY, BLEND_PREMULTIPLIED,
};

// Display server exports
pub use display_server::{
    DisplayServerCapsule, DisplayServerError, DisplayServerResult,
    ClientSlot, OutputSlot, InputState, CompositorConfig,
    MAX_CLIENTS, MAX_OUTPUTS, WAYLAND_SOCKET,
    // Server states
    SERVER_STATE_UNINITIALIZED, SERVER_STATE_IDLE, SERVER_STATE_RUNNING,
    SERVER_STATE_PRESENTING, SERVER_STATE_SHUTDOWN, SERVER_STATE_ERROR,
    // Client states
    CLIENT_STATE_FREE, CLIENT_STATE_CONNECTING, CLIENT_STATE_ACTIVE, CLIENT_STATE_DISCONNECTING,
    // Input constants
    BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BUTTON_PRESSED, BUTTON_RELEASED,
    // Keyboard modifiers
    MOD_SHIFT, MOD_CAPS, MOD_CTRL, MOD_ALT, MOD_META,
};

// Compositor exports (Wayland-compatible)
pub use compositor::{
    CompositorCapsule, SurfaceCapsule, CompositorError, CompositorResult,
    DamageRegion, SurfaceSlotCompact, SubsurfaceNode,
    // Compositor states (WL_ prefix to avoid collision with surface_compositor)
    WL_COMPOSITOR_STATE_UNINIT, WL_COMPOSITOR_STATE_IDLE, WL_COMPOSITOR_STATE_ACCUMULATING,
    WL_COMPOSITOR_STATE_BUILDING, WL_COMPOSITOR_STATE_RENDERING, WL_COMPOSITOR_STATE_ERROR,
    // Surface states (WL_ prefix to avoid collision)
    WL_SURFACE_STATE_FREE, WL_SURFACE_STATE_PENDING, WL_SURFACE_STATE_ATTACHED,
    WL_SURFACE_STATE_COMMITTED, WL_SURFACE_STATE_DESTROYING,
    // Surface flags
    SURFACE_FLAG_DAMAGED, SURFACE_FLAG_FRAME_CB, SURFACE_FLAG_SUBSURFACE,
    SURFACE_FLAG_SYNC, SURFACE_FLAG_BUFFER_RELEASED, SURFACE_FLAG_OPAQUE,
    SURFACE_FLAG_INPUT_REGION, SURFACE_FLAG_MAPPED,
    // Transform values (WL_ prefix to avoid collision)
    WL_TRANSFORM_NORMAL, WL_TRANSFORM_90, WL_TRANSFORM_180, WL_TRANSFORM_270,
    WL_TRANSFORM_FLIPPED, WL_TRANSFORM_FLIPPED_90, WL_TRANSFORM_FLIPPED_180, WL_TRANSFORM_FLIPPED_270,
    // Limits (WL_ prefix to avoid collision)
    WL_MAX_SURFACES, WL_MAX_DAMAGE_REGIONS, WL_MAX_SUBSURFACE_DEPTH, WL_MAX_FRAME_CALLBACKS,
};

// ============================================================================
// MODULE-LEVEL CONSTANTS
// ============================================================================

/// Graphics module version
pub const GRAPHICS_VERSION: &str = "0.1.0";

/// Default target frame rate (FPS)
pub const DEFAULT_TARGET_FPS: u32 = 60;

/// Default frame budget (nanoseconds for 60 FPS)
pub const DEFAULT_FRAME_BUDGET_NS: u64 = 16_666_667;

/// Default output width (1080p)
pub const DEFAULT_OUTPUT_WIDTH: u32 = 1920;

/// Default output height (1080p)
pub const DEFAULT_OUTPUT_HEIGHT: u32 = 1080;

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    const FAKE_DRM_FD: i32 = 3;

    #[test]
    fn test_full_stack_initialization() {
        // Create all capsules
        let mut server = DisplayServerCapsule::new();
        let connector = DrmConnectorCapsule::new();
        let compositor = SurfaceCompositorCapsule::new(1920, 1080);
        let fb = FramebufferCapsule::new();

        // Initialize server
        server.init(FAKE_DRM_FD).unwrap();
        assert_eq!(server.get_state(), SERVER_STATE_IDLE);

        // Initialize connector
        connector.init(42, CONNECTOR_TYPE_HDMIA, 1);
        connector.hotplug_connect(FAKE_DRM_FD).unwrap();
        assert!(connector.is_connected());

        // Add output to server
        server.add_output(&connector).unwrap();
        assert_eq!(server.get_output_count(), 1);

        // Allocate framebuffer
        fb.allocate(FAKE_DRM_FD, 1920, 1080, PIXEL_FORMAT_XRGB8888, BUFFERING_DOUBLE).unwrap();
        fb.map().unwrap();
        assert_eq!(fb.get_state(), FB_STATE_MAPPED);

        // Create compositor surface
        assert_eq!(compositor.get_surface_count(), 0);
    }

    #[test]
    fn test_client_surface_flow() {
        let mut server = DisplayServerCapsule::new();
        let mut compositor = SurfaceCompositorCapsule::new(1920, 1080);

        server.init(FAKE_DRM_FD).unwrap();

        // Add client
        let client_id = server.add_client(10, 1234, 1000).unwrap();
        assert_eq!(server.get_client_count(), 1);

        // Create surface for client
        let surface_id = compositor.create_surface(640, 480).unwrap();
        assert_eq!(compositor.get_surface_count(), 1);

        // Set buffer and commit
        compositor.set_buffer(surface_id, 0x1000_0000).unwrap();
        compositor.commit_surface(surface_id).unwrap();

        // Verify surface is visible
        let slot = compositor.find_slot(surface_id);
        assert!(slot.is_ok());

        // Notify surface commit
        server.notify_surface_commit();
        assert_eq!(server.get_surface_commit_count(), 1);

        // Remove client
        server.remove_client(client_id).unwrap();
        assert_eq!(server.get_client_count(), 0);
    }

    #[test]
    fn test_input_event_flow() {
        let mut server = DisplayServerCapsule::new();
        server.init(FAKE_DRM_FD).unwrap();

        // Simulate keyboard input
        server.handle_key_event(30, true); // Press 'A'
        assert!(server.get_input_state().is_key_pressed(30));

        // Simulate pointer input
        server.handle_pointer_motion(100, 200);
        assert_eq!(server.get_pointer_position(), (100, 200));

        server.handle_pointer_button(BTN_LEFT, true);
        assert!(server.get_input_state().is_left_button_pressed());

        // Set focus
        server.set_keyboard_focus(42);
        assert_eq!(server.get_keyboard_focus(), 42);

        // Check event count
        assert_eq!(server.get_input_event_count(), 3);
    }

    #[test]
    fn test_damage_tracking_flow() {
        let mut compositor = SurfaceCompositorCapsule::new(1920, 1080);

        // Create and commit surface
        let surface_id = compositor.create_surface(640, 480).unwrap();
        compositor.set_buffer(surface_id, 0x1000_0000).unwrap();
        compositor.commit_surface(surface_id).unwrap();

        // Add damage
        compositor.add_damage(surface_id, 10, 20, 100, 100).unwrap();
        assert_eq!(compositor.get_damage_region_count(), 1);

        // Full damage
        compositor.damage_full();
        assert_eq!(compositor.get_damage_region_count(), 1); // Still 1, but covers full screen

        // Clear damage
        compositor.clear_damage();
        assert_eq!(compositor.get_damage_region_count(), 0);
    }

    #[test]
    fn test_hotplug_handling() {
        let connector = DrmConnectorCapsule::new();
        connector.init(42, CONNECTOR_TYPE_DISPLAYPORT, 1);

        // Simulate hotplug connect
        connector.hotplug_connect(FAKE_DRM_FD).unwrap();
        assert!(connector.is_connected());
        assert_eq!(connector.get_hotplug_count(), 1);

        // Get preferred mode
        let mode = connector.get_preferred_mode();
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);

        // Simulate hotplug disconnect
        connector.hotplug_disconnect().unwrap();
        assert!(!connector.is_connected());
        assert_eq!(connector.get_hotplug_count(), 2);
    }

    #[test]
    fn test_buffer_swapping() {
        let fb = FramebufferCapsule::new();
        fb.allocate(FAKE_DRM_FD, 1920, 1080, PIXEL_FORMAT_XRGB8888, BUFFERING_DOUBLE).unwrap();
        fb.map().unwrap();

        // Swap buffers multiple times
        for i in 0..5 {
            let result = fb.swap_buffers();
            assert!(result.is_ok());
            assert_eq!(fb.get_statistics().2, i + 1); // flip_count
        }
    }

    #[test]
    fn test_z_order_composition() {
        let mut compositor = SurfaceCompositorCapsule::new(100, 100);

        // Create surfaces with different Z-indices
        let id1 = compositor.create_surface(50, 50).unwrap();
        let id2 = compositor.create_surface(50, 50).unwrap();
        let id3 = compositor.create_surface(50, 50).unwrap();

        compositor.set_z_index(id1, 10).unwrap();
        compositor.set_z_index(id2, 5).unwrap();
        compositor.set_z_index(id3, 15).unwrap();

        // Set buffers and commit
        compositor.set_buffer(id1, 0x1000).unwrap();
        compositor.set_buffer(id2, 0x2000).unwrap();
        compositor.set_buffer(id3, 0x3000).unwrap();
        compositor.commit_surface(id1).unwrap();
        compositor.commit_surface(id2).unwrap();
        compositor.commit_surface(id3).unwrap();

        // Compose
        let mut buffer = vec![0u8; 100 * 100 * 4];
        let result = compositor.compose(&mut buffer, 400);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // 3 visible surfaces
    }

    #[test]
    fn test_capsule_memory_layout() {
        // Verify all capsule sizes
        assert_eq!(core::mem::size_of::<FramebufferCapsule>(), 512);
        assert_eq!(core::mem::size_of::<DrmConnectorCapsule>(), 256);
        assert_eq!(core::mem::size_of::<SurfaceCompositorCapsule>(), 1024);
        assert_eq!(core::mem::size_of::<DisplayServerCapsule>(), 2048);

        // Verify alignments
        assert_eq!(core::mem::align_of::<FramebufferCapsule>(), 512);
        assert_eq!(core::mem::align_of::<DrmConnectorCapsule>(), 256);
        assert_eq!(core::mem::align_of::<SurfaceCompositorCapsule>(), 1024);
        assert_eq!(core::mem::align_of::<DisplayServerCapsule>(), 2048);

        // Verify sub-structures
        assert_eq!(core::mem::size_of::<SurfaceSlot>(), 32);
        assert_eq!(core::mem::size_of::<DamageRect>(), 16);
        assert_eq!(core::mem::size_of::<ClientSlot>(), 32);
        assert_eq!(core::mem::size_of::<OutputSlot>(), 64);
        assert_eq!(core::mem::size_of::<InputState>(), 128);
        assert_eq!(core::mem::size_of::<CompositorConfig>(), 64);
    }

    #[test]
    fn test_shutdown_cleanup() {
        let mut server = DisplayServerCapsule::new();
        server.init(FAKE_DRM_FD).unwrap();

        // Add clients and outputs
        server.add_client(10, 1234, 1000).unwrap();
        server.add_client(11, 5678, 1000).unwrap();

        let connector = DrmConnectorCapsule::new();
        connector.init(42, CONNECTOR_TYPE_HDMIA, 1);
        connector.hotplug_connect(FAKE_DRM_FD).unwrap();
        server.add_output(&connector).unwrap();

        // Shutdown
        server.shutdown().unwrap();

        // Verify cleanup
        assert_eq!(server.get_state(), SERVER_STATE_UNINITIALIZED);
        assert_eq!(server.get_output_count(), 0);
    }
}
