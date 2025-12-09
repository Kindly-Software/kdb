use core::sync::atomic::AtomicU64;

#[repr(C, align(512))]
struct TestSpirVCompilerCapsule {
    stats_a: AtomicU64,
    stats_b: AtomicU64,
    total_errors: AtomicU64,
    cache_entries: AtomicU64,
    target_env: AtomicU64,
    config: AtomicU64,
    cache_capacity: u32,
    _reserved: u32,
    _padding: [u8; 456],
}

fn main() {
    println!("Size: {}", core::mem::size_of::<TestSpirVCompilerCapsule>());
    println!("Align: {}", core::mem::align_of::<TestSpirVCompilerCapsule>());

    // Field sizes
    println!("AtomicU64: {}", core::mem::size_of::<AtomicU64>());
    println!("6 x AtomicU64: {}", 6 * core::mem::size_of::<AtomicU64>());
    println!("2 x u32: {}", 2 * core::mem::size_of::<u32>());
    println!("Total before padding: {}", 6 * 8 + 2 * 4);
    println!("Padding needed: {}", 512 - (6 * 8 + 2 * 4));
}
