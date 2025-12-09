//! Example demonstrating the DualAtomicBuilder pattern
//!
//! This example shows how to use the builder pattern to create
//! runtime-configurable field layouts for DualAtomicU64.

use atomic_capsule::patterns::{DualAtomicBuilder, BuilderError};

fn main() -> Result<(), BuilderError> {
    println!("=== DualAtomicBuilder Demo ===\n");

    // Example 1: Circuit Breaker Layout
    println!("1. Circuit Breaker Layout");
    println!("   Fields: state(3), failures(8), successes(8), timestamp(32)");

    let breaker_layout = DualAtomicBuilder::new()
        .primary_field("state", 3)           // 0-7 (Closed, Open, HalfOpen)
        .primary_field("failures", 8)        // 0-255 consecutive failures
        .primary_field("successes", 8)       // 0-255 consecutive successes
        .primary_field("timestamp", 32)      // Last state change timestamp
        .secondary_as_generation()           // Full 64-bit generation counter
        .build()?;

    println!("   ✓ Layout created");
    println!("   - Primary bits used: {}/64", breaker_layout.primary_bits_used());
    println!("   - Secondary is generation: {}", breaker_layout.is_generation_counter());

    // Demonstrate field access
    let state = breaker_layout.primary_field("state").unwrap();
    let failures = breaker_layout.primary_field("failures").unwrap();

    let mut packed = 0u64;
    packed = state.set(packed, 2);      // State = HalfOpen (2)
    packed = failures.set(packed, 5);   // 5 failures

    println!("   - State value: {}", state.get(packed));
    println!("   - Failures: {}", failures.get(packed));

    println!();

    // Example 2: Rate Limiter Layout
    println!("2. Rate Limiter Layout");
    println!("   Fields: tokens(16), last_refill(32), capacity(12)");

    let limiter_layout = DualAtomicBuilder::new()
        .primary_field("tokens", 16)         // Current token count (0-65535)
        .primary_field("last_refill", 32)    // Last refill timestamp
        .primary_field("capacity", 12)       // Max capacity (0-4095)
        .secondary_as_generation()
        .build()?;

    println!("   ✓ Layout created");
    println!("   - Primary bits used: {}/64", limiter_layout.primary_bits_used());

    let tokens = limiter_layout.primary_field("tokens").unwrap();
    let capacity = limiter_layout.primary_field("capacity").unwrap();

    let mut state = 0u64;
    state = tokens.set(state, 100);
    state = capacity.set(state, 1000);

    println!("   - Current tokens: {}", tokens.get(state));
    println!("   - Max capacity: {}", capacity.get(state));

    println!();

    // Example 3: Position Tracker Layout
    println!("3. Position Tracker Layout");
    println!("   Fields: position(32), timestamp(32)");

    let tracker_layout = DualAtomicBuilder::new()
        .primary_field("position", 32)       // File position or offset
        .primary_field("timestamp", 32)      // Last access timestamp
        .secondary_as_generation()
        .build()?;

    println!("   ✓ Layout created");
    println!("   - Primary bits used: {}/64", tracker_layout.primary_bits_used());

    println!();

    // Example 4: Error Handling
    println!("4. Error Handling Examples");

    // Overflow error
    println!("   Testing overflow detection...");
    match DualAtomicBuilder::new()
        .primary_field("large", 60)
        .primary_field("overflow", 10)  // Would exceed 64 bits
        .build()
    {
        Err(BuilderError::FieldOverflow { field_name, offset, width }) => {
            println!("   ✓ Caught overflow: '{}' at offset {} with width {} would exceed 64 bits",
                     field_name, offset, width);
        }
        _ => println!("   ✗ Should have caught overflow!"),
    }

    // Zero width error
    println!("   Testing zero width detection...");
    match DualAtomicBuilder::new()
        .primary_field("empty", 0)  // Invalid: zero width
        .build()
    {
        Err(BuilderError::ZeroWidth { field_name }) => {
            println!("   ✓ Caught zero width: '{}'", field_name);
        }
        _ => println!("   ✗ Should have caught zero width!"),
    }

    println!();

    // Example 5: Complex Multi-Field Layout
    println!("5. Complex Multi-Field Layout (8 fields)");
    println!("   Simulating a complex protocol header");

    let protocol_layout = DualAtomicBuilder::new()
        .primary_field("version", 4)
        .primary_field("flags", 8)
        .primary_field("sequence", 16)
        .primary_field("checksum", 16)
        .primary_field("priority", 4)
        .primary_field("reserved", 8)
        .secondary_as_generation()
        .build()?;

    println!("   ✓ Layout with {} fields created", protocol_layout.primary_field_count());
    println!("   - Total bits used: {}/64", protocol_layout.primary_bits_used());

    // Set all fields
    let mut header = 0u64;
    header = protocol_layout.primary_field("version").unwrap().set(header, 1);
    header = protocol_layout.primary_field("flags").unwrap().set(header, 0b10101010);
    header = protocol_layout.primary_field("sequence").unwrap().set(header, 12345);
    header = protocol_layout.primary_field("checksum").unwrap().set(header, 0xABCD);
    header = protocol_layout.primary_field("priority").unwrap().set(header, 7);

    println!("   - Version: {}", protocol_layout.primary_field("version").unwrap().get(header));
    println!("   - Flags: 0b{:08b}", protocol_layout.primary_field("flags").unwrap().get(header));
    println!("   - Sequence: {}", protocol_layout.primary_field("sequence").unwrap().get(header));
    println!("   - Checksum: 0x{:04X}", protocol_layout.primary_field("checksum").unwrap().get(header));
    println!("   - Priority: {}", protocol_layout.primary_field("priority").unwrap().get(header));

    println!();
    println!("=== Demo Complete ===");

    Ok(())
}
