//! Diagnostic test to verify sequence number calculation

use atomic_capsule::collections::queue::{QueueCapsule, MPMC};

#[test]
fn test_sequence_calculation() {
    let queue = QueueCapsule::<u64, MPMC>::new(4).expect("Failed to create queue");

    // The issue: sequences are initialized as [0, 2, 4, 6]
    // But after 4 items are pushed and popped, they become [2, 4, 6, 8]
    // Then on the next cycle, the formula expects [8, 10, 12, 14]
    // This causes a MISMATCH: 2 != 8, 4 != 10, etc.

    println!("Expected sequence values for rotation 0 (rotation calculation):");
    for slot in 0..4 {
        let expected = ((0 as u64) * 4 + slot as u64) * 2;
        println!("  Slot {}: (0*4 + {})*2 = {}", slot, slot, expected);
    }

    println!("\nExpected sequence values for rotation 1 (rotation calculation):");
    for slot in 0..4 {
        let expected = ((1 as u64) * 4 + slot as u64) * 2;
        println!("  Slot {}: (1*4 + {})*2 = {}", slot, slot, expected);
    }

    println!("\nBut initialization sets sequences[i] = i*2:");
    for slot in 0..4 {
        let init = (slot as u64) * 2;
        println!("  sequences[{}] = {}", slot, init);
    }

    println!("\nAfter pop phase of rotation 0, sequences[i] += 2 for each pop:");
    println!("  sequences[0]: 0 → 1 (push) → 2 (pop)");
    println!("  sequences[1]: 2 → 3 (push) → 4 (pop)");
    println!("  sequences[2]: 4 → 5 (push) → 6 (pop)");
    println!("  sequences[3]: 6 → 7 (push) → 8 (pop)");

    println!("\nMISMATCH on rotation 1:");
    println!("  Push to slot 0: expects sequence = (1*4+0)*2 = 8");
    println!("  But sequences[0] = 2 (left from pop in rotation 0)");
    println!("  CAS(expected=8, actual=2) FAILS → DEADLOCK!");

    println!("\n✗ TEST DEMONSTRATES THE BUG");
    println!("The formula and initialization are out of sync!");
}
