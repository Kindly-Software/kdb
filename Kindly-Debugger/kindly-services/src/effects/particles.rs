//! Floating Particle System
//!
//! GPU-accelerated particle effect with subtle gold sparkles.

/// Particle data for animation
#[derive(Clone)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub opacity: f32,
    pub hue: f32, // 0 = purple, 1 = gold
}

/// Particle system for floating effect
pub struct ParticleSystem {
    particles: Vec<Particle>,
    width: f32,
    height: f32,
}

impl ParticleSystem {
    /// Create a new particle system
    pub fn new(count: usize, width: f32, height: f32) -> Self {
        let mut particles = Vec::with_capacity(count);

        for _ in 0..count {
            particles.push(Self::create_particle(width, height));
        }

        Self {
            particles,
            width,
            height,
        }
    }

    fn create_particle(width: f32, height: f32) -> Particle {
        // Use simple pseudo-random (js_sys::Math::random in WASM)
        let random = || js_sys::Math::random() as f32;

        Particle {
            x: random() * width,
            y: random() * height,
            vx: (random() - 0.5) * 0.5,
            vy: -random() * 0.3 - 0.1, // Float upward
            size: random() * 3.0 + 1.0,
            opacity: random() * 0.5 + 0.1,
            hue: if random() > 0.85 { 1.0 } else { 0.0 }, // 15% gold
        }
    }

    /// Update particle positions
    pub fn update(&mut self, _dt: f32) {
        for p in &mut self.particles {
            p.x += p.vx;
            p.y += p.vy;

            // Wrap around
            if p.y < -10.0 {
                p.y = self.height + 10.0;
                p.x = js_sys::Math::random() as f32 * self.width;
            }
            if p.x < -10.0 {
                p.x = self.width + 10.0;
            }
            if p.x > self.width + 10.0 {
                p.x = -10.0;
            }

            // Subtle oscillation
            p.opacity = (p.opacity + 0.002).min(0.6);
            if js_sys::Math::random() < 0.001 {
                p.opacity = js_sys::Math::random() as f32 * 0.3 + 0.1;
            }
        }
    }

    /// Resize the particle system
    pub fn resize(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    /// Get particles for rendering
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Generate CSS for particle divs (alternative to canvas)
    pub fn to_css_particles(&self) -> String {
        let mut css = String::new();

        for (i, p) in self.particles.iter().enumerate() {
            let color = if p.hue > 0.5 {
                format!("rgba(255, 215, 0, {})", p.opacity) // Gold
            } else {
                format!("rgba(155, 89, 182, {})", p.opacity) // Purple
            };

            css.push_str(&format!(
                ".particle-{} {{ \
                    position: fixed; \
                    left: {}px; \
                    top: {}px; \
                    width: {}px; \
                    height: {}px; \
                    background: {}; \
                    border-radius: 50%; \
                    pointer-events: none; \
                    animation: float-{} {}s ease-in-out infinite; \
                }}\n",
                i, p.x, p.y, p.size, p.size, color, i, 3.0 + (i as f32 % 5.0)
            ));
        }

        css
    }
}

/// Generate keyframe animation CSS
pub fn generate_particle_keyframes(count: usize) -> String {
    let mut css = String::new();

    for i in 0..count {
        let offset_x = (i as f32 * 17.0 % 20.0) - 10.0;
        let offset_y = -(i as f32 * 13.0 % 50.0) - 20.0;

        css.push_str(&format!(
            "@keyframes float-{} {{ \
                0%, 100% {{ transform: translate(0, 0); opacity: {}; }} \
                50% {{ transform: translate({}px, {}px); opacity: {}; }} \
            }}\n",
            i,
            0.3 + (i as f32 % 3.0) * 0.1,
            offset_x,
            offset_y,
            0.5 + (i as f32 % 2.0) * 0.1
        ));
    }

    css
}
