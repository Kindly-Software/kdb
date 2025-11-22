use atomic_capsule::hash::scalar_fast_hash;
use atomic_capsule::probabilistic::HyperLogLogCapsule;

fn main() {
    let hll = HyperLogLogCapsule::new();

    println!("=== Testing HLL ===");
    println!("Initial cardinality: {}", hll.cardinality());

    // Insert single element and trace
    let element = 12345u64;
    let hash = scalar_fast_hash(&[element]);
    println!("\nInserting element {}", element);
    println!("Hash: 0x{:016x}", hash);

    let bucket_index = (hash & 0x3FFF) as usize;
    let w = hash >> 14;
    let rho = if w == 0 {
        51
    } else {
        (w.leading_zeros() - (64 - 50) + 1) as u8
    };

    println!("Bucket index: {}", bucket_index);
    println!("w (50 bits): 0x{:013x}", w);
    println!("rho (leading zeros + 1): {}", rho);

    hll.insert(element);

    let card = hll.cardinality();
    println!("\nAfter insert: cardinality = {}", card);

    // Insert a few more
    println!("\n=== Inserting 10 elements ===");
    for i in 0..10 {
        hll.insert(i);
    }
    println!("Cardinality after 10 inserts: {}", hll.cardinality());

    println!("\n=== Inserting 100 elements ===");
    for i in 10..110 {
        hll.insert(i);
    }
    println!("Cardinality after 110 inserts: {}", hll.cardinality());

    println!("\n=== Inserting 1000 elements ===");
    for i in 110..1110 {
        hll.insert(i);
    }
    let card1000 = hll.cardinality();
    println!("Cardinality after 1110 inserts: {}", card1000);
    println!(
        "Expected: ~1110, Error: {:.2}%",
        ((card1000 as f64 - 1110.0).abs() / 1110.0) * 100.0
    );
}
