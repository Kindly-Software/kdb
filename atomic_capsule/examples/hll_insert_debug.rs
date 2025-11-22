use atomic_capsule::hash::scalar_fast_hash;
use std::sync::atomic::{AtomicU8, Ordering};

fn main() {
    // Simulate HLL insert logic
    const M: usize = 16384;
    const INDEX_BITS: u32 = 14;

    let buckets: Vec<AtomicU8> = (0..M).map(|_| AtomicU8::new(0)).collect();

    println!("Testing insert logic:");

    // Insert a few elements
    for element in 0..20u64 {
        let hash = scalar_fast_hash(&[element]);
        let bucket_index = (hash & 0x3FFF) as usize;
        let w = hash >> INDEX_BITS;

        let rho = if w == 0 {
            51
        } else {
            (w.leading_zeros() - (64 - 50) + 1) as u8
        };

        let bucket = &buckets[bucket_index];
        let old = bucket.load(Ordering::Relaxed);

        println!(
            "Element {}: bucket_index={}, rho={}, old={}",
            element, bucket_index, rho, old
        );

        if rho <= old {
            println!("  -> Skipped (rho <= old)");
        } else {
            match bucket.compare_exchange_weak(old, rho, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => println!("  -> Updated (old={} -> new={})", old, rho),
                Err(actual) => println!("  -> CAS failed (expected={}, actual={})", old, actual),
            }
        }
    }

    // Count non-zero buckets
    let non_zero = buckets
        .iter()
        .filter(|b| b.load(Ordering::Relaxed) != 0)
        .count();
    println!("\nNon-zero buckets: {}", non_zero);
}
