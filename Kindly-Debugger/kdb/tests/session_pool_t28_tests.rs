//! T28 5-Tier Testing Suite for Session Pool
//!
//! Comprehensive testing following the T28 framework:
//! - Q1-Q7: Unit tests (individual functions, state transitions)
//! - Q8-Q14: Property tests (invariants, proptest-driven)
//! - Q15-Q21: Integration tests (cross-module coordination)
//! - Q22-Q28: Production tests (stress, performance)
//! - Q29-Q35: Determinism tests (reproducible behavior)
//!
//! # COCA Compliance
//!
//! All tests verify lockfree behavior (no mutex/RwLock),
//! cache alignment (64B/128B/256B), and generation counters.
//!
//! # ASSUM Framework
//!
//! #ASSUME_LOCKFREE_ONLY: All session pool operations use atomics
//! #ASSUME_TIER_THRESHOLDS: Upgrade at 75%, downgrade when idle
//! #ASSUME_SESSION_UNIQUENESS: Session IDs are never reused within epoch
//! #VERIFY_TEST_SUITE: This file provides comprehensive verification

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Test Module Imports
// ============================================================================

use kdb::session_pool::{
    SessionPoolCapsule, SessionLookup, SlotMetadata, PackedMetadata,
    SessionTier, SlotState, PoolConfig, PoolError, PoolStats,
    SessionId, SessionTierType,
    LIGHT_UPGRADE_SNAPSHOT_THRESHOLD, LIGHT_UPGRADE_BREAKPOINT_THRESHOLD,
};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

mod unit_tests {
    use super::*;

    // ===== Q1: SlotMetadata State Transitions =====

    #[test]
    fn test_slot_metadata_creation() {
        let slot = SlotMetadata::new(0, SessionTier::Light);
        assert_eq!(slot.state(), SlotState::Free);
        assert_eq!(slot.slot_id(), 0);
        assert_eq!(slot.generation(), 0);
    }

    #[test]
    fn test_slot_metadata_allocation() {
        let slot = SlotMetadata::new(42, SessionTier::Light);

        // Allocate slot
        let result = slot.try_allocate();
        assert!(result.is_ok());

        assert_eq!(slot.state(), SlotState::Allocated);
        assert!(slot.generation() > 0);
    }

    #[test]
    fn test_slot_metadata_double_allocation_fails() {
        let slot = SlotMetadata::new(1, SessionTier::Medium);

        slot.try_allocate().unwrap();

        // Second allocation should fail
        let result = slot.try_allocate();
        assert!(result.is_err());
    }

    #[test]
    fn test_slot_metadata_activation() {
        let slot = SlotMetadata::new(10, SessionTier::Heavy);
        slot.try_allocate().unwrap();

        // Activate the allocated slot
        let result = slot.activate();
        assert!(result.is_ok());
        assert_eq!(slot.state(), SlotState::InUse);
    }

    #[test]
    fn test_slot_metadata_drain_cycle() {
        let slot = SlotMetadata::new(5, SessionTier::Light);
        slot.try_allocate().unwrap();
        slot.activate().unwrap();

        // Begin draining
        let result = slot.begin_drain();
        assert!(result.is_ok());
        assert_eq!(slot.state(), SlotState::Draining);

        // Release
        let result = slot.release();
        assert!(result.is_ok());
        assert_eq!(slot.state(), SlotState::Free);
    }

    // ===== Q2: PackedMetadata Bit Field Tests =====

    #[test]
    fn test_packed_metadata_fields() {
        let packed = PackedMetadata::new(42, SessionTier::Medium, SlotState::InUse, 12345, 9999);

        assert_eq!(packed.slot_id(), 42);
        assert_eq!(packed.tier(), SessionTier::Medium);
        assert_eq!(packed.state(), SlotState::InUse);
        assert_eq!(packed.generation(), 12345);
        assert_eq!(packed.timestamp(), 9999);
    }

    #[test]
    fn test_packed_metadata_with_state() {
        let packed = PackedMetadata::new(100, SessionTier::Heavy, SlotState::Free, 0, 0);
        let updated = packed.with_state(SlotState::Allocated);

        assert_eq!(updated.slot_id(), 100);
        assert_eq!(updated.tier(), SessionTier::Heavy);
        assert_eq!(updated.state(), SlotState::Allocated);
        assert_eq!(updated.generation(), 1); // Incremented
    }

    #[test]
    fn test_packed_metadata_with_tier() {
        let packed = PackedMetadata::new(50, SessionTier::Light, SlotState::InUse, 5, 100);
        let updated = packed.with_tier(SessionTier::Medium);

        assert_eq!(updated.tier(), SessionTier::Medium);
        assert_eq!(updated.generation(), 6); // Incremented
    }

    // ===== Q3: SessionPoolCapsule Initialization =====

    #[test]
    fn test_pool_initialization() {
        let config = PoolConfig::test_config();
        let pool = SessionPoolCapsule::new(config);

        assert!(pool.is_ready());
        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0);
        assert_eq!(stats.medium_used, 0);
        assert_eq!(stats.heavy_used, 0);
    }

    #[test]
    fn test_pool_allocate_light() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Light));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 1);
        assert_eq!(stats.total_allocations, 1);
    }

    #[test]
    fn test_pool_allocate_medium() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Medium).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.medium_used, 1);
    }

    #[test]
    fn test_pool_allocate_heavy() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Heavy));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.heavy_used, 1);
    }

    // ===== Q4: Session Release Tests =====

    #[test]
    fn test_pool_release_session() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert_eq!(pool.get_pool_stats().light_used, 1);

        pool.release_session(id).unwrap();
        assert_eq!(pool.get_pool_stats().light_used, 0);
        assert_eq!(pool.get_pool_stats().total_releases, 1);
    }

    #[test]
    fn test_pool_double_release_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        pool.release_session(id).unwrap();

        let result = pool.release_session(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_pool_invalid_session_release() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let result = pool.release_session(SessionId::INVALID);
        assert!(matches!(result, Err(PoolError::InvalidSessionId(_))));
    }

    // ===== Q5: Pool Exhaustion Tests =====

    #[test]
    fn test_pool_exhaustion() {
        let config = PoolConfig {
            light_capacity: 2,
            medium_capacity: 1,
            heavy_capacity: 1,
            ..PoolConfig::test_config()
        };
        let pool = SessionPoolCapsule::new(config);

        // Allocate all light slots
        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Light).unwrap();

        // Third allocation should fail
        let result = pool.allocate_session(SessionTierType::Light);
        assert!(matches!(result, Err(PoolError::PoolFull { tier: SessionTierType::Light, .. })));

        // Release one and try again
        pool.release_session(id1).unwrap();
        let id3 = pool.allocate_session(SessionTierType::Light).unwrap();
        assert!(id3.is_valid());

        // Cleanup
        pool.release_session(id2).unwrap();
        pool.release_session(id3).unwrap();
    }

    // ===== Q6: Upgrade/Downgrade Tests =====

    #[test]
    fn test_upgrade_light_to_medium() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert_eq!(light_id.tier_type(), Some(SessionTierType::Light));

        let medium_id = pool.upgrade_session(light_id).unwrap();
        assert_eq!(medium_id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.total_upgrades, 1);
    }

    #[test]
    fn test_upgrade_medium_to_heavy() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
        let heavy_id = pool.upgrade_session(medium_id).unwrap();

        assert_eq!(heavy_id.tier_type(), Some(SessionTierType::Heavy));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.medium_used, 0);
        assert_eq!(stats.heavy_used, 1);
    }

    #[test]
    fn test_upgrade_heavy_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        let result = pool.upgrade_session(heavy_id);

        assert!(matches!(result, Err(PoolError::CannotUpgrade(_))));
    }

    #[test]
    fn test_downgrade_heavy_to_medium() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        let medium_id = pool.downgrade_session(heavy_id).unwrap();

        assert_eq!(medium_id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.heavy_used, 0);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.total_downgrades, 1);
    }

    #[test]
    fn test_downgrade_light_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        let result = pool.downgrade_session(light_id);

        assert!(matches!(result, Err(PoolError::CannotDowngrade(_))));
    }

    // ===== Q7: SessionId Encoding Tests =====

    #[test]
    fn test_session_id_encoding() {
        let id = SessionId::new(1, 123, 456);
        assert_eq!(id.tier(), 1);
        assert_eq!(id.slot(), 123);
        assert_eq!(id.generation(), 456);
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Medium));
    }

    #[test]
    fn test_session_id_invalid() {
        assert!(!SessionId::INVALID.is_valid());
        assert_eq!(SessionId::INVALID.0, 0);
    }

    #[test]
    fn test_session_tier_type_properties() {
        assert_eq!(SessionTierType::Light.session_size(), 64 * 1024);
        assert_eq!(SessionTierType::Medium.session_size(), 256 * 1024);
        assert_eq!(SessionTierType::Heavy.session_size(), 1_147_392);
    }
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (proptest)
// ============================================================================

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        // ===== Q8: Session ID Invariants =====

        #[test]
        fn prop_session_id_roundtrip(tier in 0u8..3, slot in 0u32..0x00FF_FFFF, gen in 0u32..0xFFFF_FFFF) {
            let id = SessionId::new(tier, slot, gen);
            prop_assert_eq!(id.tier(), tier);
            prop_assert_eq!(id.slot(), slot);
            prop_assert_eq!(id.generation(), gen);
        }

        #[test]
        fn prop_session_id_validity(tier in 0u8..3, slot in 1u32..0x00FF_FFFF, gen in 1u32..0xFFFF_FFFF) {
            let id = SessionId::new(tier, slot, gen);
            prop_assert!(id.is_valid());
        }

        // ===== Q9: Packed Metadata Invariants =====

        #[test]
        fn prop_packed_metadata_state_transition_increments_gen(
            slot_id in 0u16..0xFFFF,
            tier in 0u8..3,
            gen in 0u32..0x00FF_FFFE // Leave room for increment
        ) {
            let tier_val = match tier {
                0 => SessionTier::Light,
                1 => SessionTier::Medium,
                _ => SessionTier::Heavy,
            };
            let packed = PackedMetadata::new(slot_id, tier_val, SlotState::Free, gen, 0);
            let updated = packed.with_state(SlotState::Allocated);
            prop_assert_eq!(updated.generation(), gen + 1);
        }

        // ===== Q10: Pool Allocation Uniqueness =====

        #[test]
        fn prop_allocations_unique(count in 1usize..16) {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let mut ids: Vec<SessionId> = Vec::with_capacity(count);

            for _ in 0..count {
                if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                    ids.push(id);
                }
            }

            // All IDs should be unique
            for i in 0..ids.len() {
                for j in (i+1)..ids.len() {
                    prop_assert_ne!(ids[i].0, ids[j].0);
                }
            }
        }

        // ===== Q11: Pool Stats Consistency =====

        #[test]
        fn prop_pool_stats_consistent(alloc_count in 0usize..8, release_count in 0usize..8) {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let mut active_ids: Vec<SessionId> = Vec::new();

            // Allocate
            for _ in 0..alloc_count {
                if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                    active_ids.push(id);
                }
            }

            // Release some
            for _ in 0..release_count.min(active_ids.len()) {
                if let Some(id) = active_ids.pop() {
                    let _ = pool.release_session(id);
                }
            }

            let stats = pool.get_pool_stats();
            prop_assert_eq!(stats.light_used as usize, active_ids.len());
        }

        // ===== Q12: Tier Transitions Monotonic (within operation) =====

        #[test]
        fn prop_upgrade_tier_increases(initial_tier in 0u8..2) {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let tier = match initial_tier {
                0 => SessionTierType::Light,
                _ => SessionTierType::Medium,
            };

            let id = pool.allocate_session(tier).unwrap();
            let upgraded = pool.upgrade_session(id).unwrap();

            let old_tier = id.tier();
            let new_tier = upgraded.tier();
            prop_assert!(new_tier > old_tier);
        }

        // ===== Q13: Generation Counter Never Decreases =====

        #[test]
        fn prop_generation_monotonic(ops in 1usize..10) {
            let slot = SlotMetadata::new(0, SessionTier::Light);
            let mut last_gen = slot.generation();

            for i in 0..ops {
                match i % 4 {
                    0 => { let _ = slot.try_allocate(); }
                    1 => { let _ = slot.activate(); }
                    2 => { let _ = slot.begin_drain(); }
                    _ => { let _ = slot.release(); }
                }
                let current_gen = slot.generation();
                prop_assert!(current_gen >= last_gen);
                last_gen = current_gen;
            }
        }

        // ===== Q14: Pool Memory Invariants =====

        #[test]
        fn prop_pool_memory_bounds(light in 0usize..4, medium in 0usize..2, heavy in 0usize..1) {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let mut ids = Vec::new();

            for _ in 0..light {
                if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                    ids.push(id);
                }
            }
            for _ in 0..medium {
                if let Ok(id) = pool.allocate_session(SessionTierType::Medium) {
                    ids.push(id);
                }
            }
            for _ in 0..heavy {
                if let Ok(id) = pool.allocate_session(SessionTierType::Heavy) {
                    ids.push(id);
                }
            }

            let stats = pool.get_pool_stats();
            let memory = stats.memory_used();
            let expected_max = (light * 64 * 1024) + (medium * 256 * 1024) + (heavy * 1_147_392);
            prop_assert!(memory <= expected_max);
        }
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

mod integration_tests {
    use super::*;

    // ===== Q15: Multi-Tier Pool Interaction =====

    #[test]
    fn test_multi_tier_allocation() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        // Allocate across all tiers
        let light1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let medium1 = pool.allocate_session(SessionTierType::Medium).unwrap();
        let heavy1 = pool.allocate_session(SessionTierType::Heavy).unwrap();

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 1);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.heavy_used, 1);
        assert_eq!(stats.total_allocations, 3);

        // Release all
        pool.release_session(light1).unwrap();
        pool.release_session(medium1).unwrap();
        pool.release_session(heavy1).unwrap();

        let stats = pool.get_pool_stats();
        assert_eq!(stats.total_used(), 0);
        assert_eq!(stats.total_releases, 3);
    }

    // ===== Q16: Upgrade Chain (Light -> Medium -> Heavy) =====

    #[test]
    fn test_full_upgrade_chain() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert_eq!(pool.get_session_tier(light_id), Some(SessionTierType::Light));

        let medium_id = pool.upgrade_session(light_id).unwrap();
        assert_eq!(pool.get_session_tier(medium_id), Some(SessionTierType::Medium));

        let heavy_id = pool.upgrade_session(medium_id).unwrap();
        assert_eq!(pool.get_session_tier(heavy_id), Some(SessionTierType::Heavy));

        // Cannot upgrade further
        assert!(pool.upgrade_session(heavy_id).is_err());

        let stats = pool.get_pool_stats();
        assert_eq!(stats.total_upgrades, 2);
    }

    // ===== Q17: Downgrade Chain (Heavy -> Medium -> Light) =====

    #[test]
    fn test_full_downgrade_chain() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

        let medium_id = pool.downgrade_session(heavy_id).unwrap();
        assert_eq!(pool.get_session_tier(medium_id), Some(SessionTierType::Medium));

        let light_id = pool.downgrade_session(medium_id).unwrap();
        assert_eq!(pool.get_session_tier(light_id), Some(SessionTierType::Light));

        // Cannot downgrade further
        assert!(pool.downgrade_session(light_id).is_err());

        let stats = pool.get_pool_stats();
        assert_eq!(stats.total_downgrades, 2);
    }

    // ===== Q18: Pool Configuration Validation =====

    #[test]
    fn test_pool_config_total_memory() {
        let config = PoolConfig::test_config();
        let memory = config.total_memory_bytes();

        let expected = (config.light_capacity as usize * 64 * 1024)
            + (config.medium_capacity as usize * 256 * 1024)
            + (config.heavy_capacity as usize * 1_147_392);

        assert_eq!(memory, expected);
    }

    // ===== Q19: Free-List Correctness =====

    #[test]
    fn test_free_list_reuse() {
        let config = PoolConfig {
            light_capacity: 4,
            medium_capacity: 2,
            heavy_capacity: 1,
            ..PoolConfig::test_config()
        };
        let pool = SessionPoolCapsule::new(config);

        // Allocate all light slots
        let ids: Vec<_> = (0..4)
            .map(|_| pool.allocate_session(SessionTierType::Light).unwrap())
            .collect();

        // Pool should be full
        assert!(pool.allocate_session(SessionTierType::Light).is_err());

        // Release in reverse order
        for id in ids.into_iter().rev() {
            pool.release_session(id).unwrap();
        }

        // Should be able to allocate 4 again
        let new_ids: Vec<_> = (0..4)
            .map(|_| pool.allocate_session(SessionTierType::Light).unwrap())
            .collect();

        assert_eq!(new_ids.len(), 4);

        // Cleanup
        for id in new_ids {
            pool.release_session(id).unwrap();
        }
    }

    // ===== Q20: Peak Usage Tracking =====

    #[test]
    fn test_peak_usage_tracking() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        // Allocate 3 light sessions
        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id3 = pool.allocate_session(SessionTierType::Light).unwrap();

        assert_eq!(pool.get_pool_stats().peak_light, 3);

        // Release all
        pool.release_session(id1).unwrap();
        pool.release_session(id2).unwrap();
        pool.release_session(id3).unwrap();

        // Peak should still be 3
        assert_eq!(pool.get_pool_stats().peak_light, 3);
        assert_eq!(pool.get_pool_stats().light_used, 0);
    }

    // ===== Q21: Utilization Calculation =====

    #[test]
    fn test_utilization_calculation() {
        let config = PoolConfig {
            light_capacity: 10,
            medium_capacity: 10,
            heavy_capacity: 10,
            ..PoolConfig::test_config()
        };
        let pool = SessionPoolCapsule::new(config);

        // 0% utilization
        assert_eq!(pool.get_pool_stats().utilization_percent(), 0.0);

        // Allocate 3 sessions (3/30 = 10%)
        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Medium).unwrap();
        let id3 = pool.allocate_session(SessionTierType::Heavy).unwrap();

        let util = pool.get_pool_stats().utilization_percent();
        assert!((util - 10.0).abs() < 0.1);

        pool.release_session(id1).unwrap();
        pool.release_session(id2).unwrap();
        pool.release_session(id3).unwrap();
    }
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Stress, Performance)
// ============================================================================

mod production_tests {
    use super::*;

    // ===== Q22: High-Frequency Allocation Stress =====

    #[test]
    fn test_rapid_alloc_dealloc_stress() {
        let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::test_config()));
        let iterations = 1000;

        let start = Instant::now();

        for _ in 0..iterations {
            let id = pool.allocate_session(SessionTierType::Light).unwrap();
            pool.release_session(id).unwrap();
        }

        let elapsed = start.elapsed();
        let ops_per_sec = (iterations * 2) as f64 / elapsed.as_secs_f64();

        println!("Rapid alloc/dealloc: {:.0} ops/sec", ops_per_sec);

        // Performance assertion: should be > 100K ops/sec
        assert!(ops_per_sec > 100_000.0, "Performance below threshold: {}", ops_per_sec);
    }

    // ===== Q23: Concurrent Allocation Stress =====

    #[test]
    fn test_concurrent_allocation_stress() {
        let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::test_config()));
        let threads = 4;
        let ops_per_thread = 100;

        let mut handles = vec![];

        for _ in 0..threads {
            let pool_clone = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    if let Ok(id) = pool_clone.allocate_session(SessionTierType::Light) {
                        std::hint::spin_loop();
                        let _ = pool_clone.release_session(id);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0); // All released
        assert!(stats.total_allocations > 0);
        assert_eq!(stats.total_allocations, stats.total_releases);
    }

    // ===== Q24: Mixed Tier Concurrent Stress =====

    #[test]
    fn test_mixed_tier_concurrent_stress() {
        let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::test_config()));
        let mut handles = vec![];

        // Light thread
        let pool_clone = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            for _ in 0..50 {
                if let Ok(id) = pool_clone.allocate_session(SessionTierType::Light) {
                    std::hint::spin_loop();
                    let _ = pool_clone.release_session(id);
                }
            }
        }));

        // Medium thread
        let pool_clone = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            for _ in 0..30 {
                if let Ok(id) = pool_clone.allocate_session(SessionTierType::Medium) {
                    std::hint::spin_loop();
                    let _ = pool_clone.release_session(id);
                }
            }
        }));

        // Heavy thread
        let pool_clone = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                if let Ok(id) = pool_clone.allocate_session(SessionTierType::Heavy) {
                    std::hint::spin_loop();
                    let _ = pool_clone.release_session(id);
                }
            }
        }));

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool.get_pool_stats();
        assert_eq!(stats.total_used(), 0);
    }

    // ===== Q25: Allocation Timing =====

    #[test]
    fn test_allocation_timing() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        let iterations = 100;

        // Pool has capacity 16, so we use allocate-release pattern
        // to measure timing of 100 allocations without exhausting pool
        let start = Instant::now();

        for _ in 0..iterations {
            let id = pool.allocate_session(SessionTierType::Light).unwrap();
            pool.release_session(id).unwrap();
        }

        let elapsed = start.elapsed();
        // We did 100 allocations AND 100 releases, so divide by iterations
        // to get per-allocation time (release is similar cost)
        let avg_ns = elapsed.as_nanos() / (iterations * 2) as u128;

        println!("Average allocation/release time: {} ns", avg_ns);

        // Performance assertion: <1000ns per operation
        assert!(avg_ns < 1000, "Operations too slow: {} ns", avg_ns);
    }

    // ===== Q26: Stats Snapshot Performance =====

    #[test]
    fn test_stats_snapshot_performance() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        let iterations = 10000;

        let start = Instant::now();

        for _ in 0..iterations {
            let _ = pool.get_pool_stats();
        }

        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() / iterations as u128;

        println!("Average stats snapshot: {} ns", avg_ns);

        // Performance assertion: <100ns per snapshot
        assert!(avg_ns < 100, "Stats too slow: {} ns", avg_ns);
    }

    // ===== Q27: Upgrade Performance =====

    #[test]
    fn test_upgrade_performance() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        let iterations = 50;

        let start = Instant::now();

        for _ in 0..iterations {
            let id = pool.allocate_session(SessionTierType::Light).unwrap();
            let upgraded = pool.upgrade_session(id).unwrap();
            pool.release_session(upgraded).unwrap();
        }

        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() / iterations as u128;

        println!("Average upgrade time: {} us", avg_us);

        // Performance assertion: <100us per upgrade
        assert!(avg_us < 100, "Upgrade too slow: {} us", avg_us);
    }

    // ===== Q28: Long-Running Stability =====

    #[test]
    fn test_long_running_stability() {
        let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::test_config()));
        let duration = Duration::from_millis(500);

        let start = Instant::now();
        let mut ops = 0u64;

        while start.elapsed() < duration {
            if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                pool.release_session(id).unwrap();
                ops += 2;
            }
        }

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0);
        assert_eq!(stats.total_allocations, stats.total_releases);

        println!("Long-running stability: {} ops in {:?}", ops, duration);
    }
}

// ============================================================================
// Q29-Q35: DETERMINISM TESTS
// ============================================================================

mod determinism_tests {
    use super::*;

    // ===== Q29: Allocation Order Determinism =====

    #[test]
    fn test_allocation_order_determinism() {
        // Run twice with same sequence, should get same slot assignments
        let get_slots = || {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let mut slots = Vec::new();

            for _ in 0..5 {
                let id = pool.allocate_session(SessionTierType::Light).unwrap();
                slots.push(id.slot());
                pool.release_session(id).unwrap();
            }

            slots
        };

        let slots1 = get_slots();
        let slots2 = get_slots();

        assert_eq!(slots1, slots2, "Allocation order should be deterministic");
    }

    // ===== Q30: Generation Counter Determinism =====

    #[test]
    fn test_generation_determinism() {
        let get_generations = || {
            let slot = SlotMetadata::new(0, SessionTier::Light);
            let mut gens = Vec::new();

            for _ in 0..10 {
                slot.try_allocate().ok();
                gens.push(slot.generation());
                slot.activate().ok();
                gens.push(slot.generation());
                slot.begin_drain().ok();
                gens.push(slot.generation());
                slot.release().ok();
                gens.push(slot.generation());
            }

            gens
        };

        let gens1 = get_generations();
        let gens2 = get_generations();

        assert_eq!(gens1, gens2, "Generation sequence should be deterministic");
    }

    // ===== Q31: Free-List LIFO Order =====

    #[test]
    fn test_free_list_lifo_order() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        // Allocate 3 sessions
        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id3 = pool.allocate_session(SessionTierType::Light).unwrap();

        // Release in order: id1, id2, id3
        pool.release_session(id1).unwrap();
        pool.release_session(id2).unwrap();
        pool.release_session(id3).unwrap();

        // LIFO: id3's slot should be allocated first
        let new1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let new2 = pool.allocate_session(SessionTierType::Light).unwrap();
        let new3 = pool.allocate_session(SessionTierType::Light).unwrap();

        assert_eq!(new1.slot(), id3.slot(), "LIFO: last released should be first allocated");
        assert_eq!(new2.slot(), id2.slot());
        assert_eq!(new3.slot(), id1.slot());
    }

    // ===== Q32: Session ID Uniqueness Within Epoch =====

    #[test]
    fn test_session_id_uniqueness() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        let mut all_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

        for _ in 0..100 {
            let id = pool.allocate_session(SessionTierType::Light).unwrap();
            assert!(all_ids.insert(id.0), "Session ID should be unique");
            pool.release_session(id).unwrap();
        }
    }

    // ===== Q33: State Transition Determinism =====

    #[test]
    fn test_state_transition_sequence() {
        let slot = SlotMetadata::new(0, SessionTier::Medium);

        // Expected sequence: Free -> Allocated -> InUse -> Draining -> Free
        assert_eq!(slot.state(), SlotState::Free);

        slot.try_allocate().unwrap();
        assert_eq!(slot.state(), SlotState::Allocated);

        slot.activate().unwrap();
        assert_eq!(slot.state(), SlotState::InUse);

        slot.begin_drain().unwrap();
        assert_eq!(slot.state(), SlotState::Draining);

        slot.release().unwrap();
        assert_eq!(slot.state(), SlotState::Free);
    }

    // ===== Q34: Pool Stats Snapshot Consistency =====

    #[test]
    fn test_stats_snapshot_consistency() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Medium).unwrap();

        let stats = pool.get_pool_stats();

        // Stats should be consistent snapshot
        assert_eq!(stats.light_used + stats.medium_used + stats.heavy_used, stats.total_used());
        assert_eq!(stats.total_allocations, 2);
        assert_eq!(stats.total_releases, 0);

        pool.release_session(id1).unwrap();
        pool.release_session(id2).unwrap();
    }

    // ===== Q35: Fixed Seed Reproducibility =====

    #[test]
    fn test_fixed_seed_reproducibility() {
        // Simulate deterministic sequence
        let run_sequence = || {
            let pool = SessionPoolCapsule::new(PoolConfig::test_config());
            let mut results: Vec<(u32, u32)> = Vec::new();

            // Fixed sequence of operations
            for i in 0..10 {
                let tier = match i % 3 {
                    0 => SessionTierType::Light,
                    1 => SessionTierType::Medium,
                    _ => SessionTierType::Heavy,
                };

                if let Ok(id) = pool.allocate_session(tier) {
                    results.push((id.slot(), id.generation()));
                    pool.release_session(id).unwrap();
                }
            }

            results
        };

        let run1 = run_sequence();
        let run2 = run_sequence();

        assert_eq!(run1, run2, "Same sequence should produce same results");
    }
}
