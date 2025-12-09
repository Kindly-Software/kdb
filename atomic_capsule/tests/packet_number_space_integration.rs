//! Integration tests for PacketNumberSpaceCapsule (QUIC RFC 9000 §12.3)
//!
//! Requires `network` feature (which includes `std`, `native`, `tokio-compat`, `serde`, `bincode`)

#[cfg(feature = "network")]
use atomic_capsule::network::{PacketNumberSpace, PacketNumberSpaceCapsule};



#[cfg(feature = "network")]
#[test]
fn test_packet_number_space_creation() {
    let capsule = PacketNumberSpaceCapsule::new();

    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Initial), 0);
    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Handshake), 0);
    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Application), 0);
}

#[cfg(feature = "network")]
#[test]
fn test_packet_number_allocation() {
    let capsule = PacketNumberSpaceCapsule::new();

    let pn1 = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to allocate PN1");
    assert_eq!(pn1, 1);

    let pn2 = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to allocate PN2");
    assert_eq!(pn2, 2);

    let pn3 = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to allocate PN3");
    assert_eq!(pn3, 3);
}

#[cfg(feature = "network")]
#[test]
fn test_independent_packet_number_spaces() {
    let capsule = PacketNumberSpaceCapsule::new();

    // Allocate 10 Initial space packets
    for i in 1..=10 {
        let pn = capsule
            .next_packet_number(PacketNumberSpace::Initial)
            .expect("Failed to allocate Initial PN");
        assert_eq!(pn, i);
    }

    // Allocate 5 Handshake space packets (independent counter)
    for i in 1..=5 {
        let pn = capsule
            .next_packet_number(PacketNumberSpace::Handshake)
            .expect("Failed to allocate Handshake PN");
        assert_eq!(pn, i);
    }

    // Allocate 3 Application space packets (independent counter)
    for i in 1..=3 {
        let pn = capsule
            .next_packet_number(PacketNumberSpace::Application)
            .expect("Failed to allocate Application PN");
        assert_eq!(pn, i);
    }

    // Verify final state
    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Initial), 10);
    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Handshake), 5);
    assert_eq!(
        capsule.get_next_packet_number(PacketNumberSpace::Application),
        3
    );
}

#[cfg(feature = "network")]
#[test]
fn test_generation_counters() {
    let capsule = PacketNumberSpaceCapsule::new();

    // Initial generation should be 0
    assert_eq!(capsule.get_generation(PacketNumberSpace::Initial), 0);
    assert_eq!(capsule.get_generation(PacketNumberSpace::Handshake), 0);
    assert_eq!(capsule.get_generation(PacketNumberSpace::Application), 0);

    // Increment generation
    let new_gen = capsule
        .increment_generation(PacketNumberSpace::Initial)
        .expect("Failed to increment generation");
    assert_eq!(new_gen, 1);

    // Verify updated
    assert_eq!(capsule.get_generation(PacketNumberSpace::Initial), 1);
    assert_eq!(capsule.get_generation(PacketNumberSpace::Handshake), 0); // Independent
}

#[cfg(feature = "network")]
#[test]
fn test_largest_acked() {
    let capsule = PacketNumberSpaceCapsule::new();

    // ACK packet 50
    capsule
        .set_largest_acked(PacketNumberSpace::Initial, 50)
        .expect("Failed to set largest acked");

    // Verify
    let acked = capsule.get_largest_acked(PacketNumberSpace::Initial);
    assert_eq!(acked, 50);
}

#[cfg(feature = "network")]
#[test]
fn test_reset_space() {
    let capsule = PacketNumberSpaceCapsule::new();

    // Allocate some packets
    for _ in 0..10 {
        let _ = capsule
            .next_packet_number(PacketNumberSpace::Initial)
            .expect("Failed to allocate PN");
    }

    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Initial), 10);
    assert_eq!(capsule.get_generation(PacketNumberSpace::Initial), 0);

    // Reset the space
    capsule
        .reset_space(PacketNumberSpace::Initial, 1000)
        .expect("Failed to reset space");

    // Verify reset
    assert_eq!(
        capsule.get_next_packet_number(PacketNumberSpace::Initial),
        1000
    );
    assert_eq!(capsule.get_generation(PacketNumberSpace::Initial), 1);
}

#[cfg(feature = "network")]
#[test]
fn test_packet_number_space_enum() {
    assert_eq!(PacketNumberSpace::Initial.as_str(), "Initial");
    assert_eq!(PacketNumberSpace::Handshake.as_str(), "Handshake");
    assert_eq!(PacketNumberSpace::Application.as_str(), "Application");

    assert_eq!(PacketNumberSpace::Initial.to_string(), "Initial");
}

#[cfg(feature = "network")]
#[test]
fn test_high_throughput_allocation() {
    let capsule = PacketNumberSpaceCapsule::new();

    // Allocate 100,000 packet numbers
    for _ in 0..100_000 {
        let _ = capsule
            .next_packet_number(PacketNumberSpace::Application)
            .expect("Failed to allocate PN");
    }

    assert_eq!(
        capsule.get_next_packet_number(PacketNumberSpace::Application),
        100_000
    );
}

#[cfg(feature = "network")]
#[test]
fn test_rfc9000_compliance() {
    // Simulate QUIC handshake (RFC 9000 §12.3)
    let capsule = PacketNumberSpaceCapsule::new();

    // Client sends Initial packet
    let initial_pn = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to send Initial");
    assert_eq!(initial_pn, 1);

    // Client upgrades to Handshake
    let handshake_pn = capsule
        .next_packet_number(PacketNumberSpace::Handshake)
        .expect("Failed to send Handshake");
    assert_eq!(handshake_pn, 1); // Independent space

    // Server ACKs Initial
    capsule
        .set_largest_acked(PacketNumberSpace::Initial, 1)
        .expect("Failed to ACK Initial");

    // Client continues Initial
    let initial_pn2 = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to send Initial 2");
    assert_eq!(initial_pn2, 2);

    // Client upgrades to Application
    let app_pn = capsule
        .next_packet_number(PacketNumberSpace::Application)
        .expect("Failed to send Application");
    assert_eq!(app_pn, 1); // Independent space

    // Verify final state
    assert_eq!(capsule.get_largest_acked(PacketNumberSpace::Initial), 1);
}

#[cfg(feature = "network")]
#[test]
fn test_capsule_size_and_alignment() {
    assert_eq!(std::mem::size_of::<PacketNumberSpaceCapsule>(), 64);
    assert_eq!(std::mem::align_of::<PacketNumberSpaceCapsule>(), 64);
}

#[cfg(feature = "network")]
#[test]
fn test_default_trait() {
    let capsule = PacketNumberSpaceCapsule::default();
    assert_eq!(capsule.get_next_packet_number(PacketNumberSpace::Initial), 0);
}

#[cfg(feature = "network")]
#[test]
fn test_is_valid_pn() {
    let capsule = PacketNumberSpaceCapsule::new();

    // Initially, PN 0 and above are valid
    assert!(capsule.is_valid_pn(PacketNumberSpace::Initial, 0));
    assert!(capsule.is_valid_pn(PacketNumberSpace::Initial, 100));

    // Allocate one packet
    let pn = capsule
        .next_packet_number(PacketNumberSpace::Initial)
        .expect("Failed to allocate PN");
    assert_eq!(pn, 1);

    // Now PN 1 and above are valid, 0 is not
    assert!(!capsule.is_valid_pn(PacketNumberSpace::Initial, 0));
    assert!(capsule.is_valid_pn(PacketNumberSpace::Initial, 1));
    assert!(capsule.is_valid_pn(PacketNumberSpace::Initial, 1000));
}
