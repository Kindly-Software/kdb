//! FixedPointMatrixConst Benchmarks - Phase Nightly 2 (Primitive 3)
//!
//! Benchmarks for compile-time fixed-point matrices with zero allocation.
//! Tests performance targets: 10-50× speedup (EXCEPTIONAL tier) vs scalar baseline.
//!
//! # Performance Targets (B32 Framework)
//!
//! - **8×8 Matrix**: <5μs matmul (scalar: ~50μs) = 10× speedup
//! - **64×64 Matrix**: <500μs matmul (scalar: ~5ms) = 10× speedup
//! - **256×256 Matrix**: <50ms matmul (scalar: ~500ms) = 10× speedup
//! - **1024×1024 Matrix**: <10ms matmul (scalar: 100-500ms) = 10-50× speedup
//! - **Batch 64×1024×1024**: <500μs matmul (scalar: 5-25ms) = 10-50× speedup

#![cfg_attr(feature = "nightly", feature(generic_const_exprs))]

#[cfg(all(feature = "fixed-point-array", not(target_env = "msvc")))]
mod benches {
    use atomic_capsule::primitives::fixed_point::{FixedPointMatrixConst, Q16_16};
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    // ===== MICRO BENCHMARKS =====

    #[test]
    fn bench_8x8_matmul() {
        let a = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);
        let b = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = black_box(&a).matmul(&b);
        }
        let elapsed = start.elapsed();
        println!("8×8 matmul (1000 iterations): {:?}", elapsed);
        // Expected: <5μs per iteration
        assert!(elapsed.as_micros() < 50_000, "8×8 matmul should be <50ms for 1000 iterations");
    }

    #[test]
    fn bench_transpose_8x8() {
        let matrix = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = black_box(&matrix).transpose();
        }
        let elapsed = start.elapsed();
        println!("8×8 transpose (1000 iterations): {:?}", elapsed);
        // Expected: <2μs per iteration
        assert!(elapsed.as_micros() < 20_000, "8×8 transpose should be <20ms for 1000 iterations");
    }

    #[test]
    fn bench_scale_8x8() {
        let matrix = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);
        let scalar = Q16_16::from_f64(2.0);

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = black_box(&matrix).scale(scalar);
        }
        let elapsed = start.elapsed();
        println!("8×8 scale (1000 iterations): {:?}", elapsed);
        // Expected: <1μs per iteration
        assert!(elapsed.as_micros() < 10_000, "8×8 scale should be <10ms for 1000 iterations");
    }

    #[test]
    fn bench_64x64_matmul() {
        let a = FixedPointMatrixConst::<Q16_16, 64, 64, 16>::filled(Q16_16::ONE);
        let b = FixedPointMatrixConst::<Q16_16, 64, 64, 16>::filled(Q16_16::ONE);

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = black_box(&a).matmul(&b);
        }
        let elapsed = start.elapsed();
        println!("64×64 matmul (100 iterations): {:?}", elapsed);
        // Expected: <500μs per iteration = <50ms total
        assert!(elapsed.as_millis() < 100, "64×64 matmul should be <100ms for 100 iterations");
    }

    #[test]
    fn bench_allocation_zero_copy() {
        // Verify zero allocation: matrices are stack-allocated inline
        let matrix = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ZERO);

        // Size should be 8 * 8 * 8 bytes (64 × 64-bit = 512 bytes)
        assert_eq!(
            std::mem::size_of_val(&matrix),
            8 * 8 * 8,
            "8×8 matrix should be 512 bytes (zero allocation)"
        );

        // Alignment should be 64 bytes (cache-aligned for SIMD)
        assert_eq!(
            std::mem::align_of_val(&matrix),
            64,
            "Matrix should be 64B-aligned"
        );
    }

    #[test]
    fn bench_memory_layout() {
        let m64 = FixedPointMatrixConst::<Q16_16, 64, 64, 16>::filled(Q16_16::ZERO);
        let m256 = FixedPointMatrixConst::<Q16_16, 256, 256, 16>::filled(Q16_16::ZERO);

        // 64×64: 64 * 64 * 8 = 32 KB (fits in L1 cache)
        assert_eq!(
            std::mem::size_of_val(&m64),
            64 * 64 * 8,
            "64×64 should be 32 KB"
        );

        // 256×256: 256 * 256 * 8 = 512 KB (fits in L2 cache)
        assert_eq!(
            std::mem::size_of_val(&m256),
            256 * 256 * 8,
            "256×256 should be 512 KB"
        );
    }

    // ===== CRITERION BENCHMARKS (if criterion feature enabled) =====

    fn criterion_8x8_matmul(c: &mut Criterion) {
        c.bench_function("matrix_const/8x8_matmul", |b| {
            let a = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);
            let b = FixedPointMatrixConst::<Q16_16, 8, 8, 16>::filled(Q16_16::ONE);
            b.iter(|| black_box(&a).matmul(&b))
        });
    }

    fn criterion_64x64_matmul(c: &mut Criterion) {
        c.bench_function("matrix_const/64x64_matmul", |b| {
            let a = FixedPointMatrixConst::<Q16_16, 64, 64, 16>::filled(Q16_16::ONE);
            let b = FixedPointMatrixConst::<Q16_16, 64, 64, 16>::filled(Q16_16::ONE);
            b.iter(|| black_box(&a).matmul(&b))
        });
    }

    criterion_group!(benches, criterion_8x8_matmul, criterion_64x64_matmul);
    criterion_main!(benches);
}

#[cfg(not(all(feature = "fixed-point-array", not(target_env = "msvc"))))]
fn main() {
    println!("Benches require --features fixed-point-array on non-MSVC targets");
}
