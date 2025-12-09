//! Venue Selection and Load Balancing
//!
//! Intelligent venue selection algorithms for optimal coordination
//! across multiple trading venues with health monitoring and
//! load balancing capabilities.

use crate::{
    types::{VenueId, VenueHealth, VenueStatus, VenueSelectionConfig, CoordinationPriority},
    venue_array::{VenueArray, VenueSnapshot},
    error::{CoordinationError, VenueError},
    MAX_VENUES,
};

/// Venue selection strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Round-robin selection
    RoundRobin,
    /// Lowest latency first
    LowestLatency,
    /// Highest success rate first
    HighestSuccessRate,
    /// Weighted selection based on health score
    WeightedHealth,
    /// Random selection
    Random,
    /// Load balanced selection
    LoadBalanced,
}

/// Venue selector for intelligent venue coordination
#[derive(Debug)]
pub struct VenueSelector {
    /// Selection strategy
    strategy: SelectionStrategy,
    /// Configuration
    config: VenueSelectionConfig,
    /// Load balancer
    load_balancer: LoadBalancer,
    /// Venue health cache
    health_cache: [Option<VenueHealth>; MAX_VENUES],
    /// Last health check timestamp
    last_health_check: u64,
    /// Round-robin counter
    round_robin_counter: usize,
}

impl VenueSelector {
    /// Create new venue selector
    pub fn new(strategy: SelectionStrategy, config: VenueSelectionConfig) -> Self {
        Self {
            strategy,
            config,
            load_balancer: LoadBalancer::new(),
            health_cache: [None; MAX_VENUES],
            last_health_check: 0,
            round_robin_counter: 0,
        }
    }

    /// Select optimal venues for coordination
    pub fn select_venues(
        &mut self,
        venue_array: &VenueArray,
        requested_venues: &[VenueId],
        priority: CoordinationPriority,
    ) -> Result<Vec<VenueId>, CoordinationError> {
        // Update health cache if needed
        self.update_health_cache_if_needed(venue_array)?;

        // Filter available venues
        let available_venues = self.filter_available_venues(requested_venues)?;

        if available_venues.is_empty() {
            return Err(CoordinationError::VenueUnavailable(0)); // No venues available
        }

        // Apply selection strategy
        let selected = match self.strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin(&available_venues),
            SelectionStrategy::LowestLatency => self.select_lowest_latency(&available_venues),
            SelectionStrategy::HighestSuccessRate => self.select_highest_success_rate(&available_venues),
            SelectionStrategy::WeightedHealth => self.select_weighted_health(&available_venues),
            SelectionStrategy::Random => self.select_random(&available_venues),
            SelectionStrategy::LoadBalanced => self.select_load_balanced(&available_venues, priority),
        };

        // Limit to maximum venues per operation
        let limited = selected
            .into_iter()
            .take(self.config.max_venues_per_operation)
            .collect();

        Ok(limited)
    }

    /// Update health cache if interval has passed
    fn update_health_cache_if_needed(&mut self, venue_array: &VenueArray) -> Result<(), CoordinationError> {
        let current_time = self.get_timestamp_ns();

        if current_time.saturating_sub(self.last_health_check) >= self.config.health_check_interval_ns {
            self.update_health_cache(venue_array)?;
            self.last_health_check = current_time;
        }

        Ok(())
    }

    /// Update venue health cache
    fn update_health_cache(&mut self, venue_array: &VenueArray) -> Result<(), CoordinationError> {
        for venue_id in 0..MAX_VENUES {
            if let Ok(venue) = venue_array.venue(venue_id) {
                let health = self.calculate_venue_health(venue_id, venue);
                self.health_cache[venue_id] = Some(health);
            }
        }
        Ok(())
    }

    /// Calculate venue health metrics
    fn calculate_venue_health(&self, venue_id: VenueId, venue: &VenueSnapshot) -> VenueHealth {
        let metrics = venue.metrics();
        let state_flags = venue.state_flags();

        let status = if state_flags.contains(crate::venue_array::VenueState::EMERGENCY_STOP) {
            VenueStatus::EmergencyStop
        } else if state_flags.contains(crate::venue_array::VenueState::MAINTENANCE) {
            VenueStatus::Maintenance
        } else if state_flags.contains(crate::venue_array::VenueState::HALTED) {
            VenueStatus::Halted
        } else if state_flags.contains(crate::venue_array::VenueState::UNSTABLE) {
            VenueStatus::Unstable
        } else if state_flags.contains(crate::venue_array::VenueState::ACTIVE) {
            VenueStatus::Active
        } else {
            VenueStatus::Inactive
        };

        VenueHealth {
            venue_id,
            status,
            success_rate: metrics.success_rate(),
            avg_latency_ns: metrics.avg_update_latency_ns,
            last_update_ns: metrics.last_update_ns,
            error_count: metrics.update_failures as u32,
        }
    }

    /// Filter venues that meet availability criteria
    fn filter_available_venues(&self, requested_venues: &[VenueId]) -> Result<Vec<VenueId>, CoordinationError> {
        let mut available = Vec::new();

        for &venue_id in requested_venues {
            if venue_id >= MAX_VENUES {
                continue;
            }

            if let Some(health) = &self.health_cache[venue_id] {
                if health.status.is_available() &&
                   health.success_rate >= self.config.min_success_rate &&
                   health.avg_latency_ns <= self.config.max_latency_ns {
                    available.push(venue_id);
                }
            }
        }

        Ok(available)
    }

    /// Round-robin venue selection
    fn select_round_robin(&mut self, available_venues: &[VenueId]) -> Vec<VenueId> {
        if available_venues.is_empty() {
            return Vec::new();
        }

        let selected = available_venues[self.round_robin_counter % available_venues.len()];
        self.round_robin_counter = self.round_robin_counter.wrapping_add(1);
        vec![selected]
    }

    /// Select venue with lowest latency
    fn select_lowest_latency(&self, available_venues: &[VenueId]) -> Vec<VenueId> {
        let mut venues_with_latency: Vec<_> = available_venues
            .iter()
            .filter_map(|&venue_id| {
                self.health_cache[venue_id]
                    .as_ref()
                    .map(|health| (venue_id, health.avg_latency_ns))
            })
            .collect();

        venues_with_latency.sort_by_key(|&(_, latency)| latency);
        venues_with_latency.into_iter().map(|(venue_id, _)| venue_id).collect()
    }

    /// Select venue with highest success rate
    fn select_highest_success_rate(&self, available_venues: &[VenueId]) -> Vec<VenueId> {
        let mut venues_with_success_rate: Vec<_> = available_venues
            .iter()
            .filter_map(|&venue_id| {
                self.health_cache[venue_id]
                    .as_ref()
                    .map(|health| (venue_id, health.success_rate))
            })
            .collect();

        venues_with_success_rate.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        venues_with_success_rate.into_iter().map(|(venue_id, _)| venue_id).collect()
    }

    /// Select venues based on weighted health scores
    fn select_weighted_health(&self, available_venues: &[VenueId]) -> Vec<VenueId> {
        let mut venues_with_health: Vec<_> = available_venues
            .iter()
            .filter_map(|&venue_id| {
                self.health_cache[venue_id]
                    .as_ref()
                    .map(|health| (venue_id, health.health_score()))
            })
            .collect();

        venues_with_health.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        venues_with_health.into_iter().map(|(venue_id, _)| venue_id).collect()
    }

    /// Random venue selection
    fn select_random(&self, available_venues: &[VenueId]) -> Vec<VenueId> {
        if available_venues.is_empty() {
            return Vec::new();
        }

        // Simple pseudo-random selection based on timestamp
        let current_time = self.get_timestamp_ns();
        let index = (current_time as usize) % available_venues.len();
        vec![available_venues[index]]
    }

    /// Load balanced venue selection
    fn select_load_balanced(&mut self, available_venues: &[VenueId], priority: CoordinationPriority) -> Vec<VenueId> {
        self.load_balancer.select_venues(available_venues, priority, &self.health_cache)
    }

    /// Get venue health from cache
    pub fn get_venue_health(&self, venue_id: VenueId) -> Option<&VenueHealth> {
        if venue_id < MAX_VENUES {
            self.health_cache[venue_id].as_ref()
        } else {
            None
        }
    }

    /// Get current selection strategy
    pub fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }

    /// Set selection strategy
    pub fn set_strategy(&mut self, strategy: SelectionStrategy) {
        self.strategy = strategy;
    }

    /// Get configuration
    pub fn config(&self) -> &VenueSelectionConfig {
        &self.config
    }

    /// Get current timestamp
    #[cfg(feature = "std")]
    fn get_timestamp_ns(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    #[cfg(not(feature = "std"))]
    fn get_timestamp_ns(&self) -> u64 {
        0
    }
}

/// Load balancer for venue coordination
#[derive(Debug)]
pub struct LoadBalancer {
    /// Per-venue load tracking
    venue_loads: [u64; MAX_VENUES],
    /// Last load update timestamp
    last_update: u64,
    /// Load decay factor (for exponential decay)
    decay_factor: f64,
}

impl LoadBalancer {
    /// Create new load balancer
    pub fn new() -> Self {
        Self {
            venue_loads: [0; MAX_VENUES],
            last_update: 0,
            decay_factor: 0.95, // 5% decay per update
        }
    }

    /// Select venues based on load balancing
    pub fn select_venues(
        &mut self,
        available_venues: &[VenueId],
        priority: CoordinationPriority,
        health_cache: &[Option<VenueHealth>; MAX_VENUES],
    ) -> Vec<VenueId> {
        // Update load decay
        self.update_load_decay();

        // Calculate venue scores (lower is better)
        let mut venue_scores: Vec<_> = available_venues
            .iter()
            .filter_map(|&venue_id| {
                health_cache[venue_id].as_ref().map(|health| {
                    let load_score = self.venue_loads[venue_id] as f64;
                    let health_score = 1.0 - health.health_score(); // Invert so lower is better
                    let latency_score = health.avg_latency_ns as f64 / 1_000_000.0; // Normalize to ms

                    // Weight factors based on priority
                    let (load_weight, health_weight, latency_weight) = match priority {
                        CoordinationPriority::Emergency => (0.1, 0.3, 0.6), // Prioritize latency
                        CoordinationPriority::High => (0.2, 0.4, 0.4),
                        CoordinationPriority::Normal => (0.4, 0.3, 0.3),
                        CoordinationPriority::Low => (0.6, 0.2, 0.2),
                        CoordinationPriority::Background => (0.8, 0.1, 0.1), // Prioritize load balancing
                    };

                    let total_score = load_score * load_weight +
                                    health_score * health_weight +
                                    latency_score * latency_weight;

                    (venue_id, total_score)
                })
            })
            .collect();

        // Sort by score (ascending - lower is better)
        venue_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Select venues and update their load
        let selected: Vec<_> = venue_scores.into_iter().map(|(venue_id, _)| {
            self.record_venue_selection(venue_id);
            venue_id
        }).collect();

        selected
    }

    /// Record venue selection to update load
    pub fn record_venue_selection(&mut self, venue_id: VenueId) {
        if venue_id < MAX_VENUES {
            self.venue_loads[venue_id] = self.venue_loads[venue_id].saturating_add(1);
        }
    }

    /// Update load decay for all venues
    fn update_load_decay(&mut self) {
        let current_time = self.get_timestamp_ns();

        // Apply decay if enough time has passed (e.g., every second)
        if current_time.saturating_sub(self.last_update) >= 1_000_000_000 {
            for load in &mut self.venue_loads {
                *load = (*load as f64 * self.decay_factor) as u64;
            }
            self.last_update = current_time;
        }
    }

    /// Get current load for venue
    pub fn get_venue_load(&self, venue_id: VenueId) -> u64 {
        if venue_id < MAX_VENUES {
            self.venue_loads[venue_id]
        } else {
            0
        }
    }

    /// Reset load for venue
    pub fn reset_venue_load(&mut self, venue_id: VenueId) {
        if venue_id < MAX_VENUES {
            self.venue_loads[venue_id] = 0;
        }
    }

    /// Get total system load
    pub fn total_load(&self) -> u64 {
        self.venue_loads.iter().sum()
    }

    /// Get current timestamp
    #[cfg(feature = "std")]
    fn get_timestamp_ns(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    #[cfg(not(feature = "std"))]
    fn get_timestamp_ns(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::venue_array::VenueArray;

    #[test]
    fn test_venue_selector_creation() {
        let config = VenueSelectionConfig::default();
        let selector = VenueSelector::new(SelectionStrategy::RoundRobin, config);

        assert_eq!(selector.strategy(), SelectionStrategy::RoundRobin);
    }

    #[test]
    fn test_load_balancer() {
        let mut load_balancer = LoadBalancer::new();

        // Record some selections
        load_balancer.record_venue_selection(0);
        load_balancer.record_venue_selection(1);
        load_balancer.record_venue_selection(0);

        assert_eq!(load_balancer.get_venue_load(0), 2);
        assert_eq!(load_balancer.get_venue_load(1), 1);
        assert_eq!(load_balancer.total_load(), 3);

        load_balancer.reset_venue_load(0);
        assert_eq!(load_balancer.get_venue_load(0), 0);
    }

    #[test]
    fn test_venue_health_calculation() {
        use crate::venue_array::VenueSnapshot;

        let venue = VenueSnapshot::new(0);
        let config = VenueSelectionConfig::default();
        let selector = VenueSelector::new(SelectionStrategy::WeightedHealth, config);

        let health = selector.calculate_venue_health(0, &venue);
        assert_eq!(health.venue_id, 0);
        assert_eq!(health.status, VenueStatus::Inactive); // Default state
    }

    #[test]
    fn test_selection_strategies() {
        let config = VenueSelectionConfig::default();
        let mut selector = VenueSelector::new(SelectionStrategy::RoundRobin, config);

        // Test round robin
        let venues = vec![0, 1, 2];
        let selected1 = selector.select_round_robin(&venues);
        let selected2 = selector.select_round_robin(&venues);
        let selected3 = selector.select_round_robin(&venues);

        assert_eq!(selected1, vec![0]);
        assert_eq!(selected2, vec![1]);
        assert_eq!(selected3, vec![2]);
    }
}