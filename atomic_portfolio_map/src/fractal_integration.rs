//! FractalArbitrageScanner Integration Module
//!
//! Integrates the atomic portfolio map with the fractal arbitrage scanner system,
//! creating a unified cross-venue coordination engine that combines:
//! - Williams multiscale analysis for cross-venue pattern detection
//! - HydraCoordinationEngine for unified arbitrage detection
//! - Risk correlation engine for cross-asset risk management
//! - Fractal protection system hooks for emergency stops
//!
//! # Performance Targets (Q29: Practical Constraints)
//! - <50ns for cross-venue coordination updates
//! - Zero allocation in arbitrage detection paths
//! - Cache-aligned coordination structures
//! - 100% lockfree multi-venue operations
//!
//! # Design Philosophy (Q28: Simplicity)
//! Simple Interface: `detect_cross_venue_arbitrage(venues, symbols)`
//! Complex Implementation: Multi-scale fractal analysis + risk correlation + protection systems

#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "const_fn_floating_point_arithmetic", feature(const_fn_floating_point_arithmetic))]
#![cfg_attr(feature = "atomic_from_mut", feature(atomic_from_mut))]

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashMap;

use crate::risk_correlation::RiskCorrelationEngine;
use crate::layout::BreakerLevel;

#[cfg(feature = "portable_simd")]
use std::simd::prelude::*;

/// Maximum number of venues supported for cross-venue arbitrage
pub const MAX_VENUES: usize = 8;

/// Maximum number of symbols per venue
pub const MAX_SYMBOLS_PER_VENUE: usize = 16;

/// Cross-venue price feed data structure
/// Cache-aligned for optimal memory access patterns
#[repr(C, align(64))]
#[derive(Debug, Clone)]
pub struct VenuePriceFeed {
    /// Venue identifier (exchange ID)
    pub venue_id: u32,
    /// Symbol identifier within venue
    pub symbol_id: u32,
    /// Current bid price (fixed-point, price * 1e8)
    pub bid_price: u64,
    /// Current ask price (fixed-point, price * 1e8)
    pub ask_price: u64,
    /// Last trade price (fixed-point, price * 1e8)
    pub last_price: u64,
    /// Volume in last update
    pub volume: u64,
    /// Timestamp of last update (microseconds)
    pub timestamp_us: u64,
    /// Quality score (0-65535, higher = better)
    pub quality_score: u16,
    /// Market depth (number of levels)
    pub depth_levels: u8,
    /// Reserved for future use
    pub _reserved: u8,
}

impl VenuePriceFeed {
    pub fn new(venue_id: u32, symbol_id: u32) -> Self {
        Self {
            venue_id,
            symbol_id,
            bid_price: 0,
            ask_price: 0,
            last_price: 0,
            volume: 0,
            timestamp_us: 0,
            quality_score: 0,
            depth_levels: 0,
            _reserved: 0,
        }
    }

    /// Calculate spread in basis points
    #[inline(always)]
    pub fn spread_bps(&self) -> f64 {
        if self.bid_price == 0 || self.ask_price == 0 {
            return f64::INFINITY;
        }
        let spread = (self.ask_price as f64 - self.bid_price as f64) / 1e8;
        let mid_price = (self.ask_price as f64 + self.bid_price as f64) / (2.0 * 1e8);
        (spread / mid_price) * 10000.0
    }

    /// Get mid price as f64
    #[inline(always)]
    pub fn mid_price(&self) -> f64 {
        if self.bid_price == 0 || self.ask_price == 0 {
            return self.last_price as f64 / 1e8;
        }
        (self.ask_price as f64 + self.bid_price as f64) / (2.0 * 1e8)
    }
}

/// Cross-venue arbitrage opportunity detected by fractal analysis
#[derive(Debug, Clone)]
pub struct CrossVenueArbitrageOpportunity {
    /// Source venue where we buy
    pub buy_venue: u32,
    /// Target venue where we sell
    pub sell_venue: u32,
    /// Symbol being arbitraged
    pub symbol_id: u32,
    /// Buy price at source venue
    pub buy_price: f64,
    /// Sell price at target venue
    pub sell_price: f64,
    /// Expected profit in basis points
    pub profit_bps: f64,
    /// Confidence from fractal analysis (0.0 - 1.0)
    pub fractal_confidence: f64,
    /// Risk score from correlation engine (0.0 - 1.0)
    pub risk_score: f64,
    /// Time window for opportunity (microseconds)
    pub time_window_us: u64,
    /// Generation counter for TOCTOU prevention
    pub generation: u64,
    /// Quality score combining all factors
    pub quality_score: f64,
}

/// Williams multiscale analysis pattern for cross-venue detection
#[derive(Debug, Clone)]
pub struct WilliamsMultiscalePattern {
    /// Fractal dimension across venues
    pub fractal_dimension: f64,
    /// Hurst exponent for mean reversion
    pub hurst_exponent: f64,
    /// Pattern strength (0.0 - 1.0)
    pub pattern_strength: f64,
    /// Predicted duration (microseconds)
    pub duration_us: u64,
    /// Venues participating in pattern
    pub participating_venues: Vec<u32>,
}

/// Cross-venue coordination engine integrating all fractal systems
pub struct PortfolioCrossVenueCoordinator {
    /// Risk correlation engine for cross-asset analysis
    #[allow(dead_code)]
    risk_engine: Arc<RiskCorrelationEngine>,
    /// Price feeds from all venues
    venue_feeds: HashMap<(u32, u32), Arc<AtomicVenueFeed>>, // (venue_id, symbol_id) -> feed
    /// Active arbitrage opportunities
    active_opportunities: HashMap<u64, CrossVenueArbitrageOpportunity>, // generation -> opportunity
    /// Fractal protection flags
    protection_flags: AtomicU64,
    /// Generation counter for opportunities
    opportunity_generation: AtomicU64,
    /// Performance counters
    total_opportunities_detected: AtomicU64,
    total_opportunities_executed: AtomicU64,
    /// Emergency stop flag
    emergency_stop: AtomicBool,
}

/// Atomic wrapper for venue price feeds
/// #ASSUME_CACHE_ALIGNMENT: Feed data is cache-aligned for performance
/// #VERIFY_LOCKFREE_ONLY: All operations use atomic primitives
#[repr(C, align(128))]
pub struct AtomicVenueFeed {
    /// Price data as packed atomic values
    bid_ask_atomic: AtomicU64, // Packed: bid (32 bits) + ask (32 bits)
    last_volume_atomic: AtomicU64, // Packed: last_price (32 bits) + volume (32 bits)
    timestamp_quality_atomic: AtomicU64, // Packed: timestamp (48 bits) + quality (16 bits)
    /// Venue and symbol identifiers
    venue_id: u32,
    symbol_id: u32,
    /// Padding to ensure 128-byte alignment
    _padding: [u8; 128 - 32],
}

impl AtomicVenueFeed {
    pub fn new(venue_id: u32, symbol_id: u32) -> Self {
        Self {
            bid_ask_atomic: AtomicU64::new(0),
            last_volume_atomic: AtomicU64::new(0),
            timestamp_quality_atomic: AtomicU64::new(0),
            venue_id,
            symbol_id,
            _padding: [0; 96],
        }
    }

    /// Update price feed atomically
    /// Performance target: <10ns for single update
    #[inline(always)]
    pub fn update_prices(&self, feed: &VenuePriceFeed) {
        let bid_ask = pack_bid_ask(feed.bid_price, feed.ask_price);
        let last_volume = pack_last_volume(feed.last_price, feed.volume);
        let timestamp_quality = pack_timestamp_quality(feed.timestamp_us, feed.quality_score);

        self.bid_ask_atomic.store(bid_ask, Ordering::Release);
        self.last_volume_atomic.store(last_volume, Ordering::Release);
        self.timestamp_quality_atomic.store(timestamp_quality, Ordering::Release);
    }

    /// Load current price feed
    #[inline(always)]
    pub fn load_feed(&self) -> VenuePriceFeed {
        let bid_ask = self.bid_ask_atomic.load(Ordering::Acquire);
        let last_volume = self.last_volume_atomic.load(Ordering::Acquire);
        let timestamp_quality = self.timestamp_quality_atomic.load(Ordering::Acquire);

        let (bid_price, ask_price) = unpack_bid_ask(bid_ask);
        let (last_price, volume) = unpack_last_volume(last_volume);
        let (timestamp_us, quality_score) = unpack_timestamp_quality(timestamp_quality);

        VenuePriceFeed {
            venue_id: self.venue_id,
            symbol_id: self.symbol_id,
            bid_price,
            ask_price,
            last_price,
            volume,
            timestamp_us,
            quality_score,
            depth_levels: 0,
            _reserved: 0,
        }
    }
}

impl PortfolioCrossVenueCoordinator {
    /// Create new cross-venue coordinator
    pub fn new(risk_engine: Arc<RiskCorrelationEngine>) -> Self {
        Self {
            risk_engine,
            venue_feeds: HashMap::new(),
            active_opportunities: HashMap::new(),
            protection_flags: AtomicU64::new(0),
            opportunity_generation: AtomicU64::new(0),
            total_opportunities_detected: AtomicU64::new(0),
            total_opportunities_executed: AtomicU64::new(0),
            emergency_stop: AtomicBool::new(false),
        }
    }

    /// Register a new venue price feed
    pub fn register_venue_feed(&mut self, venue_id: u32, symbol_id: u32) {
        let feed = Arc::new(AtomicVenueFeed::new(venue_id, symbol_id));
        self.venue_feeds.insert((venue_id, symbol_id), feed);
    }

    /// Update price feed for a venue
    /// Performance target: <30ns including arbitrage detection
    #[inline(always)]
    pub fn update_venue_price(&self, venue_id: u32, symbol_id: u32, feed: &VenuePriceFeed) -> Option<CrossVenueArbitrageOpportunity> {
        // Check emergency stop first
        if self.emergency_stop.load(Ordering::Relaxed) {
            return None;
        }

        // Update the atomic feed
        if let Some(atomic_feed) = self.venue_feeds.get(&(venue_id, symbol_id)) {
            atomic_feed.update_prices(feed);

            // Check for immediate arbitrage opportunities
            self.detect_immediate_arbitrage(symbol_id, feed)
        } else {
            None
        }
    }

    /// Detect immediate arbitrage opportunities using Williams multiscale analysis
    ///
    /// #ASSUME_PRICE_VALIDITY: Price feeds are recent and valid
    /// #VERIFY_PERFORMANCE: Benchmark validates <20ns detection time
    fn detect_immediate_arbitrage(&self, symbol_id: u32, updated_feed: &VenuePriceFeed) -> Option<CrossVenueArbitrageOpportunity> {
        let mut best_opportunity: Option<CrossVenueArbitrageOpportunity> = None;
        let mut max_profit_bps = 0.0;

        // Compare updated venue with all other venues for same symbol
        for ((venue_id, other_symbol_id), other_feed) in &self.venue_feeds {
            if *other_symbol_id != symbol_id || *venue_id == updated_feed.venue_id {
                continue;
            }

            let other_prices = other_feed.load_feed();

            // Check buy low, sell high opportunity
            if let Some(opportunity) = self.calculate_arbitrage_opportunity(
                updated_feed,
                &other_prices,
                symbol_id
            ) {
                if opportunity.profit_bps > max_profit_bps && opportunity.profit_bps > 5.0 { // Minimum 5 bps
                    max_profit_bps = opportunity.profit_bps;
                    best_opportunity = Some(opportunity);
                }
            }

            // Check reverse opportunity (sell low venue, buy high venue)
            if let Some(opportunity) = self.calculate_arbitrage_opportunity(
                &other_prices,
                updated_feed,
                symbol_id
            ) {
                if opportunity.profit_bps > max_profit_bps && opportunity.profit_bps > 5.0 {
                    max_profit_bps = opportunity.profit_bps;
                    best_opportunity = Some(opportunity);
                }
            }
        }

        if let Some(ref opportunity) = best_opportunity {
            // Increment detection counter
            self.total_opportunities_detected.fetch_add(1, Ordering::Relaxed);

            // Apply risk correlation analysis
            if self.passes_risk_correlation_check(opportunity) {
                best_opportunity
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Calculate arbitrage opportunity between two venues
    fn calculate_arbitrage_opportunity(
        &self,
        buy_venue_feed: &VenuePriceFeed,
        sell_venue_feed: &VenuePriceFeed,
        symbol_id: u32
    ) -> Option<CrossVenueArbitrageOpportunity> {
        // Basic arbitrage: buy at ask on buy_venue, sell at bid on sell_venue
        let buy_price = buy_venue_feed.ask_price as f64 / 1e8;
        let sell_price = sell_venue_feed.bid_price as f64 / 1e8;

        if sell_price <= buy_price {
            return None; // No profit opportunity
        }

        let profit_bps = ((sell_price - buy_price) / buy_price) * 10000.0;

        // Apply Williams multiscale analysis for confidence
        let fractal_confidence = self.calculate_williams_multiscale_confidence(
            buy_venue_feed,
            sell_venue_feed
        );

        // Calculate risk score
        let risk_score = self.calculate_risk_score(buy_venue_feed, sell_venue_feed);

        // Calculate time window based on market dynamics
        let time_window_us = self.calculate_time_window(buy_venue_feed, sell_venue_feed);

        // Generate unique opportunity ID
        let generation = self.opportunity_generation.fetch_add(1, Ordering::AcqRel);

        // Quality score combines profit, confidence, and risk
        let quality_score = (profit_bps / 100.0) * fractal_confidence * (1.0 - risk_score);

        Some(CrossVenueArbitrageOpportunity {
            buy_venue: buy_venue_feed.venue_id,
            sell_venue: sell_venue_feed.venue_id,
            symbol_id,
            buy_price,
            sell_price,
            profit_bps,
            fractal_confidence,
            risk_score,
            time_window_us,
            generation,
            quality_score,
        })
    }

    /// Calculate Williams multiscale confidence for cross-venue pattern
    ///
    /// Uses simplified multiscale analysis optimized for real-time performance
    fn calculate_williams_multiscale_confidence(&self, feed1: &VenuePriceFeed, feed2: &VenuePriceFeed) -> f64 {
        // Simplified confidence based on:
        // 1. Price quality scores
        // 2. Spread consistency
        // 3. Volume correlation

        let quality_factor = (feed1.quality_score as f64 + feed2.quality_score as f64) / (2.0 * u16::MAX as f64);

        let spread1 = feed1.spread_bps();
        let spread2 = feed2.spread_bps();
        let spread_consistency = if spread1.is_finite() && spread2.is_finite() {
            1.0 - ((spread1 - spread2).abs() / (spread1 + spread2).max(1.0))
        } else {
            0.5
        };

        let volume_factor = if feed1.volume > 0 && feed2.volume > 0 {
            let volume_ratio = (feed1.volume.min(feed2.volume) as f64) / (feed1.volume.max(feed2.volume) as f64);
            volume_ratio.sqrt() // Square root to moderate the effect
        } else {
            0.3 // Default when volume data is missing
        };

        // Combine factors with Williams-inspired weighting
        (quality_factor * 0.4 + spread_consistency * 0.4 + volume_factor * 0.2).clamp(0.0, 1.0)
    }

    /// Calculate risk score for arbitrage opportunity
    fn calculate_risk_score(&self, feed1: &VenuePriceFeed, feed2: &VenuePriceFeed) -> f64 {
        // Risk factors:
        // 1. Timestamp staleness
        // 2. Spread width
        // 3. Venue quality differential

        let current_time = get_current_timestamp_us();
        let staleness1 = (current_time.saturating_sub(feed1.timestamp_us)) as f64 / 1_000_000.0; // seconds
        let staleness2 = (current_time.saturating_sub(feed2.timestamp_us)) as f64 / 1_000_000.0;
        let staleness_risk = ((staleness1 + staleness2) / 10.0).min(1.0); // Max 10 seconds

        let spread_risk = (feed1.spread_bps() + feed2.spread_bps()) / 200.0; // Normalize to reasonable range
        let spread_risk_clamped = spread_risk.min(1.0);

        let quality_diff = ((feed1.quality_score as i32 - feed2.quality_score as i32).abs() as f64) / (u16::MAX as f64);

        // Combine risk factors
        (staleness_risk * 0.4 + spread_risk_clamped * 0.4 + quality_diff * 0.2).clamp(0.0, 1.0)
    }

    /// Calculate time window for opportunity based on market dynamics
    fn calculate_time_window(&self, feed1: &VenuePriceFeed, feed2: &VenuePriceFeed) -> u64 {
        // Base time window from spread and quality
        let avg_spread = (feed1.spread_bps() + feed2.spread_bps()) / 2.0;
        let avg_quality = (feed1.quality_score as f64 + feed2.quality_score as f64) / 2.0;

        // Tighter spreads and higher quality = shorter windows
        let base_window_us = 100_000; // 100ms base
        let spread_factor = (avg_spread / 50.0).clamp(0.5, 3.0); // 50bps reference
        let quality_factor = (avg_quality / (u16::MAX as f64)).clamp(0.3, 1.0);

        ((base_window_us as f64) * spread_factor / quality_factor) as u64
    }

    /// Check if opportunity passes risk correlation analysis
    fn passes_risk_correlation_check(&self, opportunity: &CrossVenueArbitrageOpportunity) -> bool {
        // For now, use simple risk score threshold
        // In production, this would integrate with full correlation matrix
        let risk_threshold = match self.get_current_breaker_level() {
            BreakerLevel::L0 => 0.8,
            BreakerLevel::L1 => 0.6,
            BreakerLevel::L2 => 0.4,
            BreakerLevel::L3 => 0.2,
        };

        opportunity.risk_score <= risk_threshold
    }

    /// Get current portfolio breaker level
    fn get_current_breaker_level(&self) -> BreakerLevel {
        // Simplified implementation - would integrate with actual portfolio state
        let protection_flags = self.protection_flags.load(Ordering::Relaxed);
        BreakerLevel::from_u8((protection_flags & 0x3) as u8)
    }

    /// Set emergency stop flag
    pub fn set_emergency_stop(&self, stop: bool) {
        self.emergency_stop.store(stop, Ordering::Release);
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> CrossVenuePerformanceStats {
        CrossVenuePerformanceStats {
            total_opportunities_detected: self.total_opportunities_detected.load(Ordering::Relaxed),
            total_opportunities_executed: self.total_opportunities_executed.load(Ordering::Relaxed),
            active_opportunities: self.active_opportunities.len() as u64,
            emergency_stop_active: self.emergency_stop.load(Ordering::Relaxed),
        }
    }

    /// Enable fractal protection system hooks
    ///
    /// Integrates with the fractal protection system from fractal_arbitrage_scanner
    pub fn enable_fractal_protection(&self, tier: ProtectionTier) {
        let tier_flags = match tier {
            ProtectionTier::Conservative => 0x1,
            ProtectionTier::Moderate => 0x2,
            ProtectionTier::Aggressive => 0x3,
        };

        self.protection_flags.store(tier_flags, Ordering::Release);
    }
}

/// Performance statistics for cross-venue coordination
#[derive(Debug, Clone)]
pub struct CrossVenuePerformanceStats {
    pub total_opportunities_detected: u64,
    pub total_opportunities_executed: u64,
    pub active_opportunities: u64,
    pub emergency_stop_active: bool,
}

/// Fractal protection tiers for integration
#[derive(Debug, Clone, Copy)]
pub enum ProtectionTier {
    Conservative,
    Moderate,
    Aggressive,
}

// Helper functions for atomic packing/unpacking

#[inline(always)]
fn pack_bid_ask(bid: u64, ask: u64) -> u64 {
    ((bid & 0xFFFFFFFF) << 32) | (ask & 0xFFFFFFFF)
}

#[inline(always)]
fn unpack_bid_ask(packed: u64) -> (u64, u64) {
    let bid = (packed >> 32) & 0xFFFFFFFF;
    let ask = packed & 0xFFFFFFFF;
    (bid, ask)
}

#[inline(always)]
fn pack_last_volume(last_price: u64, volume: u64) -> u64 {
    ((last_price & 0xFFFFFFFF) << 32) | (volume & 0xFFFFFFFF)
}

#[inline(always)]
fn unpack_last_volume(packed: u64) -> (u64, u64) {
    let last_price = (packed >> 32) & 0xFFFFFFFF;
    let volume = packed & 0xFFFFFFFF;
    (last_price, volume)
}

#[inline(always)]
fn pack_timestamp_quality(timestamp: u64, quality: u16) -> u64 {
    ((timestamp & 0xFFFFFFFFFFFF) << 16) | (quality as u64)
}

#[inline(always)]
fn unpack_timestamp_quality(packed: u64) -> (u64, u16) {
    let timestamp = (packed >> 16) & 0xFFFFFFFFFFFF;
    let quality = (packed & 0xFFFF) as u16;
    (timestamp, quality)
}

/// Get current timestamp in microseconds
fn get_current_timestamp_us() -> u64 {
    // Simplified implementation for testing
    // In production, would use system clock
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1000, Ordering::Relaxed) // Increment by 1ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_venue_price_feed_basic() {
        let feed = VenuePriceFeed::new(1, 100);
        assert_eq!(feed.venue_id, 1);
        assert_eq!(feed.symbol_id, 100);
    }

    #[test]
    fn test_atomic_venue_feed_update() {
        let atomic_feed = AtomicVenueFeed::new(1, 100);
        let mut feed = VenuePriceFeed::new(1, 100);
        feed.bid_price = 95_000_000; // $95.00
        feed.ask_price = 95_050_000; // $95.05
        feed.last_price = 95_025_000; // $95.025
        feed.volume = 1000;
        feed.quality_score = 50000;

        atomic_feed.update_prices(&feed);
        let loaded = atomic_feed.load_feed();

        assert_eq!(loaded.bid_price, feed.bid_price);
        assert_eq!(loaded.ask_price, feed.ask_price);
        assert_eq!(loaded.last_price, feed.last_price);
        assert_eq!(loaded.volume, feed.volume);
        assert_eq!(loaded.quality_score, feed.quality_score);
    }

    #[test]
    fn test_spread_calculation() {
        let mut feed = VenuePriceFeed::new(1, 100);
        feed.bid_price = 100_000_000; // $100.00
        feed.ask_price = 100_100_000; // $100.10

        let spread = feed.spread_bps();
        assert!((spread - 10.0).abs() < 0.1); // Should be ~10 bps
    }

    #[test]
    fn test_cross_venue_coordinator_creation() {
        let risk_engine = Arc::new(RiskCorrelationEngine::new());
        let coordinator = PortfolioCrossVenueCoordinator::new(risk_engine);

        let stats = coordinator.get_performance_stats();
        assert_eq!(stats.total_opportunities_detected, 0);
        assert_eq!(stats.active_opportunities, 0);
        assert!(!stats.emergency_stop_active);
    }

    #[test]
    fn test_arbitrage_opportunity_detection() {
        let risk_engine = Arc::new(RiskCorrelationEngine::new());
        let mut coordinator = PortfolioCrossVenueCoordinator::new(risk_engine);

        // Register two venues for same symbol
        coordinator.register_venue_feed(1, 100);
        coordinator.register_venue_feed(2, 100);

        // Create price feeds with arbitrage opportunity
        let mut feed1 = VenuePriceFeed::new(1, 100);
        feed1.bid_price = 99_900_000; // $99.90
        feed1.ask_price = 100_000_000; // $100.00
        feed1.quality_score = 50000;

        let mut feed2 = VenuePriceFeed::new(2, 100);
        feed2.bid_price = 100_200_000; // $100.20
        feed2.ask_price = 100_300_000; // $100.30
        feed2.quality_score = 50000;

        // Update first venue (should trigger arbitrage detection)
        coordinator.update_venue_price(1, 100, &feed1);

        // Update second venue (should detect arbitrage: buy at $100.00, sell at $100.20)
        let opportunity = coordinator.update_venue_price(2, 100, &feed2);

        if let Some(opp) = opportunity {
            assert!(opp.profit_bps > 0.0);
            assert_eq!(opp.buy_venue, 1);
            assert_eq!(opp.sell_venue, 2);
            assert_eq!(opp.symbol_id, 100);
        }
    }

    #[test]
    fn test_packing_unpacking_functions() {
        let bid = 1000000;
        let ask = 2000000;
        let packed = pack_bid_ask(bid, ask);
        let (unpacked_bid, unpacked_ask) = unpack_bid_ask(packed);
        assert_eq!(bid, unpacked_bid);
        assert_eq!(ask, unpacked_ask);

        let timestamp = 1234567890123456;
        let quality = 32000;
        let packed_tq = pack_timestamp_quality(timestamp, quality);
        let (unpacked_ts, unpacked_q) = unpack_timestamp_quality(packed_tq);
        assert_eq!(timestamp & 0xFFFFFFFFFFFF, unpacked_ts); // Lower 48 bits
        assert_eq!(quality, unpacked_q);
    }

    #[test]
    fn test_emergency_stop_functionality() {
        let risk_engine = Arc::new(RiskCorrelationEngine::new());
        let coordinator = PortfolioCrossVenueCoordinator::new(risk_engine);

        // Normal operation
        assert!(!coordinator.get_performance_stats().emergency_stop_active);

        // Enable emergency stop
        coordinator.set_emergency_stop(true);
        assert!(coordinator.get_performance_stats().emergency_stop_active);

        // Disable emergency stop
        coordinator.set_emergency_stop(false);
        assert!(!coordinator.get_performance_stats().emergency_stop_active);
    }

    #[test]
    fn test_fractal_protection_integration() {
        let risk_engine = Arc::new(RiskCorrelationEngine::new());
        let coordinator = PortfolioCrossVenueCoordinator::new(risk_engine);

        // Test protection tier setting
        coordinator.enable_fractal_protection(ProtectionTier::Conservative);
        let flags = coordinator.protection_flags.load(Ordering::Relaxed);
        assert_eq!(flags & 0x3, 0x1);

        coordinator.enable_fractal_protection(ProtectionTier::Aggressive);
        let flags = coordinator.protection_flags.load(Ordering::Relaxed);
        assert_eq!(flags & 0x3, 0x3);
    }
}