#version 450

// Post-Processing Effects Fragment Shader
// UCE34 Tier: T7 (GPU acceleration)
//
// This is a fragment shader alternative to effects.comp for systems
// without compute shader support or for full-screen quad rendering.
//
// Features:
// - CRT effects (scanlines, curvature, phosphor persistence)
// - Bloom/glow
// - Chromatic aberration
// - Vignette
// - Film grain/noise
// - Color grading (gamma, contrast, saturation, temperature)
// - Blur effects (box, gaussian)
// - Retro pixel effects

// Input from vertex shader (full-screen quad)
layout(location = 0) in vec2 v_uv;  // [0,1] screen coordinates

// Input texture (terminal render output)
layout(set = 0, binding = 0) uniform texture2D t_input;
layout(set = 0, binding = 1) uniform sampler s_input;

// Effect uniforms
layout(set = 0, binding = 2) uniform EffectUniforms {
    uint u_enabled_flags;       // Bitfield of enabled effects
    float u_time;               // Animation time
    uint u_frame;               // Frame counter
    uint _pad0;

    // CRT effects [scanline_intensity, curvature, phosphor_decay, interlace]
    vec4 u_crt;

    // Bloom [threshold, intensity, radius, _pad]
    vec4 u_bloom;

    // Color grading [gamma, contrast, saturation, temperature]
    vec4 u_color;

    // Vignette [intensity, radius, softness, _pad]
    vec4 u_vignette;

    // Chromatic aberration [r_offset, g_offset, b_offset, _pad]
    vec4 u_chroma;

    // Noise [intensity, speed, grain_size, _pad]
    vec4 u_noise;

    // Blur [radius, sigma, direction_x, direction_y]
    vec4 u_blur;

    // Resolution for pixel effects [target_width, target_height, _pad, _pad]
    vec4 u_resolution;
};

// Output
layout(location = 0) out vec4 o_color;

// Effect flags (must match Rust EffectFlags)
const uint EFFECT_CRT_SCANLINES = 1u;
const uint EFFECT_CRT_CURVATURE = 2u;
const uint EFFECT_CRT_PHOSPHOR = 4u;
const uint EFFECT_CRT_INTERLACE = 8u;
const uint EFFECT_BLOOM = 16u;
const uint EFFECT_CHROMATIC_ABERRATION = 32u;
const uint EFFECT_VIGNETTE = 64u;
const uint EFFECT_NOISE = 128u;
const uint EFFECT_COLOR_GRADING = 256u;
const uint EFFECT_BLUR = 512u;
const uint EFFECT_PIXELATE = 1024u;
const uint EFFECT_INVERT = 2048u;
const uint EFFECT_GRAYSCALE = 4096u;

// ============================================================================
// Utility Functions
// ============================================================================

// High-quality hash for noise generation
float hash(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Improved noise with time variation
float noise(vec2 uv, float time, float speed) {
    vec2 p = uv * 100.0 + time * speed;
    return hash(p) * 2.0 - 1.0;
}

// Perlin-like smooth noise
float smooth_noise(vec2 uv, float time) {
    vec2 i = floor(uv);
    vec2 f = fract(uv);
    f = f * f * (3.0 - 2.0 * f); // Smoothstep

    float a = hash(i + vec2(0.0, 0.0) + time);
    float b = hash(i + vec2(1.0, 0.0) + time);
    float c = hash(i + vec2(0.0, 1.0) + time);
    float d = hash(i + vec2(1.0, 1.0) + time);

    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// ============================================================================
// CRT Effects
// ============================================================================

// CRT scanlines effect
vec3 apply_scanlines(vec3 color, vec2 uv, float intensity, float time) {
    // Horizontal scanlines
    float scanline = sin(uv.y * 3.14159 * u_resolution.y) * 0.5 + 0.5;
    scanline = pow(scanline, 0.5); // Soften

    // Optional interlace effect (alternating lines per frame)
    if ((u_enabled_flags & EFFECT_CRT_INTERLACE) != 0u) {
        float line = floor(uv.y * u_resolution.y);
        float interlace = mod(line + float(u_frame), 2.0);
        scanline *= mix(0.8, 1.0, interlace);
    }

    return color * mix(1.0, scanline, intensity);
}

// CRT curvature distortion
vec2 apply_curvature(vec2 uv, float amount) {
    // Convert to centered coordinates
    vec2 centered = uv * 2.0 - 1.0;

    // Apply barrel distortion
    vec2 offset = centered * vec2(
        1.0 + (centered.y * centered.y) * amount,
        1.0 + (centered.x * centered.x) * amount
    );

    // Convert back
    return offset * 0.5 + 0.5;
}

// Phosphor persistence (RGB subpixel glow)
vec3 apply_phosphor(vec3 color, float decay) {
    // Simulate different phosphor decay rates for RGB
    vec3 decay_rates = vec3(0.98, 0.95, 0.92); // R decays slowest
    return color * mix(vec3(1.0), decay_rates, decay);
}

// ============================================================================
// Color Effects
// ============================================================================

// Vignette effect
vec3 apply_vignette(vec3 color, vec2 uv, float intensity, float radius, float softness) {
    vec2 center = uv - 0.5;
    float dist = length(center);
    float vignette = smoothstep(radius, radius - softness, dist);
    return color * mix(1.0, vignette, intensity);
}

// Bloom effect (simple approximation)
vec3 apply_bloom(vec3 color, float threshold, float intensity) {
    // Extract bright parts
    vec3 bright = max(color - threshold, vec3(0.0));
    float luminance = dot(bright, vec3(0.299, 0.587, 0.114));

    // Add bloom glow
    return color + bright * intensity * luminance;
}

// Chromatic aberration
vec3 apply_chromatic_aberration(vec2 uv, vec3 offsets) {
    vec2 center = uv - 0.5;
    float dist = length(center) * 2.0;

    // Offset each channel differently from center
    vec2 r_uv = uv + center * offsets.r * dist;
    vec2 g_uv = uv + center * offsets.g * dist;
    vec2 b_uv = uv + center * offsets.b * dist;

    // Clamp to valid UV range
    r_uv = clamp(r_uv, 0.0, 1.0);
    g_uv = clamp(g_uv, 0.0, 1.0);
    b_uv = clamp(b_uv, 0.0, 1.0);

    vec3 color;
    color.r = texture(sampler2D(t_input, s_input), r_uv).r;
    color.g = texture(sampler2D(t_input, s_input), g_uv).g;
    color.b = texture(sampler2D(t_input, s_input), b_uv).b;

    return color;
}

// Color grading (gamma, contrast, saturation, temperature)
vec3 apply_color_grading(vec3 color, vec4 params) {
    float gamma = params.x;
    float contrast = params.y;
    float saturation = params.z;
    float temperature = params.w;

    // Gamma correction
    color = pow(color, vec3(1.0 / gamma));

    // Contrast adjustment
    color = (color - 0.5) * contrast + 0.5;

    // Saturation adjustment
    float luma = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(luma), color, saturation);

    // Color temperature (warm = positive, cool = negative)
    if (temperature != 0.0) {
        color.r += temperature * 0.1;
        color.b -= temperature * 0.1;
    }

    return clamp(color, 0.0, 1.0);
}

// ============================================================================
// Blur Effects
// ============================================================================

// Box blur
vec3 apply_box_blur(vec2 uv, float radius) {
    vec3 color = vec3(0.0);
    float count = 0.0;

    vec2 texel_size = 1.0 / u_resolution.xy;
    int r = int(radius);

    for (int x = -r; x <= r; x++) {
        for (int y = -r; y <= r; y++) {
            vec2 offset = vec2(float(x), float(y)) * texel_size;
            color += texture(sampler2D(t_input, s_input), uv + offset).rgb;
            count += 1.0;
        }
    }

    return color / count;
}

// Gaussian blur (1D, call twice for 2D)
vec3 apply_gaussian_blur(vec2 uv, vec2 direction, float sigma) {
    vec3 color = vec3(0.0);
    float total_weight = 0.0;

    vec2 texel_size = 1.0 / u_resolution.xy;
    int radius = int(ceil(sigma * 3.0));

    for (int i = -radius; i <= radius; i++) {
        float x = float(i);
        float weight = exp(-(x * x) / (2.0 * sigma * sigma));

        vec2 offset = direction * x * texel_size;
        color += texture(sampler2D(t_input, s_input), uv + offset).rgb * weight;
        total_weight += weight;
    }

    return color / total_weight;
}

// ============================================================================
// Special Effects
// ============================================================================

// Pixelate effect (reduce effective resolution)
vec2 apply_pixelate(vec2 uv, vec2 target_res) {
    vec2 pixel = floor(uv * target_res) / target_res;
    return pixel + 0.5 / target_res; // Center of pixel
}

// Film grain noise
vec3 apply_noise(vec3 color, vec2 uv, float intensity, float time, float grain_size) {
    float n = noise(uv * grain_size, time, u_noise.y);
    return color + n * intensity;
}

// Grayscale conversion
vec3 apply_grayscale(vec3 color) {
    float luma = dot(color, vec3(0.299, 0.587, 0.114));
    return vec3(luma);
}

// Color inversion
vec3 apply_invert(vec3 color) {
    return vec3(1.0) - color;
}

// ============================================================================
// Main
// ============================================================================

void main() {
    vec2 uv = v_uv;
    vec4 color;

    // Apply CRT curvature first (affects sampling)
    if ((u_enabled_flags & EFFECT_CRT_CURVATURE) != 0u) {
        vec2 curved_uv = apply_curvature(uv, u_crt.y);

        // Check if outside screen bounds
        if (curved_uv.x < 0.0 || curved_uv.x > 1.0 ||
            curved_uv.y < 0.0 || curved_uv.y > 1.0) {
            o_color = vec4(0.0, 0.0, 0.0, 1.0);
            return;
        }

        uv = curved_uv;
    }

    // Apply pixelate (affects sampling)
    if ((u_enabled_flags & EFFECT_PIXELATE) != 0u) {
        uv = apply_pixelate(uv, u_resolution.xy);
    }

    // Sample base color (with optional chromatic aberration)
    if ((u_enabled_flags & EFFECT_CHROMATIC_ABERRATION) != 0u) {
        color.rgb = apply_chromatic_aberration(uv, u_chroma.rgb);
        color.a = texture(sampler2D(t_input, s_input), uv).a;
    } else {
        color = texture(sampler2D(t_input, s_input), uv);
    }

    // Apply blur
    if ((u_enabled_flags & EFFECT_BLUR) != 0u) {
        vec2 direction = normalize(u_blur.zw);
        if (length(direction) < 0.01) direction = vec2(1.0, 0.0); // Default horizontal

        if (u_blur.y > 0.0) {
            // Gaussian blur
            color.rgb = apply_gaussian_blur(uv, direction, u_blur.y);
        } else if (u_blur.x > 0.0) {
            // Box blur
            color.rgb = apply_box_blur(uv, u_blur.x);
        }
    }

    // Apply CRT scanlines
    if ((u_enabled_flags & EFFECT_CRT_SCANLINES) != 0u) {
        color.rgb = apply_scanlines(color.rgb, uv, u_crt.x, u_time);
    }

    // Apply phosphor persistence
    if ((u_enabled_flags & EFFECT_CRT_PHOSPHOR) != 0u) {
        color.rgb = apply_phosphor(color.rgb, u_crt.z);
    }

    // Apply bloom
    if ((u_enabled_flags & EFFECT_BLOOM) != 0u) {
        color.rgb = apply_bloom(color.rgb, u_bloom.x, u_bloom.y);
    }

    // Apply vignette
    if ((u_enabled_flags & EFFECT_VIGNETTE) != 0u) {
        color.rgb = apply_vignette(color.rgb, v_uv, u_vignette.x, u_vignette.y, u_vignette.z);
    }

    // Apply noise/film grain
    if ((u_enabled_flags & EFFECT_NOISE) != 0u) {
        color.rgb = apply_noise(color.rgb, uv, u_noise.x, u_time, u_noise.z);
    }

    // Apply color grading
    if ((u_enabled_flags & EFFECT_COLOR_GRADING) != 0u) {
        color.rgb = apply_color_grading(color.rgb, u_color);
    }

    // Apply grayscale
    if ((u_enabled_flags & EFFECT_GRAYSCALE) != 0u) {
        color.rgb = apply_grayscale(color.rgb);
    }

    // Apply inversion
    if ((u_enabled_flags & EFFECT_INVERT) != 0u) {
        color.rgb = apply_invert(color.rgb);
    }

    // Final clamp and output
    color.rgb = clamp(color.rgb, 0.0, 1.0);
    o_color = color;
}
