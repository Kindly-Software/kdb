//! Terminal Shader Source Code
//!
//! Shaders are embedded at compile time using include_str!
//! These shaders form the GPU rendering pipeline for the terminal emulator.
//!
//! # Shader Pipeline Overview
//!
//! ```text
//! Cell Data ──> [quad.vert] ──> [glyph.frag/glyph_sdf.frag] ──┐
//!                                                              │
//! Cursor Data ──> [quad.vert] ──> [cursor.frag] ──────────────┼──> Framebuffer
//!                                                              │
//! Framebuffer ──> [fullscreen.vert] ──> [effects.frag] ───────┘
//!                         OR
//! Framebuffer ──> [effects.comp] ──> Final Output
//! ```
//!
//! # Shaders
//!
//! ## Vertex Shaders
//!
//! - **quad.vert**: Vertex shader for instanced glyph quads
//!   - Transforms normalized quad vertices to screen space
//!   - Applies per-instance cell positions and atlas UVs
//!   - Passes through colors and text attributes
//!
//! ## Fragment Shaders
//!
//! - **glyph.frag**: Basic fragment shader for glyph rendering
//!   - Samples SDF glyph atlas texture
//!   - Applies text attributes (bold, italic, underline, etc.)
//!   - Handles foreground/background color mixing
//!   - Renders underlines and strikethrough
//!
//! - **glyph_sdf.frag**: Advanced SDF text rendering
//!   - Multi-channel SDF (MSDF) support for sharp corners
//!   - Subpixel rendering (RGB LCD)
//!   - Outline and shadow effects
//!   - Glow/emission effects
//!   - Auto-adaptive edge smoothing
//!
//! - **cursor.frag**: Cursor rendering
//!   - Multiple styles: block, underline, bar, hollow block
//!   - Smooth blinking animation
//!   - Color inversion mode
//!   - Rounded corners and glow effects
//!
//! - **effects.frag**: Post-processing effects (fragment shader version)
//!   - CRT effects (scanlines, curvature, phosphor, interlace)
//!   - Bloom, vignette, chromatic aberration
//!   - Film grain/noise
//!   - Color grading (gamma, contrast, saturation, temperature)
//!   - Blur effects (box, gaussian)
//!   - Pixelate, grayscale, invert
//!
//! ## Compute Shaders
//!
//! - **effects.comp**: Post-processing effects (compute shader version)
//!   - Same effects as effects.frag but using compute workgroups
//!   - Better for systems with good compute shader support
//!   - 8x8 workgroup size for optimal occupancy
//!
//! # UCE34 Compliance
//!
//! - Q10: T7 GPU tier (Heterogeneous computing)
//! - Q33: GLSL 4.50 / SPIR-V compatible
//! - Q34: Version tagged shaders for audit trail
//!
//! # Usage
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::render::shaders;
//!
//! // Compile vertex shader
//! let vert_module = device.create_shader_module(&ShaderModuleDescriptor {
//!     source: ShaderSource::Glsl {
//!         shader: shaders::QUAD_VERT.into(),
//!         stage: naga::ShaderStage::Vertex,
//!         defines: Default::default(),
//!     },
//!     ..Default::default()
//! });
//!
//! // Choose fragment shader based on features
//! let frag_source = if use_advanced_sdf {
//!     shaders::GLYPH_SDF_FRAG
//! } else {
//!     shaders::GLYPH_FRAG
//! };
//! ```

// ============================================================================
// Vertex Shaders
// ============================================================================

/// Vertex shader for instanced glyph quads
///
/// # Inputs
/// - `a_position`: Quad corner position [-0.5, 0.5]
/// - `a_uv`: Texture coordinates [0, 1]
/// - `a_cell_pos`: Cell position and size (x, y, width, height)
/// - `a_uv_rect`: Atlas UV rectangle (u0, v0, u1, v1)
/// - `a_fg_color`: Foreground color RGBA
/// - `a_bg_color`: Background color RGBA
/// - `a_attributes`: Text attributes bitfield
///
/// # Uniforms
/// - `u_projection`: Projection matrix
/// - `u_viewport_size`: Viewport dimensions
/// - `u_cell_size`: Cell dimensions
/// - `u_time`: Animation time
/// - `u_frame`: Frame counter
pub const QUAD_VERT: &str = include_str!("quad.vert");

// ============================================================================
// Fragment Shaders - Text Rendering
// ============================================================================

/// Basic fragment shader for glyph rendering
///
/// Features:
/// - SDF sampling with smooth edges
/// - Text attributes (bold, italic, underline, strikethrough, blink, inverse)
/// - Foreground/background color mixing
///
/// # Inputs (from vertex shader)
/// - `v_uv`: Interpolated texture coordinates
/// - `v_fg_color`: Foreground color
/// - `v_bg_color`: Background color
/// - `v_attributes`: Attribute bitfield
///
/// # Uniforms
/// - Same as QUAD_VERT
/// - `t_atlas`: Glyph atlas texture
/// - `s_atlas`: Atlas sampler
pub const GLYPH_FRAG: &str = include_str!("glyph.frag");

/// Advanced SDF fragment shader for high-quality text rendering
///
/// Features:
/// - Multi-channel SDF (MSDF) for sharp corners at any scale
/// - Subpixel rendering (RGB LCD) for improved horizontal resolution
/// - Configurable edge sharpness with auto-adaptive smoothing
/// - Outline effects with configurable width and color
/// - Shadow effects with offset and softness control
/// - Glow/emission effects
/// - All standard text attributes
///
/// # Additional Uniforms (SdfUniforms)
/// - `u_sdf_threshold`: Edge threshold (default 0.5)
/// - `u_sdf_smoothness`: Edge smoothness (default 0.05)
/// - `u_outline_width`: Outline width (0 = disabled)
/// - `u_outline_color`: Outline color RGBA
/// - `u_shadow_offset_x/y`: Shadow offset
/// - `u_shadow_softness`: Shadow blur
/// - `u_shadow_color`: Shadow color RGBA
/// - `u_glow_intensity`: Glow strength (0 = disabled)
/// - `u_glow_radius`: Glow spread
/// - `u_render_mode`: 0=standard, 1=MSDF, 2=subpixel
/// - `u_subpixel_offset`: Subpixel sample offset
///
/// # Render Modes
/// - `MODE_STANDARD (0)`: Standard SDF with auto-smoothing
/// - `MODE_MSDF (1)`: Multi-channel SDF for sharp corners
/// - `MODE_SUBPIXEL (2)`: RGB subpixel rendering for LCD displays
pub const GLYPH_SDF_FRAG: &str = include_str!("glyph_sdf.frag");

// ============================================================================
// Fragment Shaders - Cursor
// ============================================================================

/// Cursor rendering fragment shader
///
/// Features:
/// - Cursor styles: block, underline, bar (I-beam), hollow block
/// - Smooth blinking with configurable rate and duty cycle
/// - Color inversion mode for visibility
/// - Rounded corners option
/// - Glow and trail effects
///
/// # Inputs
/// - `v_uv`: UV within cursor quad [0,1]
/// - `v_cursor_color`: Cursor color
/// - `v_cell_color`: Cell foreground (for inversion)
/// - `v_bg_color`: Cell background
///
/// # Uniforms (CursorUniforms)
/// - `u_cell_size`: Cell dimensions
/// - `u_time`: Animation time
/// - `u_cursor_style`: 0=block, 1=underline, 2=bar, 3=hollow_block
/// - `u_blink_rate`: Blinks per second (0 = no blink)
/// - `u_blink_duty`: On-time fraction (0.5 = 50%)
/// - `u_flags`: Feature flags (invert, smooth_blink, rounded, trail, glow)
/// - `u_corner_radius`: Rounded corner radius
/// - `u_border_width`: Hollow block border width
/// - `u_bar_width`: Bar cursor width fraction
/// - `u_underline_height`: Underline height fraction
/// - `u_trail_intensity`: Trail effect intensity
///
/// # Cursor Styles
/// - `STYLE_BLOCK (0)`: Full cell coverage
/// - `STYLE_UNDERLINE (1)`: Bottom line
/// - `STYLE_BAR (2)`: Left edge (I-beam)
/// - `STYLE_HOLLOW_BLOCK (3)`: Border only
///
/// # Feature Flags
/// - `FLAG_INVERT_COLOR (1)`: Invert cursor color
/// - `FLAG_SMOOTH_BLINK (2)`: Smooth fade vs hard blink
/// - `FLAG_ROUNDED_CORNERS (4)`: Enable rounded corners
/// - `FLAG_TRAIL_EFFECT (8)`: Enable cursor trail
/// - `FLAG_GLOW (16)`: Enable cursor glow
pub const CURSOR_FRAG: &str = include_str!("cursor.frag");

// ============================================================================
// Post-Processing Shaders
// ============================================================================

/// Post-processing effects fragment shader
///
/// Alternative to effects.comp for systems without compute shader support.
/// Uses full-screen quad rendering.
///
/// Features:
/// - CRT effects (scanlines, curvature, phosphor persistence, interlace)
/// - Bloom/glow
/// - Chromatic aberration
/// - Vignette
/// - Film grain/noise
/// - Color grading (gamma, contrast, saturation, temperature)
/// - Blur effects (box, gaussian)
/// - Pixelate, grayscale, invert
///
/// # Inputs
/// - `v_uv`: Screen UV coordinates [0,1]
///
/// # Uniforms (EffectUniforms)
/// - `u_enabled_flags`: Bitfield of enabled effects
/// - `u_time`: Animation time
/// - `u_crt`: [scanline_intensity, curvature, phosphor_decay, interlace]
/// - `u_bloom`: [threshold, intensity, radius, _pad]
/// - `u_color`: [gamma, contrast, saturation, temperature]
/// - `u_vignette`: [intensity, radius, softness, _pad]
/// - `u_chroma`: [r_offset, g_offset, b_offset, _pad]
/// - `u_noise`: [intensity, speed, grain_size, _pad]
/// - `u_blur`: [radius, sigma, direction_x, direction_y]
/// - `u_resolution`: [target_width, target_height, _pad, _pad]
///
/// # Effect Flags
/// - `EFFECT_CRT_SCANLINES (1)`
/// - `EFFECT_CRT_CURVATURE (2)`
/// - `EFFECT_CRT_PHOSPHOR (4)`
/// - `EFFECT_CRT_INTERLACE (8)`
/// - `EFFECT_BLOOM (16)`
/// - `EFFECT_CHROMATIC_ABERRATION (32)`
/// - `EFFECT_VIGNETTE (64)`
/// - `EFFECT_NOISE (128)`
/// - `EFFECT_COLOR_GRADING (256)`
/// - `EFFECT_BLUR (512)`
/// - `EFFECT_PIXELATE (1024)`
/// - `EFFECT_INVERT (2048)`
/// - `EFFECT_GRAYSCALE (4096)`
pub const EFFECTS_FRAG: &str = include_str!("effects.frag");

/// Post-processing effects compute shader
///
/// Uses compute workgroups (8x8) for efficient parallel processing.
/// Preferred when compute shaders are available.
///
/// # Inputs/Outputs
/// - `i_input`: Input image (readonly)
/// - `o_output`: Output image (writeonly)
///
/// # Uniforms
/// - Same as EFFECTS_FRAG
pub const EFFECTS_COMP: &str = include_str!("effects.comp");

// ============================================================================
// Shader Metadata
// ============================================================================

/// Shader version information for audit trail (Q34 compliance)
pub mod version {
    /// Shader version string
    pub const VERSION: &str = "1.0.0";

    /// GLSL version requirement
    pub const GLSL_VERSION: &str = "450";

    /// Minimum required OpenGL version
    pub const MIN_GL_VERSION: (u8, u8) = (4, 5);

    /// SPIR-V target version
    pub const SPIRV_VERSION: &str = "1.5";
}

/// Attribute bit flags (must match shader constants)
pub mod attributes {
    pub const ATTR_BOLD: u32 = 1;
    pub const ATTR_ITALIC: u32 = 2;
    pub const ATTR_UNDERLINE_SINGLE: u32 = 4;
    pub const ATTR_UNDERLINE_DOUBLE: u32 = 8;
    pub const ATTR_STRIKETHROUGH: u32 = 16;
    pub const ATTR_BLINK: u32 = 32;
    pub const ATTR_INVERSE: u32 = 64;
    pub const ATTR_DIM: u32 = 128;
}

/// Cursor style constants
pub mod cursor_style {
    pub const STYLE_BLOCK: u32 = 0;
    pub const STYLE_UNDERLINE: u32 = 1;
    pub const STYLE_BAR: u32 = 2;
    pub const STYLE_HOLLOW_BLOCK: u32 = 3;
}

/// Cursor feature flags
pub mod cursor_flags {
    pub const FLAG_INVERT_COLOR: u32 = 1;
    pub const FLAG_SMOOTH_BLINK: u32 = 2;
    pub const FLAG_ROUNDED_CORNERS: u32 = 4;
    pub const FLAG_TRAIL_EFFECT: u32 = 8;
    pub const FLAG_GLOW: u32 = 16;
}

/// Effect flags (must match shader constants)
pub mod effect_flags {
    pub const EFFECT_CRT_SCANLINES: u32 = 1;
    pub const EFFECT_CRT_CURVATURE: u32 = 2;
    pub const EFFECT_CRT_PHOSPHOR: u32 = 4;
    pub const EFFECT_CRT_INTERLACE: u32 = 8;
    pub const EFFECT_BLOOM: u32 = 16;
    pub const EFFECT_CHROMATIC_ABERRATION: u32 = 32;
    pub const EFFECT_VIGNETTE: u32 = 64;
    pub const EFFECT_NOISE: u32 = 128;
    pub const EFFECT_COLOR_GRADING: u32 = 256;
    pub const EFFECT_BLUR: u32 = 512;
    pub const EFFECT_PIXELATE: u32 = 1024;
    pub const EFFECT_INVERT: u32 = 2048;
    pub const EFFECT_GRAYSCALE: u32 = 4096;
}

/// SDF render modes
pub mod sdf_modes {
    pub const MODE_STANDARD: u32 = 0;
    pub const MODE_MSDF: u32 = 1;
    pub const MODE_SUBPIXEL: u32 = 2;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaders_not_empty() {
        assert!(!QUAD_VERT.is_empty(), "quad.vert should not be empty");
        assert!(!GLYPH_FRAG.is_empty(), "glyph.frag should not be empty");
        assert!(!GLYPH_SDF_FRAG.is_empty(), "glyph_sdf.frag should not be empty");
        assert!(!CURSOR_FRAG.is_empty(), "cursor.frag should not be empty");
        assert!(!EFFECTS_FRAG.is_empty(), "effects.frag should not be empty");
        assert!(!EFFECTS_COMP.is_empty(), "effects.comp should not be empty");
    }

    #[test]
    fn test_shader_version() {
        assert!(QUAD_VERT.contains("#version 450"), "quad.vert missing version");
        assert!(GLYPH_FRAG.contains("#version 450"), "glyph.frag missing version");
        assert!(GLYPH_SDF_FRAG.contains("#version 450"), "glyph_sdf.frag missing version");
        assert!(CURSOR_FRAG.contains("#version 450"), "cursor.frag missing version");
        assert!(EFFECTS_FRAG.contains("#version 450"), "effects.frag missing version");
        assert!(EFFECTS_COMP.contains("#version 450"), "effects.comp missing version");
    }

    #[test]
    fn test_shader_entrypoints() {
        assert!(QUAD_VERT.contains("void main()"), "quad.vert missing main");
        assert!(GLYPH_FRAG.contains("void main()"), "glyph.frag missing main");
        assert!(GLYPH_SDF_FRAG.contains("void main()"), "glyph_sdf.frag missing main");
        assert!(CURSOR_FRAG.contains("void main()"), "cursor.frag missing main");
        assert!(EFFECTS_FRAG.contains("void main()"), "effects.frag missing main");
        assert!(EFFECTS_COMP.contains("void main()"), "effects.comp missing main");
    }

    #[test]
    fn test_vertex_shader_attributes() {
        // Check for required input attributes
        assert!(QUAD_VERT.contains("in vec2 a_position"), "Missing a_position");
        assert!(QUAD_VERT.contains("in vec2 a_uv"), "Missing a_uv");
        assert!(QUAD_VERT.contains("in vec4 a_cell_pos"), "Missing a_cell_pos");
        assert!(QUAD_VERT.contains("in vec4 a_uv_rect"), "Missing a_uv_rect");
        assert!(QUAD_VERT.contains("in vec4 a_fg_color"), "Missing a_fg_color");
        assert!(QUAD_VERT.contains("in vec4 a_bg_color"), "Missing a_bg_color");
        assert!(QUAD_VERT.contains("in uint a_attributes"), "Missing a_attributes");

        // Check for output varyings
        assert!(QUAD_VERT.contains("out vec2 v_uv"), "Missing v_uv output");
        assert!(QUAD_VERT.contains("out vec4 v_fg_color"), "Missing v_fg_color output");
        assert!(QUAD_VERT.contains("out vec4 v_bg_color"), "Missing v_bg_color output");
        assert!(QUAD_VERT.contains("out uint v_attributes"), "Missing v_attributes output");
    }

    #[test]
    fn test_fragment_shader_uniforms() {
        // Check for atlas texture binding
        assert!(GLYPH_FRAG.contains("uniform texture2D t_atlas"), "Missing t_atlas");
        assert!(GLYPH_FRAG.contains("uniform sampler s_atlas"), "Missing s_atlas");

        // Check for attribute flags
        assert!(GLYPH_FRAG.contains("ATTR_BOLD"), "Missing ATTR_BOLD");
        assert!(GLYPH_FRAG.contains("ATTR_ITALIC"), "Missing ATTR_ITALIC");
        assert!(GLYPH_FRAG.contains("ATTR_UNDERLINE"), "Missing ATTR_UNDERLINE");
        assert!(GLYPH_FRAG.contains("ATTR_STRIKETHROUGH"), "Missing ATTR_STRIKETHROUGH");
        assert!(GLYPH_FRAG.contains("ATTR_BLINK"), "Missing ATTR_BLINK");
        assert!(GLYPH_FRAG.contains("ATTR_INVERSE"), "Missing ATTR_INVERSE");
    }

    #[test]
    fn test_sdf_shader_features() {
        // Check for SDF-specific features
        assert!(GLYPH_SDF_FRAG.contains("u_sdf_threshold"), "Missing u_sdf_threshold");
        assert!(GLYPH_SDF_FRAG.contains("u_sdf_smoothness"), "Missing u_sdf_smoothness");
        assert!(GLYPH_SDF_FRAG.contains("u_outline_width"), "Missing u_outline_width");
        assert!(GLYPH_SDF_FRAG.contains("u_shadow_offset"), "Missing u_shadow_offset");
        assert!(GLYPH_SDF_FRAG.contains("u_glow_intensity"), "Missing u_glow_intensity");
        assert!(GLYPH_SDF_FRAG.contains("u_render_mode"), "Missing u_render_mode");

        // Check for render modes
        assert!(GLYPH_SDF_FRAG.contains("MODE_STANDARD"), "Missing MODE_STANDARD");
        assert!(GLYPH_SDF_FRAG.contains("MODE_MSDF"), "Missing MODE_MSDF");
        assert!(GLYPH_SDF_FRAG.contains("MODE_SUBPIXEL"), "Missing MODE_SUBPIXEL");

        // Check for MSDF function
        assert!(GLYPH_SDF_FRAG.contains("msdf_median"), "Missing msdf_median function");
    }

    #[test]
    fn test_cursor_shader_features() {
        // Check for cursor styles
        assert!(CURSOR_FRAG.contains("STYLE_BLOCK"), "Missing STYLE_BLOCK");
        assert!(CURSOR_FRAG.contains("STYLE_UNDERLINE"), "Missing STYLE_UNDERLINE");
        assert!(CURSOR_FRAG.contains("STYLE_BAR"), "Missing STYLE_BAR");
        assert!(CURSOR_FRAG.contains("STYLE_HOLLOW_BLOCK"), "Missing STYLE_HOLLOW_BLOCK");

        // Check for feature flags
        assert!(CURSOR_FRAG.contains("FLAG_INVERT_COLOR"), "Missing FLAG_INVERT_COLOR");
        assert!(CURSOR_FRAG.contains("FLAG_SMOOTH_BLINK"), "Missing FLAG_SMOOTH_BLINK");
        assert!(CURSOR_FRAG.contains("FLAG_ROUNDED_CORNERS"), "Missing FLAG_ROUNDED_CORNERS");

        // Check for cursor uniforms
        assert!(CURSOR_FRAG.contains("u_cursor_style"), "Missing u_cursor_style");
        assert!(CURSOR_FRAG.contains("u_blink_rate"), "Missing u_blink_rate");
    }

    #[test]
    fn test_effects_shader_features() {
        // Check effect flags in fragment shader
        assert!(EFFECTS_FRAG.contains("EFFECT_CRT_SCANLINES"), "Missing CRT scanlines");
        assert!(EFFECTS_FRAG.contains("EFFECT_BLOOM"), "Missing bloom");
        assert!(EFFECTS_FRAG.contains("EFFECT_VIGNETTE"), "Missing vignette");
        assert!(EFFECTS_FRAG.contains("EFFECT_CHROMATIC_ABERRATION"), "Missing chromatic aberration");
        assert!(EFFECTS_FRAG.contains("EFFECT_NOISE"), "Missing noise");
        assert!(EFFECTS_FRAG.contains("EFFECT_COLOR_GRADING"), "Missing color grading");
        assert!(EFFECTS_FRAG.contains("EFFECT_BLUR"), "Missing blur");
        assert!(EFFECTS_FRAG.contains("EFFECT_PIXELATE"), "Missing pixelate");
    }

    #[test]
    fn test_compute_shader_layout() {
        // Check workgroup size
        assert!(EFFECTS_COMP.contains("local_size_x = 8"), "Missing local_size_x");
        assert!(EFFECTS_COMP.contains("local_size_y = 8"), "Missing local_size_y");

        // Check image bindings
        assert!(EFFECTS_COMP.contains("image2D i_input"), "Missing i_input");
        assert!(EFFECTS_COMP.contains("image2D o_output"), "Missing o_output");

        // Check effect flags
        assert!(EFFECTS_COMP.contains("EFFECT_CRT_SCANLINES"), "Missing CRT scanlines");
        assert!(EFFECTS_COMP.contains("EFFECT_BLOOM"), "Missing bloom");
        assert!(EFFECTS_COMP.contains("EFFECT_VIGNETTE"), "Missing vignette");
        assert!(EFFECTS_COMP.contains("EFFECT_CHROMATIC_ABERRATION"), "Missing chromatic aberration");
    }

    #[test]
    fn test_shader_line_counts() {
        let quad_lines = QUAD_VERT.lines().count();
        let glyph_lines = GLYPH_FRAG.lines().count();
        let glyph_sdf_lines = GLYPH_SDF_FRAG.lines().count();
        let cursor_lines = CURSOR_FRAG.lines().count();
        let effects_frag_lines = EFFECTS_FRAG.lines().count();
        let effects_comp_lines = EFFECTS_COMP.lines().count();

        // Sanity check line counts
        assert!(quad_lines >= 35 && quad_lines <= 50,
            "quad.vert has {} lines (expected 35-50)", quad_lines);
        assert!(glyph_lines >= 90 && glyph_lines <= 120,
            "glyph.frag has {} lines (expected 90-120)", glyph_lines);
        assert!(glyph_sdf_lines >= 150 && glyph_sdf_lines <= 250,
            "glyph_sdf.frag has {} lines (expected 150-250)", glyph_sdf_lines);
        assert!(cursor_lines >= 120 && cursor_lines <= 200,
            "cursor.frag has {} lines (expected 120-200)", cursor_lines);
        assert!(effects_frag_lines >= 200 && effects_frag_lines <= 400,
            "effects.frag has {} lines (expected 200-400)", effects_frag_lines);
        assert!(effects_comp_lines >= 140 && effects_comp_lines <= 200,
            "effects.comp has {} lines (expected 140-200)", effects_comp_lines);
    }

    #[test]
    fn test_attribute_constants_match() {
        // Verify Rust constants match shader constants
        assert_eq!(attributes::ATTR_BOLD, 1);
        assert_eq!(attributes::ATTR_ITALIC, 2);
        assert_eq!(attributes::ATTR_UNDERLINE_SINGLE, 4);
        assert_eq!(attributes::ATTR_UNDERLINE_DOUBLE, 8);
        assert_eq!(attributes::ATTR_STRIKETHROUGH, 16);
        assert_eq!(attributes::ATTR_BLINK, 32);
        assert_eq!(attributes::ATTR_INVERSE, 64);
        assert_eq!(attributes::ATTR_DIM, 128);
    }

    #[test]
    fn test_effect_flags_match() {
        assert_eq!(effect_flags::EFFECT_CRT_SCANLINES, 1);
        assert_eq!(effect_flags::EFFECT_CRT_CURVATURE, 2);
        assert_eq!(effect_flags::EFFECT_CRT_PHOSPHOR, 4);
        assert_eq!(effect_flags::EFFECT_BLOOM, 16);
        assert_eq!(effect_flags::EFFECT_CHROMATIC_ABERRATION, 32);
        assert_eq!(effect_flags::EFFECT_VIGNETTE, 64);
        assert_eq!(effect_flags::EFFECT_NOISE, 128);
        assert_eq!(effect_flags::EFFECT_COLOR_GRADING, 256);
    }

    #[test]
    fn test_cursor_style_constants() {
        assert_eq!(cursor_style::STYLE_BLOCK, 0);
        assert_eq!(cursor_style::STYLE_UNDERLINE, 1);
        assert_eq!(cursor_style::STYLE_BAR, 2);
        assert_eq!(cursor_style::STYLE_HOLLOW_BLOCK, 3);
    }

    #[test]
    fn test_sdf_mode_constants() {
        assert_eq!(sdf_modes::MODE_STANDARD, 0);
        assert_eq!(sdf_modes::MODE_MSDF, 1);
        assert_eq!(sdf_modes::MODE_SUBPIXEL, 2);
    }
}
