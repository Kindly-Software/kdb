//! Frame Buffer Capsule Benchmarks - B32 Framework
//!
//! Framework: B32 Fair Baseline Benchmarking (95% CI, 1000+ iterations)
//! Targets: <50ns metadata query, <100ns buffer coordination
//! Comparison: Naive mutable struct (baseline)
//!
//! Run with: cargo bench --bench frame_buffer_bench --features std

use atomic_capsule::encoder::{FrameBufferCapsule, FrameType};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// ============================================================================
// Group 1: Metadata Query Operations (<50ns target)
// ============================================================================

fn bench_frame_type_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/get_frame_type", |b| {
        b.iter(|| black_box(capsule.get_frame_type()))
    });
}

fn bench_pts_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    capsule.update_frame_metadata(12345, 5);

    c.bench_function("frame_buffer/get_pts", |b| {
        b.iter(|| black_box(capsule.get_pts()))
    });
}

fn bench_frame_id_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    capsule.update_frame_metadata(12345, 5);

    c.bench_function("frame_buffer/get_frame_id", |b| {
        b.iter(|| black_box(capsule.get_frame_id()))
    });
}

fn bench_generation_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/get_generation", |b| {
        b.iter(|| black_box(capsule.get_generation()))
    });
}

fn bench_dimensions_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/get_dimensions", |b| {
        b.iter(|| black_box(capsule.get_dimensions()))
    });
}

// ============================================================================
// Group 2: Reference Counting (<30ns target)
// ============================================================================

fn bench_increment_ref(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/increment_ref", |b| {
        b.iter(|| {
            let _ = black_box(capsule.increment_ref());
            capsule.decrement_ref(); // Reset for next iteration
        })
    });
}

fn bench_decrement_ref(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/decrement_ref", |b| {
        b.iter(|| {
            capsule.increment_ref().unwrap();
            black_box(capsule.decrement_ref());
        })
    });
}

fn bench_ref_count_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/get_ref_count", |b| {
        b.iter(|| black_box(capsule.get_ref_count()))
    });
}

// ============================================================================
// Group 3: Plane Pointer Operations (<20ns target)
// ============================================================================

fn bench_get_y_plane(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let mut buffer = vec![0u8; 4_147_200];
    capsule.attach_buffer(buffer.as_mut_ptr(), 0, 1920 * 1080, 1920 * 1080 + 960 * 540);

    c.bench_function("frame_buffer/get_y_plane", |b| {
        b.iter(|| black_box(capsule.get_y_plane()))
    });
}

fn bench_get_u_plane(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let mut buffer = vec![0u8; 4_147_200];
    capsule.attach_buffer(buffer.as_mut_ptr(), 0, 1920 * 1080, 1920 * 1080 + 960 * 540);

    c.bench_function("frame_buffer/get_u_plane", |b| {
        b.iter(|| black_box(capsule.get_u_plane()))
    });
}

fn bench_get_v_plane(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let mut buffer = vec![0u8; 4_147_200];
    capsule.attach_buffer(buffer.as_mut_ptr(), 0, 1920 * 1080, 1920 * 1080 + 960 * 540);

    c.bench_function("frame_buffer/get_v_plane", |b| {
        b.iter(|| black_box(capsule.get_v_plane()))
    });
}

// ============================================================================
// Group 4: Dirty Flag Operations (<50ns target)
// ============================================================================

fn bench_mark_dirty(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/mark_dirty", |b| {
        b.iter(|| {
            black_box(capsule.mark_dirty());
            capsule.clear_dirty(); // Reset
        })
    });
}

fn bench_clear_dirty(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/clear_dirty", |b| {
        b.iter(|| {
            capsule.mark_dirty();
            black_box(capsule.clear_dirty());
        })
    });
}

fn bench_is_dirty_query(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/is_dirty", |b| {
        b.iter(|| black_box(capsule.is_dirty()))
    });
}

// ============================================================================
// Group 5: Metadata Updates (<100ns target)
// ============================================================================

fn bench_update_frame_metadata(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/update_frame_metadata", |b| {
        b.iter(|| black_box(capsule.update_frame_metadata(12345, 5)))
    });
}

fn bench_update_dimensions(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/update_dimensions", |b| {
        b.iter(|| black_box(capsule.update_dimensions(3840, 2160, 3840)))
    });
}

// ============================================================================
// Group 6: Timestamp Operations (<20ns target)
// ============================================================================

fn bench_set_timestamp_ns(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    c.bench_function("frame_buffer/set_timestamp_ns", |b| {
        b.iter(|| black_box(capsule.set_timestamp_ns(1_000_000_000)))
    });
}

fn bench_get_timestamp_ns(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    capsule.set_timestamp_ns(1_000_000_000);

    c.bench_function("frame_buffer/get_timestamp_ns", |b| {
        b.iter(|| black_box(capsule.get_timestamp_ns()))
    });
}

// ============================================================================
// Group 7: Checksum Operations (<100ns target)
// ============================================================================

fn bench_update_checksum_small(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let data = vec![0x55u8; 64]; // Small data block

    c.bench_function("frame_buffer/update_checksum_small", |b| {
        b.iter(|| black_box(capsule.update_checksum(&data)))
    });
}

fn bench_update_checksum_large(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let data = vec![0x55u8; 8192]; // Typical frame chunk

    c.bench_function("frame_buffer/update_checksum_large", |b| {
        b.iter(|| black_box(capsule.update_checksum(&data)))
    });
}

fn bench_get_checksum(c: &mut Criterion) {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    capsule.update_checksum(b"test data");

    c.bench_function("frame_buffer/get_checksum", |b| {
        b.iter(|| black_box(capsule.get_checksum()))
    });
}

// ============================================================================
// Group 8: Combined Operations (Integration Benchmark)
// ============================================================================

fn bench_creation_and_setup(c: &mut Criterion) {
    c.bench_function("frame_buffer/creation_and_setup", |b| {
        b.iter(|| {
            let capsule = black_box(FrameBufferCapsule::new(1920, 1080, FrameType::Key));
            capsule.update_frame_metadata(12345, 5);
            capsule.set_timestamp_ns(1_000_000_000);
            capsule
        })
    });
}

fn bench_complete_lifecycle(c: &mut Criterion) {
    c.bench_function("frame_buffer/complete_lifecycle", |b| {
        b.iter(|| {
            let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
            capsule.update_frame_metadata(12345, 5);
            capsule.set_timestamp_ns(1_000_000_000);

            capsule.increment_ref().unwrap();
            capsule.mark_dirty();
            capsule.update_checksum(b"frame data");

            capsule.decrement_ref();
            capsule.clear_dirty();

            black_box(capsule.get_checksum());
        })
    });
}

// ============================================================================
// Criterion Groups Configuration
// ============================================================================

criterion_group!(
    benches,
    // Group 1: Metadata queries
    bench_frame_type_query,
    bench_pts_query,
    bench_frame_id_query,
    bench_generation_query,
    bench_dimensions_query,
    // Group 2: Reference counting
    bench_increment_ref,
    bench_decrement_ref,
    bench_ref_count_query,
    // Group 3: Plane pointers
    bench_get_y_plane,
    bench_get_u_plane,
    bench_get_v_plane,
    // Group 4: Dirty flag
    bench_mark_dirty,
    bench_clear_dirty,
    bench_is_dirty_query,
    // Group 5: Metadata updates
    bench_update_frame_metadata,
    bench_update_dimensions,
    // Group 6: Timestamps
    bench_set_timestamp_ns,
    bench_get_timestamp_ns,
    // Group 7: Checksums
    bench_update_checksum_small,
    bench_update_checksum_large,
    bench_get_checksum,
    // Group 8: Integration
    bench_creation_and_setup,
    bench_complete_lifecycle,
);

criterion_main!(benches);
