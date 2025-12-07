//! Session Pool and Memory Replay Tools Tests
//!
//! Tests for MCP tools 13-23:
//! - Session Pool Tools (13-17): allocate, release, get_tier, upgrade, get_pool_stats
//! - Memory Replay Tools (18-23): enable, capture, read, navigate, stats, verify

use kdb::session_pool::{
    SessionPoolCapsule, SessionTierType, SessionId, PoolConfig, PoolError, PoolStats,
};
use kdb::memory_replay::{
    MemoryReplayCapsule, ReplayConfig, ReplayError, ReplayStats,
    MAX_TRACKED_PAGES, MAX_DELTAS_PER_SNAPSHOT,
};

// ============================================================================
// Session Pool Capsule Tests (Tools 13-17)
// ============================================================================

#[test]
fn test_session_pool_capsule_creation() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let stats = pool.get_pool_stats();
    assert_eq!(stats.light_used, 0);
    assert_eq!(stats.medium_used, 0);
    assert_eq!(stats.heavy_used, 0);
}

#[test]
fn test_session_allocate_light() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let result = pool.allocate_session(SessionTierType::Light);
    assert!(result.is_ok());

    let session_id = result.unwrap();
    let tier = pool.get_session_tier(session_id);
    assert!(tier.is_some());
    assert_eq!(tier.unwrap(), SessionTierType::Light);
}

#[test]
fn test_session_allocate_medium() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let result = pool.allocate_session(SessionTierType::Medium);
    assert!(result.is_ok());

    let session_id = result.unwrap();
    let tier = pool.get_session_tier(session_id);
    assert!(tier.is_some());
    assert_eq!(tier.unwrap(), SessionTierType::Medium);
}

#[test]
fn test_session_allocate_heavy() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let result = pool.allocate_session(SessionTierType::Heavy);
    assert!(result.is_ok());

    let session_id = result.unwrap();
    let tier = pool.get_session_tier(session_id);
    assert!(tier.is_some());
    assert_eq!(tier.unwrap(), SessionTierType::Heavy);
}

#[test]
fn test_session_release() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let session_id = pool.allocate_session(SessionTierType::Light).unwrap();

    let stats_before = pool.get_pool_stats();
    assert_eq!(stats_before.light_used, 1);

    let release_result = pool.release_session(session_id);
    assert!(release_result.is_ok());

    let stats_after = pool.get_pool_stats();
    assert_eq!(stats_after.light_used, 0);
}

#[test]
fn test_session_id_tier_extraction() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    // Allocate sessions of each tier
    let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
    let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
    let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

    // Verify tier extraction from session ID
    assert_eq!(light_id.tier_type(), Some(SessionTierType::Light));
    assert_eq!(medium_id.tier_type(), Some(SessionTierType::Medium));
    assert_eq!(heavy_id.tier_type(), Some(SessionTierType::Heavy));
}

#[test]
fn test_pool_stats_tracking() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    // Initial state
    let stats = pool.get_pool_stats();
    assert_eq!(stats.total_allocations, 0);
    assert_eq!(stats.total_releases, 0);

    // Allocate some sessions
    let s1 = pool.allocate_session(SessionTierType::Light).unwrap();
    let _s2 = pool.allocate_session(SessionTierType::Medium).unwrap();

    let stats = pool.get_pool_stats();
    assert_eq!(stats.total_allocations, 2);
    assert_eq!(stats.light_used, 1);
    assert_eq!(stats.medium_used, 1);

    // Release one
    pool.release_session(s1).unwrap();

    let stats = pool.get_pool_stats();
    assert_eq!(stats.total_releases, 1);
    assert_eq!(stats.light_used, 0);
}

#[test]
fn test_pool_capacity_limits() {
    // Create pool with small capacity
    let config = PoolConfig {
        light_capacity: 2,
        medium_capacity: 1,
        heavy_capacity: 1,
        ..PoolConfig::default()
    };
    let pool = SessionPoolCapsule::new(config);

    // Allocate up to capacity
    let _s1 = pool.allocate_session(SessionTierType::Light).unwrap();
    let _s2 = pool.allocate_session(SessionTierType::Light).unwrap();

    // Third allocation should fail with PoolFull
    let result = pool.allocate_session(SessionTierType::Light);
    assert!(matches!(result, Err(PoolError::PoolFull { tier: SessionTierType::Light, capacity: 2 })));
}

// ============================================================================
// Memory Replay Capsule Tests (Tools 18-23)
// ============================================================================

#[test]
fn test_memory_replay_capsule_creation() {
    let replay = MemoryReplayCapsule::new();
    let stats = replay.get_stats();

    assert_eq!(stats.total_snapshots, 0);
    assert_eq!(stats.total_deltas, 0);
}

#[test]
fn test_memory_replay_with_config() {
    let config = ReplayConfig::minimal();
    let replay = MemoryReplayCapsule::with_config(config);

    let stats = replay.get_stats();
    assert_eq!(stats.total_snapshots, 0);
}

#[test]
fn test_memory_replay_config_presets() {
    // Test all config presets exist
    let _default = ReplayConfig::default();
    let _minimal = ReplayConfig::minimal();
    let _performance = ReplayConfig::performance();
    let _compliance = ReplayConfig::compliance();
}

#[test]
fn test_memory_replay_stats_fields() {
    let replay = MemoryReplayCapsule::new();
    let stats = replay.get_stats();

    // Verify all expected fields exist
    let _ = stats.total_snapshots;
    let _ = stats.total_deltas;
    let _ = stats.memory_usage_bytes;
    let _ = stats.avg_snapshot_us;
    let _ = stats.tracked_pages;
    let _ = stats.last_dirty_count;
    let _ = stats.storage_fill;
    let _ = stats.cache_hit_rate;
}

#[test]
fn test_memory_replay_max_constants() {
    // Verify constants are reasonable
    assert!(MAX_TRACKED_PAGES > 0);
    assert!(MAX_DELTAS_PER_SNAPSHOT > 0);
    assert!(MAX_TRACKED_PAGES >= 1024); // At least 4MB of pages
    assert!(MAX_DELTAS_PER_SNAPSHOT >= 16); // Reasonable delta capacity
}

// ============================================================================
// Session ID Encoding Tests
// ============================================================================

#[test]
fn test_session_id_u64_encoding() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let session_id = pool.allocate_session(SessionTierType::Light).unwrap();

    // Session ID should be extractable as u64
    let raw_id: u64 = session_id.0;
    assert!(raw_id > 0);

    // Should be reconstructible
    let reconstructed = SessionId(raw_id);
    assert_eq!(reconstructed.0, raw_id);
    assert_eq!(reconstructed.tier_type(), session_id.tier_type());
}

#[test]
fn test_session_tier_type_as_str() {
    // Verify tier string representations
    let light = SessionTierType::Light;
    let medium = SessionTierType::Medium;
    let heavy = SessionTierType::Heavy;

    assert_eq!(light.as_str(), "Light");
    assert_eq!(medium.as_str(), "Medium");
    assert_eq!(heavy.as_str(), "Heavy");
}

// ============================================================================
// Pool Error Handling Tests
// ============================================================================

#[test]
fn test_release_invalid_session() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    // Try to release a session that was never allocated
    let fake_id = SessionId(0xDEADBEEF);
    let result = pool.release_session(fake_id);

    // Should return an error
    assert!(result.is_err());
}

#[test]
fn test_double_release() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    let session_id = pool.allocate_session(SessionTierType::Light).unwrap();

    // First release should succeed
    assert!(pool.release_session(session_id).is_ok());

    // Second release should fail (AlreadyReleased)
    let result = pool.release_session(session_id);
    assert!(matches!(result, Err(PoolError::AlreadyReleased(_))));
}

// ============================================================================
// Pool Configuration Tests
// ============================================================================

#[test]
fn test_pool_config_default() {
    let config = PoolConfig::default();

    // Default capacities should be reasonable
    assert!(config.light_capacity >= 16);
    assert!(config.medium_capacity >= 8);
    assert!(config.heavy_capacity >= 4);
}

#[test]
fn test_pool_peak_tracking() {
    let config = PoolConfig::default();
    let pool = SessionPoolCapsule::new(config);

    // Allocate several sessions
    let s1 = pool.allocate_session(SessionTierType::Light).unwrap();
    let s2 = pool.allocate_session(SessionTierType::Light).unwrap();
    let s3 = pool.allocate_session(SessionTierType::Light).unwrap();

    let stats = pool.get_pool_stats();
    assert_eq!(stats.peak_light, 3);

    // Release some
    pool.release_session(s1).unwrap();
    pool.release_session(s2).unwrap();

    // Peak should still be 3
    let stats = pool.get_pool_stats();
    assert_eq!(stats.peak_light, 3);
    assert_eq!(stats.light_used, 1);

    // Allocate more, peak should update
    let _s4 = pool.allocate_session(SessionTierType::Light).unwrap();
    let _s5 = pool.allocate_session(SessionTierType::Light).unwrap();
    let _s6 = pool.allocate_session(SessionTierType::Light).unwrap();
    let _s7 = pool.allocate_session(SessionTierType::Light).unwrap();

    let stats = pool.get_pool_stats();
    assert_eq!(stats.peak_light, 5); // s3 + 4 new = 5

    let _ = s3; // Keep s3 alive
}
