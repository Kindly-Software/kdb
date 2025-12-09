use clapi_core::capsules::PaymentCapsule256;

fn main() {
    let payment = PaymentCapsule256::new(1, 2, 1_000_00);
    
    println!("Initial state:");
    println!("  hash: {}", payment.hash());
    println!("  prev_hash: {}", payment.prev_hash());
    println!("  verify: {}", payment.verify_chain());
    
    payment.update_hash_chain();
    println!("\nAfter first update_hash_chain():");
    println!("  hash: {}", payment.hash());
    println!("  prev_hash: {}", payment.prev_hash());
    println!("  verify: {}", payment.verify_chain());
    
    payment.start_processing().unwrap();
    println!("\nAfter start_processing() (before hash update):");
    println!("  hash: {}", payment.hash());
    println!("  prev_hash: {}", payment.prev_hash());
    println!("  verify: {}", payment.verify_chain());
    
    payment.update_hash_chain();
    println!("\nAfter second update_hash_chain():");
    println!("  hash: {}", payment.hash());
    println!("  prev_hash: {}", payment.prev_hash());
    println!("  verify: {}", payment.verify_chain());
}
