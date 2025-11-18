//! Spring physics animation system (zero external dependencies)
//!
//! Custom implementation using damped harmonic oscillator physics.
//! Provides smooth, natural-feeling animations with configurable spring parameters.

use std::time::{Duration, Instant};

/// Spring physics animator using damped harmonic oscillator
#[derive(Debug, Clone)]
pub struct SpringAnimation {
    /// Current value
    pub value: f32,

    /// Target value
    target: f32,

    /// Current velocity
    velocity: f32,

    /// Spring stiffness (higher = snappier)
    stiffness: f32,

    /// Damping coefficient (higher = less oscillation)
    damping: f32,

    /// Mass (higher = slower acceleration)
    mass: f32,

    /// Last update time
    last_update: Option<Instant>,

    /// Whether the animation is currently active
    is_animating: bool,
}

impl SpringAnimation {
    /// Create a new spring animation with custom parameters
    ///
    /// # Parameters
    /// - `initial_value`: Starting value
    /// - `mass`: Mass of the spring (typically 1.0)
    /// - `stiffness`: Spring stiffness (80-200 for UI animations)
    /// - `damping`: Damping coefficient (10-20 for smooth motion)
    pub fn new(initial_value: f32, mass: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial_value,
            target: initial_value,
            velocity: 0.0,
            stiffness,
            damping,
            mass,
            last_update: None,
            is_animating: false,
        }
    }

    /// Create a stiff spring (fast, snappy response)
    pub fn stiff(initial_value: f32) -> Self {
        Self::new(initial_value, 1.0, 120.0, 12.0)
    }

    /// Create a soft spring (slow, gentle response)
    pub fn soft(initial_value: f32) -> Self {
        Self::new(initial_value, 1.0, 80.0, 15.0)
    }

    /// Set the target value (starts animation)
    pub fn set_target(&mut self, target: f32) {
        if (self.target - target).abs() > 0.001 {
            self.target = target;
            self.is_animating = true;
            self.last_update = Some(Instant::now());
        }
    }

    /// Update the animation (call from AnimationTick)
    pub fn update(&mut self) {
        if !self.is_animating {
            return;
        }

        let now = Instant::now();
        let dt = if let Some(last) = self.last_update {
            (now - last).as_secs_f32()
        } else {
            1.0 / 60.0 // Default to 60 FPS
        };
        self.last_update = Some(now);

        // Clamp dt to prevent instability
        let dt = dt.min(1.0 / 30.0);

        // Spring physics (damped harmonic oscillator)
        // F = -k*x - c*v (Hooke's law + damping)
        let displacement = self.value - self.target;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        // Semi-implicit Euler integration (more stable than Euler)
        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;

        // Stop animating if nearly at rest (optimization)
        let at_rest = displacement.abs() < 0.001 && self.velocity.abs() < 0.01;
        if at_rest {
            self.value = self.target;
            self.velocity = 0.0;
            self.is_animating = false;
        }
    }

    /// Check if the animation is currently running
    pub fn is_animating(&self) -> bool {
        self.is_animating
    }

    /// Get the current value
    pub fn current_value(&self) -> f32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_converges() {
        let mut spring = SpringAnimation::stiff(0.0);
        spring.set_target(1.0);

        // Simulate 1 second at 60 FPS
        for _ in 0..60 {
            spring.update();
            std::thread::sleep(Duration::from_millis(16));
        }

        // Should be close to target after 1 second
        assert!((spring.current_value() - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_spring_stops_at_target() {
        let mut spring = SpringAnimation::stiff(0.0);
        spring.set_target(1.0);

        // Simulate until animation stops
        let mut iterations = 0;
        while spring.is_animating() && iterations < 1000 {
            spring.update();
            std::thread::sleep(Duration::from_millis(1));
            iterations += 1;
        }

        // Should stop exactly at target
        assert_eq!(spring.current_value(), 1.0);
        assert!(!spring.is_animating());
    }
}
