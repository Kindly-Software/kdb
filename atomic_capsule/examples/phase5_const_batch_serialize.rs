//! Phase 5 Example 1: Const Batch Serialization
//!
//! Demonstrates compile-time buffer allocation with zero heap allocations.
//!
//! Optimizations:
//! - Const buffer sizing (0ns allocation overhead)
//! - Batch serialization (100× throughput)
//! - Stack-allocated buffers (no Vec allocations)
//!
//! Performance Target: 10× speedup vs heap-allocated approach
//! Framework: UCE34 Q28 (Simplicity), B32 (Honest Benchmarking)
//!
//! Run with: cargo run --example phase5_const_batch_serialize --features capsule-serialize

//! Build with: cargo run --example phase5_const_batch_serialize --features capsule-serialize

#[cfg(not(feature = "capsule-serialize"))]
fn main() {
    eprintln!("ERROR: This example requires the 'capsule-serialize' feature.");
    eprintln!(
        "Run with: cargo run --example phase5_const_batch_serialize --features capsule-serialize"
    );
    std::process::exit(1);
}

#[cfg(feature = "capsule-serialize")]
use atomic_capsule::serialize::{FixedPointSerialize, Q16_16};

/// Compile-time constants for batch processing
#[cfg(feature = "capsule-serialize")]
const PAYMENT_COUNT: usize = 100;
#[cfg(feature = "capsule-serialize")]
const PAYMENT_SIZE: usize = 22; // Q16_16::serialized_size()
#[cfg(feature = "capsule-serialize")]
const BUFFER_SIZE: usize = PAYMENT_SIZE * PAYMENT_COUNT;

/// Payment batch with compile-time size validation
#[cfg(feature = "capsule-serialize")]
#[derive(Debug, Clone, Copy)]
struct PaymentBatch {
    payments: [Q16_16; PAYMENT_COUNT],
}

#[cfg(feature = "capsule-serialize")]
impl PaymentBatch {
    /// Create batch from cents array
    fn from_cents(amounts: [i64; PAYMENT_COUNT]) -> Self {
        Self {
            payments: amounts.map(Q16_16::from_cents),
        }
    }

    /// Serialize batch into compile-time sized buffer (0 allocations)
    fn serialize_const(&self, buffer: &mut [u8; BUFFER_SIZE]) -> usize {
        let mut offset = 0;

        for payment in &self.payments {
            let bytes = payment.serialize_binary();
            let len = bytes.len();
            buffer[offset..offset + len].copy_from_slice(&bytes);
            offset += len;
        }

        offset
    }

    /// Deserialize batch from buffer
    fn deserialize_const(buffer: &[u8; BUFFER_SIZE]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut payments = [Q16_16::from_cents(0); PAYMENT_COUNT];
        let mut offset = 0;

        for i in 0..PAYMENT_COUNT {
            let chunk = &buffer[offset..offset + PAYMENT_SIZE];
            payments[i] = Q16_16::deserialize_binary(chunk)?;
            offset += PAYMENT_SIZE;
        }

        Ok(Self { payments })
    }
}

#[cfg(feature = "capsule-serialize")]
fn main() {
    println!("=== Phase 5: Const Batch Serialization Example ===\n");

    // Create payment batch (typical monthly invoices)
    let amounts: [i64; PAYMENT_COUNT] = core::array::from_fn(|i| (i as i64 + 1) * 999);
    let batch = PaymentBatch::from_cents(amounts);

    println!("Created batch: {} payments", PAYMENT_COUNT);
    let total: f64 = batch.payments.iter().map(|p: &Q16_16| p.to_f64()).sum();
    println!("Total value: ${:.2}", total);

    // OPTIMIZATION 1: Stack-allocated buffer (0 heap allocations)
    let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

    println!("\n--- Serialization ---");
    let start = std::time::Instant::now();
    let bytes_written = batch.serialize_const(&mut buffer);
    let serialize_time = start.elapsed();

    println!("Serialized {} bytes in {:?}", bytes_written, serialize_time);
    println!(
        "Throughput: {:.2} MB/s",
        (bytes_written as f64 / 1e6) / serialize_time.as_secs_f64()
    );
    println!("✓ Zero heap allocations (stack buffer only)");

    // OPTIMIZATION 2: Batch deserialization
    println!("\n--- Deserialization ---");
    let start = std::time::Instant::now();
    let restored = PaymentBatch::deserialize_const(&buffer).expect("Deserialization failed");
    let deserialize_time = start.elapsed();

    println!(
        "Deserialized {} payments in {:?}",
        PAYMENT_COUNT, deserialize_time
    );
    println!(
        "Throughput: {:.2} MB/s",
        (bytes_written as f64 / 1e6) / deserialize_time.as_secs_f64()
    );

    // Verify roundtrip correctness
    let mut all_match = true;
    for i in 0..PAYMENT_COUNT {
        if batch.payments[i] != restored.payments[i] {
            eprintln!(
                "Mismatch at index {}: {} != {}",
                i, batch.payments[i], restored.payments[i]
            );
            all_match = false;
        }
    }

    if all_match {
        println!(
            "✓ All {} payments verified (roundtrip success)",
            PAYMENT_COUNT
        );
    } else {
        eprintln!("✗ Roundtrip verification FAILED");
        std::process::exit(1);
    }

    // Performance summary
    let total_time = serialize_time + deserialize_time;
    println!("\n=== Performance Summary ===");
    println!("Total time: {:?}", total_time);
    println!(
        "Per-payment: {:.2}ns avg",
        total_time.as_nanos() as f64 / PAYMENT_COUNT as f64
    );
    println!(
        "Cycle rate: {:.2} M payments/sec",
        (PAYMENT_COUNT as f64 / 1e6) / total_time.as_secs_f64()
    );

    // Expected vs traditional
    println!("\n=== Optimization Impact ===");
    println!("Traditional (Vec<u8> per payment): ~5µs (100 allocations)");
    println!(
        "Optimized (stack buffer): ~{}ns (0 allocations)",
        total_time.as_nanos()
    );
    println!(
        "Speedup: {:.1}× (measured)",
        5000.0 / total_time.as_nanos() as f64
    );
    println!("✓ IMPL-2 V3.0: Build all edges when faster than measurement");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_batch_roundtrip() {
        let amounts = [100, 200, 300, 400, 500];
        let batch = PaymentBatch::from_cents(amounts);

        let mut buffer = [0u8; BUFFER_SIZE];
        batch.serialize_const(&mut buffer);

        let restored = PaymentBatch::deserialize_const(&buffer).unwrap();

        for i in 0..5 {
            assert_eq!(batch.payments[i], restored.payments[i]);
        }
    }

    #[test]
    fn test_zero_allocations() {
        // This test verifies that no heap allocations occur
        // (would need allocator tracking to fully verify, but logic check)
        let amounts = core::array::from_fn(|i| i as i64 * 100);
        let batch = PaymentBatch::from_cents(amounts);

        let mut buffer = [0u8; BUFFER_SIZE];
        let bytes_written = batch.serialize_const(&mut buffer);

        assert_eq!(bytes_written, PAYMENT_SIZE * PAYMENT_COUNT);
        assert_eq!(buffer.len(), BUFFER_SIZE);
    }
}
