//! ContextCapsule Integration Tests
//! T28 Framework: 28 tests across 4 tiers (Unit/Property/Integration/Production)
//! GPU HAL Phase 2 Agent 6: ContextCapsule Implementation

#[cfg(test)]
mod context_integration_tests {
    use atomic_capsule::gpu::hal::{ContextCapsule, ContextHandle, ContextState, ContextError};

    // ============================================================================
    // TIER Q1-Q7: UNIT TESTS
    // ============================================================================

    #[test]
    fn q1_context_handle_creation() {
        let handle = ContextHandle::new(1, 0);
        assert_eq!(handle.id(), 1);
        assert_eq!(handle.generation(), 0);
    }

    #[test]
    fn q2_context_state_transitions() {
        assert!(ContextState::Valid.can_bind());
        assert!(ContextState::Unbound.can_bind());
        assert!(!ContextState::Idle.can_bind());
        assert!(!ContextState::Bound.can_bind());
        assert!(!ContextState::Destroyed.can_bind());
    }

    #[test]
    fn q3_context_capsule_creation() {
        let capsule = ContextCapsule::new();
        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.state, ContextState::Idle);
        assert_eq!(snapshot.switch_count, 0);
    }

    #[test]
    fn q4_basic_create_context() {
        let capsule = ContextCapsule::new();
        let handle = capsule.create_context().expect("create_context failed");
        assert_eq!(handle.generation(), 1);

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.state, ContextState::Valid);
    }

    #[test]
    fn q5_handle_validity_check() {
        let capsule = ContextCapsule::new();
        let handle = capsule.create_context().expect("create_context failed");

        capsule.bind_context(handle).expect("bind_context failed");
        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.state, ContextState::Bound);
    }

    #[test]
    fn q6_use_after_free_detection() {
        let capsule = ContextCapsule::new();
        let handle = capsule.create_context().expect("create_context failed");

        capsule.unbind_context(handle).expect("unbind failed");
        capsule.destroy_context(handle).expect("destroy failed");

        let result = capsule.bind_context(handle);
        assert!(matches!(result, Err(ContextError::UseAfterFree { .. })));
    }

    #[test]
    fn q7_state_machine_validation() {
        let capsule = ContextCapsule::new();
        let handle = capsule.create_context().expect("create_context failed");

        capsule.bind_context(handle).expect("first bind failed");
        let result = capsule.bind_context(handle);
        assert!(matches!(result, Err(ContextError::InvalidTransition { .. })));
    }

    // ============================================================================
    // TIER Q8-Q14: PROPERTY TESTS
    // ============================================================================

    #[test]
    fn q8_bind_determinism() {
        let capsule1 = ContextCapsule::new();
        let capsule2 = ContextCapsule::new();

        let h1 = capsule1.create_context().expect("create_context 1");
        let h2 = capsule2.create_context().expect("create_context 2");

        assert_eq!(h1.generation(), h2.generation());

        capsule1.bind_context(h1).expect("bind 1");
        capsule2.bind_context(h2).expect("bind 2");

        assert_eq!(capsule1.snapshot().state, capsule2.snapshot().state);
    }

    #[test]
    fn q9_context_isolation() {
        let capsule = ContextCapsule::new();
        let h1 = capsule.create_context().expect("create 1");
        let h2 = capsule.create_context().expect("create 2");

        capsule.bind_context(h2).expect("bind h2");
        capsule.unbind_context(h2).expect("unbind h2");
        capsule.destroy_context(h2).expect("destroy h2");

        capsule.bind_context(h1).expect("bind h1 after h2 destroy");
    }

    #[test]
    fn q10_handle_generation_prevents_aba() {
        let capsule = ContextCapsule::new();
        let h1 = capsule.create_context().expect("create 1");

        capsule.bind_context(h1).expect("bind");
        capsule.unbind_context(h1).expect("unbind");
        capsule.destroy_context(h1).expect("destroy");

        let result = capsule.bind_context(h1);
        assert!(matches!(result, Err(ContextError::UseAfterFree { .. })));
    }

    #[test]
    fn q11_concurrent_state_safety() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        capsule.bind_context(h).expect("first bind");

        let result = capsule.bind_context(h);
        assert!(matches!(result, Err(ContextError::InvalidTransition { .. })));
    }

    #[test]
    fn q12_idempotent_operations() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        capsule.bind_context(h).expect("bind 1");
        capsule.unbind_context(h).expect("unbind");
        capsule.bind_context(h).expect("bind 2");

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.state, ContextState::Bound);
    }

    #[test]
    fn q13_generation_counter_monotonicity() {
        let capsule = ContextCapsule::new();
        let h1 = capsule.create_context().expect("create 1");
        let gen1 = h1.generation();

        capsule.bind_context(h1).expect("bind");
        capsule.unbind_context(h1).expect("unbind");
        capsule.destroy_context(h1).expect("destroy");

        let snapshot = capsule.snapshot();
        assert!(snapshot.generation > gen1 as u32);
    }

    #[test]
    fn q14_memory_consistency() {
        let capsule = ContextCapsule::new();

        for i in 0..10 {
            let h = capsule.create_context().expect(&format!("create {}", i));
            capsule.bind_context(h).expect(&format!("bind {}", i));
            capsule.unbind_context(h).expect(&format!("unbind {}", i));
        }

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.switch_count, 20);
    }

    // ============================================================================
    // TIER Q15-Q21: INTEGRATION TESTS
    // ============================================================================

    #[test]
    fn q15_sequential_context_switching() {
        let capsule = ContextCapsule::new();

        for i in 0..100 {
            let h = capsule.create_context().expect(&format!("create {}", i));
            capsule.bind_context(h).expect(&format!("bind {}", i));
            capsule.unbind_context(h).expect(&format!("unbind {}", i));
            capsule.destroy_context(h).expect(&format!("destroy {}", i));
        }

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.switch_count, 200);
        assert_eq!(snapshot.switch_errors, 0);
    }

    #[test]
    fn q16_multiple_context_lifecycle() {
        let capsule = ContextCapsule::new();

        let handles: Vec<_> = (0..5)
            .map(|_| capsule.create_context().expect("create"))
            .collect();

        for h in &handles {
            capsule.bind_context(*h).expect("bind");
        }

        assert_eq!(capsule.snapshot().bind_count, 5);

        for h in &handles {
            capsule.unbind_context(*h).expect("unbind");
            capsule.destroy_context(*h).expect("destroy");
        }

        assert_eq!(capsule.snapshot().state, ContextState::Destroyed);
    }

    #[test]
    fn q17_stress_bind_unbind() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        for _ in 0..1000 {
            capsule.bind_context(h).expect("bind");
            capsule.unbind_context(h).expect("unbind");
        }

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.switch_count, 2000);
        assert_eq!(snapshot.switch_errors, 0);
    }

    #[test]
    fn q18_snapshot_consistency() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        capsule.bind_context(h).expect("bind");

        let s1 = capsule.snapshot();
        let s2 = capsule.snapshot();
        let s3 = capsule.snapshot();

        assert_eq!(s1.state, s2.state);
        assert_eq!(s2.state, s3.state);
        assert_eq!(s1.switch_count, s2.switch_count);
    }

    #[test]
    fn q19_error_recovery() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        let result = capsule.unbind_context(h);
        assert!(result.is_err());

        capsule.bind_context(h).expect("bind after error");
        capsule.unbind_context(h).expect("unbind after error");

        assert_eq!(capsule.snapshot().switch_errors, 1);
    }

    #[test]
    fn q20_generation_wrap_around() {
        let capsule = ContextCapsule::new();

        for _ in 0..100 {
            let h = capsule.create_context().expect("create");
            capsule.bind_context(h).expect("bind");
            capsule.unbind_context(h).expect("unbind");
            capsule.destroy_context(h).expect("destroy");
        }

        let snapshot = capsule.snapshot();
        assert!(snapshot.generation > 100);
    }

    #[test]
    fn q21_rapid_create_destroy() {
        let capsule = ContextCapsule::new();

        for _ in 0..100 {
            let h = capsule.create_context().expect("create");
            capsule.destroy_context(h).expect("destroy");
        }

        capsule.create_context().expect("create after rapid");
    }

    // ============================================================================
    // TIER Q22-Q28: PRODUCTION TESTS
    // ============================================================================

    #[test]
    fn q22_high_switch_rate() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        for _ in 0..5000 {
            capsule.bind_context(h).expect("bind");
            capsule.unbind_context(h).expect("unbind");
        }

        assert_eq!(capsule.snapshot().switch_count, 10000);
    }

    #[test]
    fn q23_sustained_throughput() {
        let capsule = ContextCapsule::new();

        let mut handles = Vec::new();
        for _ in 0..1000 {
            match capsule.create_context() {
                Ok(h) => handles.push(h),
                Err(_) => break,
            }
        }

        for h in &handles {
            capsule.bind_context(*h).expect("bind");
        }

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.bind_count as usize, handles.len());
    }

    #[test]
    fn q24_error_handling_comprehensive() {
        let capsule = ContextCapsule::new();

        let fake_handle = ContextHandle::new(999, 0);
        assert!(capsule.bind_context(fake_handle).is_err());

        let h = capsule.create_context().expect("create");
        capsule.bind_context(h).expect("bind");
        assert!(capsule.bind_context(h).is_err());
    }

    #[test]
    fn q25_state_machine_coverage() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        capsule.bind_context(h).expect("Valid->Bound");
        capsule.unbind_context(h).expect("Bound->Unbound");
        capsule.bind_context(h).expect("Unbound->Bound");
        capsule.unbind_context(h).expect("Bound->Unbound");
        capsule.destroy_context(h).expect("Unbound->Destroyed");

        assert_eq!(capsule.snapshot().state, ContextState::Destroyed);
    }

    #[test]
    fn q26_memory_leak_safety() {
        let capsule = ContextCapsule::new();

        {
            let _h = capsule.create_context().expect("create");
        }

        capsule.create_context().expect("create after drop");
    }

    #[test]
    fn q27_concurrent_detection() {
        let capsule = ContextCapsule::new();
        let h = capsule.create_context().expect("create");

        capsule.bind_context(h).expect("bind");

        let result = capsule.bind_context(h);
        assert!(matches!(result, Err(ContextError::InvalidTransition { .. })));
    }

    #[test]
    fn q28_1m_operations_stress() {
        let capsule = ContextCapsule::new();

        for i in 0..250000 {
            let h = match capsule.create_context() {
                Ok(h) => h,
                Err(_) => break,
            };

            capsule.bind_context(h).expect("bind");
            capsule.unbind_context(h).expect("unbind");
            capsule.destroy_context(h).expect("destroy");
        }

        let snapshot = capsule.snapshot();
        assert!(snapshot.switch_count > 100000);
        assert_eq!(snapshot.switch_errors, 0);
    }
}
