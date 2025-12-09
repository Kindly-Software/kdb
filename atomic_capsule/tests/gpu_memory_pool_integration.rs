// Integration tests for GpuMemoryPoolCapsule
// Validates complete functionality of bitmap-based GPU memory pool

use atomic_capsule::gpu::kernels::{GpuMemoryPoolCapsule, GpuAllocation, GpuMemoryPoolSnapshot};

#[test]
fn test_memory_pool_layout() {
    // Verify Chaos compliance: 512B alignment
    assert_eq!(core::mem::size_of::<GpuMemoryPoolCapsule>(), 512);
    assert_eq!(core::mem::align_of::<GpuMemoryPoolCapsule>(), 512);
}

#[test]
fn test_memory_pool_creation() {
    let pool = GpuMemoryPoolCapsule::new(0, 102400, 1024).unwrap();

    assert_eq!(pool.block_size(), 1024);
    assert_eq!(pool.device_id(), 0);
    assert_eq!(pool.available(), 102400);
    assert_eq!(pool.used(), 0);
}

#[test]
fn test_memory_pool_allocation_deallocation() {
    let pool = GpuMemoryPoolCapsule::new(0, 10240, 1024).unwrap();

    // Allocate block
    let alloc = pool.allocate(1024).unwrap();
    assert_eq!(alloc.size, 1024);
    assert_eq!(alloc.block_idx, 0);
    assert_eq!(pool.used(), 1024);
    assert_eq!(pool.available(), 10240 - 1024);

    // Deallocate block
    pool.deallocate(alloc).unwrap();
    assert_eq!(pool.used(), 0);
    assert_eq!(pool.available(), 10240);
}

#[test]
fn test_memory_pool_multiple_allocations() {
    let pool = GpuMemoryPoolCapsule::new(0, 5120, 1024).unwrap();

    let alloc1 = pool.allocate(1024).unwrap();
    let alloc2 = pool.allocate(1024).unwrap();
    let alloc3 = pool.allocate(1024).unwrap();

    assert_eq!(alloc1.block_idx, 0);
    assert_eq!(alloc2.block_idx, 1);
    assert_eq!(alloc3.block_idx, 2);
    assert_eq!(pool.used(), 3072);
}

#[test]
fn test_memory_pool_exhaustion() {
    let pool = GpuMemoryPoolCapsule::new(0, 2560, 512).unwrap();

    // Allocate all 5 blocks
    for _ in 0..5 {
        pool.allocate(512).unwrap();
    }

    // 6th allocation should fail
    assert!(pool.allocate(512).is_err());
    assert_eq!(pool.available(), 0);
}

#[test]
fn test_memory_pool_snapshot() {
    let pool = GpuMemoryPoolCapsule::new(0, 10240, 1024).unwrap();

    // Initial snapshot
    let snap1 = pool.snapshot();
    assert_eq!(snap1.total_size, 10240);
    assert_eq!(snap1.used_size, 0);
    assert_eq!(snap1.alloc_count, 0);
    assert_eq!(snap1.free_count, 0);
    assert_eq!(snap1.available_blocks, 512);

    // Allocate 2 blocks
    pool.allocate(1024).unwrap();
    pool.allocate(1024).unwrap();

    let snap2 = pool.snapshot();
    assert_eq!(snap2.used_size, 2048);
    assert_eq!(snap2.alloc_count, 2);
    assert_eq!(snap2.available_blocks, 510);
}

#[test]
fn test_memory_pool_generation_counter() {
    let pool = GpuMemoryPoolCapsule::new(0, 5120, 1024).unwrap();

    let alloc1 = pool.allocate(1024).unwrap();
    assert_eq!(alloc1.generation, 0);

    let alloc2 = pool.allocate(1024).unwrap();
    assert_eq!(alloc2.generation, 1);

    let alloc3 = pool.allocate(1024).unwrap();
    assert_eq!(alloc3.generation, 2);
}

#[test]
fn test_memory_pool_double_free_detection() {
    let pool = GpuMemoryPoolCapsule::new(0, 5120, 1024).unwrap();

    let alloc = pool.allocate(1024).unwrap();
    pool.deallocate(alloc).unwrap();

    // Second deallocate should fail
    assert!(pool.deallocate(alloc).is_err());
}

#[test]
fn test_memory_pool_reuse_after_free() {
    let pool = GpuMemoryPoolCapsule::new(0, 5120, 1024).unwrap();

    // Allocate, deallocate, re-allocate
    let alloc1 = pool.allocate(1024).unwrap();
    let block_idx1 = alloc1.block_idx;

    pool.deallocate(alloc1).unwrap();

    let alloc2 = pool.allocate(1024).unwrap();

    // Should reuse the same block (first-fit)
    assert_eq!(alloc2.block_idx, block_idx1);
}
