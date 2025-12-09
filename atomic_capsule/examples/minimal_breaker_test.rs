// Minimal Circuit Breaker Test
// This example tests ONLY the circuit breaker integration without other dependencies

fn main() {
    #[cfg(all(feature = "std", feature = "circuit-breaker-standard64"))]
    {
        use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State};
        use core::sync::atomic::AtomicU64;

        println!("=== Minimal Circuit Breaker Test ===\n");

        // Test 1: Create breakers
        let breaker_rest = CircuitBreaker::new(State::Closed);
        let breaker_graphql = CircuitBreaker::new(State::Closed);
        println!("✓ Created 2 circuit breakers");

        // Test 2: Check states
        assert_eq!(breaker_rest.state(), State::Closed);
        assert_eq!(breaker_graphql.state(), State::Closed);
        println!("✓ Initial states are Closed");

        // Test 3: Open circuit
        breaker_rest.open();
        assert_eq!(breaker_rest.state(), State::Open);
        println!("✓ Opened REST circuit");

        // Test 4: Other breaker unaffected
        assert_eq!(breaker_graphql.state(), State::Closed);
        println!("✓ GraphQL circuit still Closed (isolation verified)");

        // Test 5: Close circuit
        breaker_rest.close();
        assert_eq!(breaker_rest.state(), State::Closed);
        println!("✓ Closed REST circuit");

        // Test 6: Half-open state
        breaker_rest.half_open();
        assert_eq!(breaker_rest.state(), State::HalfOpen);
        println!("✓ Transitioned to HalfOpen");

        // Test 7: Force open
        breaker_rest.force_open();
        assert_eq!(breaker_rest.state(), State::ForcedOpen);
        println!("✓ Force-opened circuit (admin override)");

        println!("\n=== All Tests Passed ✓ ===");
        println!("Circuit breakers are working correctly!");
    }

    #[cfg(not(all(feature = "std", feature = "circuit-breaker-standard64")))]
    {
        eprintln!("Error: This example requires features: std,circuit-breaker-standard64");
        eprintln!("Run: cargo run --example minimal_breaker_test --features \"std,circuit-breaker-standard64\"");
    }
}
