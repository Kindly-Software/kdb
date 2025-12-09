// Integration tests for HttpBodyBufferCapsule
// Tests the T4 Batch tier HTTP body buffering capsule

use atomic_capsule::http::HttpBodyBufferCapsule;

#[test]
fn test_body_buffer_new_default() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    assert_eq!(capsule.memory_capacity(), 1024 * 1024); // 1MB
    assert_eq!(capsule.memory_used(), 0);
    assert_eq!(capsule.total_bytes_buffered(), 0);
}

#[test]
fn test_body_buffer_new_custom_size() {
    let capsule = HttpBodyBufferCapsule::new(512 * 1024).unwrap();
    assert_eq!(capsule.memory_capacity(), 512 * 1024);
    assert_eq!(capsule.memory_used(), 0);
}

#[test]
fn test_body_buffer_append_small() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let data = b"Hello, World!";
    let written = capsule.append(data).unwrap();
    assert_eq!(written, 13);
    assert_eq!(capsule.memory_used(), 13);
    assert_eq!(capsule.total_bytes_buffered(), 13);
}

#[test]
fn test_body_buffer_append_multiple() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(b"Part 1").unwrap();
    capsule.append(b"Part 2").unwrap();
    capsule.append(b"Part 3").unwrap();
    assert_eq!(capsule.memory_used(), 18);
    assert_eq!(capsule.total_bytes_buffered(), 18);
}

#[test]
fn test_body_buffer_read_in_memory() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(b"Hello, World!").unwrap();
    let data = capsule.read(0, 5).unwrap();
    assert_eq!(&data[..], b"Hello");
}

#[test]
fn test_body_buffer_read_offset() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(b"Hello, World!").unwrap();
    let data = capsule.read(7, 5).unwrap();
    assert_eq!(&data[..], b"World");
}

#[test]
fn test_body_buffer_read_full() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(b"Hello, World!").unwrap();
    let data = capsule.read(0, 13).unwrap();
    assert_eq!(&data[..], b"Hello, World!");
}

#[test]
fn test_body_buffer_metrics_accuracy() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    for i in 0..10 {
        capsule.append(&vec![0u8; 1000]).unwrap();
        assert_eq!(capsule.total_bytes_buffered() as usize, (i + 1) * 1000);
    }
}

#[test]
fn test_body_buffer_reset() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(b"Some data").unwrap();
    assert!(capsule.memory_used() > 0);
    capsule.reset().unwrap();
    assert_eq!(capsule.memory_used(), 0);
}

#[test]
fn test_body_buffer_generation_counter() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let gen1 = capsule.generation();
    capsule.append(b"data").unwrap();
    let gen2 = capsule.generation();
    capsule.append(b"more").unwrap();
    let gen3 = capsule.generation();
    assert!(gen2 > gen1);
    assert!(gen3 > gen2);
}

#[test]
fn test_body_buffer_cache_alignment() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 256, 0, "Capsule not 256-byte aligned");
}

#[test]
fn test_body_buffer_large_append() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let large_data = vec![0x42u8; 1024 * 500]; // 500KB
    let written = capsule.append(&large_data).unwrap();
    assert_eq!(written, 1024 * 500);
    assert_eq!(capsule.memory_used() as usize, 1024 * 500);
}

#[test]
fn test_body_buffer_read_empty() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let data = capsule.read(0, 0).unwrap();
    assert_eq!(data.len(), 0);
}

#[test]
fn test_body_buffer_toctou_generation() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    let gen_before = capsule.generation();
    capsule.append(b"data").unwrap();
    let gen_after = capsule.generation();
    assert!(gen_after > gen_before, "Generation counter not incremented");
}

#[test]
fn test_body_buffer_metrics_consistency() {
    let capsule = HttpBodyBufferCapsule::new_default().unwrap();
    capsule.append(&vec![0u8; 100]).unwrap();
    capsule.append(&vec![0u8; 200]).unwrap();
    capsule.append(&vec![0u8; 300]).unwrap();

    let total = capsule.total_bytes_buffered();
    assert_eq!(total, 600);
    assert_eq!(capsule.memory_used() as u64, 600);
}

#[test]
fn test_body_buffer_capsule_size() {
    use std::mem;
    assert_eq!(mem::size_of::<HttpBodyBufferCapsule>(), 256);
    assert_eq!(mem::align_of::<HttpBodyBufferCapsule>(), 256);
}
