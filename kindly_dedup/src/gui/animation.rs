//! Simple animation system for GUI micro-interactions

use std::time::{Duration, Instant};

/// Simple spring-based animation for f32 values
#[derive(Debug, Clone)]
pub struct Animation {
    from: f32,
    to: f32,
    current: f32,
    velocity: f32,
    mass: f32,
    stiffness: f32,
    damping: f32,
    start_time: Option<Instant>,
    delay: Duration,
    complete: bool,
}

impl Animation {
    /// Create a new animation (not started)
    pub fn new(initial_value: f32) -> Self {
        Self {
            from: initial_value,
            to: initial_value,
            current: initial_value,
            velocity: 0.0,
            mass: 1.0,
            stiffness: 100.0,
            damping: 10.0,
            start_time: None,
            delay: Duration::ZERO,
            complete: true,
        }
    }

    /// Get current animated value
    pub fn value(&self) -> f32 {
        self.current
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Update animation (call every frame)
    pub fn update(&mut self, dt: f32) {
        if self.complete {
            return;
        }

        // Check if still in delay period
        if let Some(start) = self.start_time {
            if start.elapsed() < self.delay {
                return;
            }
        }

        // Spring physics: F = -k(x - x0) - c*v
        let displacement = self.current - self.to;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        // Euler integration
        self.velocity += acceleration * dt;
        self.current += self.velocity * dt;

        // Check if settled (within 1% of target, low velocity)
        let error = (self.current - self.to).abs();
        let vel_threshold = 0.01;
        if error < 0.01 && self.velocity.abs() < vel_threshold {
            self.current = self.to;
            self.velocity = 0.0;
            self.complete = true;
        }
    }
}

/// Builder for animations
pub struct AnimationBuilder {
    from: f32,
    to: f32,
    mass: f32,
    stiffness: f32,
    damping: f32,
    delay: Duration,
}

impl AnimationBuilder {
    /// Create new animation builder
    pub fn new() -> Self {
        Self {
            from: 0.0,
            to: 1.0,
            mass: 1.0,
            stiffness: 100.0,
            damping: 10.0,
            delay: Duration::ZERO,
        }
    }

    /// Set start value
    pub fn from(mut self, value: f32) -> Self {
        self.from = value;
        self
    }

    /// Set end value
    pub fn to(mut self, value: f32) -> Self {
        self.to = value;
        self
    }

    /// Set spring parameters (mass, stiffness, damping)
    pub fn spring_params(mut self, mass: f32, stiffness: f32, damping: f32) -> Self {
        self.mass = mass;
        self.stiffness = stiffness;
        self.damping = damping;
        self
    }

    /// Set delay before animation starts
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Start the animation
    pub fn begin(self) -> Animation {
        Animation {
            from: self.from,
            to: self.to,
            current: self.from,
            velocity: 0.0,
            mass: self.mass,
            stiffness: self.stiffness,
            damping: self.damping,
            start_time: Some(Instant::now()),
            delay: self.delay,
            complete: false,
        }
    }
}

impl Default for AnimationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create 5 card entrance animations with staggered delays
/// (Header, File Input, Settings, Action Button, Feature Badges)
pub fn create_card_animations() -> Vec<Animation> {
    vec![
        AnimationBuilder::new()
            .from(0.0)
            .to(1.0)
            .delay(Duration::from_millis(0)) // Header (instant)
            .spring_params(1.0, 60.0, 20.0) // Softer spring
            .begin(),
        AnimationBuilder::new()
            .from(0.0)
            .to(1.0)
            .delay(Duration::from_millis(100)) // File input
            .spring_params(1.0, 60.0, 20.0)
            .begin(),
        AnimationBuilder::new()
            .from(0.0)
            .to(1.0)
            .delay(Duration::from_millis(200)) // Settings
            .spring_params(1.0, 60.0, 20.0)
            .begin(),
        AnimationBuilder::new()
            .from(0.0)
            .to(1.0)
            .delay(Duration::from_millis(300)) // Action button
            .spring_params(1.0, 60.0, 20.0)
            .begin(),
        AnimationBuilder::new()
            .from(0.0)
            .to(1.0)
            .delay(Duration::from_millis(400)) // Feature badges
            .spring_params(1.0, 60.0, 20.0)
            .begin(),
    ]
}
