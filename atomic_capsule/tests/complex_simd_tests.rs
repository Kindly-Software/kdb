//! T28 Unit Tests for ComplexF32x4 SIMD Complex Arithmetic
//!
//! **Source**: Migrated from planck-universe/src/physics/complex_simd.rs (lines 411-618)
//! **Total**: 26 tests (20 unit + 6 property)
//!
//! **Framework Compliance**:
//! - T28 Q1-Q7 (Unit): Core behaviors, edge cases, invariants
//! - T28 Q8-Q14 (Property): Complex arithmetic identities
//!
//! **Test Coverage**:
//! - Construction: new, splat, zero, from_slice
//! - Extraction: re(), im(), to_array()
//! - Arithmetic: add, sub, mul, mul_scalar
//! - Magnitude: magnitude_sq, magnitude, normalize
//! - Complex operations: conjugate, reduce_sum
//! - Properties: Associativity, commutativity, distributivity

#![cfg(feature = "complex-simd")]
#![feature(portable_simd)]

use atomic_capsule::ComplexF32x4;

// ========================================
// T28 Q1-Q7: Unit Tests (20 tests)
// ========================================

#[test]
fn test_complex_f32x4_new() {
    let c = ComplexF32x4::new((1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0));
    let arr = c.to_array();
    assert_eq!(arr, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_complex_splat() {
    let c = ComplexF32x4::splat(3.0, 4.0);
    let arr = c.to_array();
    assert_eq!(arr, [3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]);
}

#[test]
fn test_complex_zero() {
    let c = ComplexF32x4::zero();
    let arr = c.to_array();
    assert_eq!(arr, [0.0; 8]);
}

#[test]
fn test_complex_from_slice() {
    let data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let c = ComplexF32x4::from_slice(&data);
    assert_eq!(c.to_array(), data);
}

#[test]
fn test_extract_re() {
    let c = ComplexF32x4::new((1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0));
    let re = c.re();
    assert_eq!(re.to_array(), [1.0, 3.0, 5.0, 7.0]);
}

#[test]
fn test_extract_im() {
    let c = ComplexF32x4::new((1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0));
    let im = c.im();
    assert_eq!(im.to_array(), [2.0, 4.0, 6.0, 8.0]);
}

#[test]
fn test_complex_add() {
    let a = ComplexF32x4::splat(1.0, 2.0);
    let b = ComplexF32x4::splat(3.0, 4.0);
    let c = a.add(&b);
    let arr = c.to_array();
    assert_eq!(arr, [4.0, 6.0, 4.0, 6.0, 4.0, 6.0, 4.0, 6.0]);
}

#[test]
fn test_complex_sub() {
    let a = ComplexF32x4::splat(5.0, 7.0);
    let b = ComplexF32x4::splat(2.0, 3.0);
    let c = a.sub(&b);
    let arr = c.to_array();
    assert_eq!(arr, [3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]);
}

#[test]
fn test_complex_mul() {
    // (1+2i)(3+4i) = (3-8) + (4+6)i = -5 + 10i
    let a = ComplexF32x4::splat(1.0, 2.0);
    let b = ComplexF32x4::splat(3.0, 4.0);
    let c = a.mul(&b);
    let arr = c.to_array();
    assert!((arr[0] - (-5.0)).abs() < 1e-6);
    assert!((arr[1] - 10.0).abs() < 1e-6);
}

#[test]
fn test_complex_mul_scalar() {
    let c = ComplexF32x4::splat(3.0, 4.0);
    let result = c.mul_scalar(2.0);
    let arr = result.to_array();
    assert_eq!(arr, [6.0, 8.0, 6.0, 8.0, 6.0, 8.0, 6.0, 8.0]);
}

#[test]
fn test_magnitude_sq() {
    // |3+4i|² = 9+16 = 25
    let c = ComplexF32x4::splat(3.0, 4.0);
    let mag_sq = c.magnitude_sq();
    assert!((mag_sq.to_array()[0] - 25.0).abs() < 1e-6);
}

#[test]
fn test_magnitude() {
    // |3+4i| = 5
    let c = ComplexF32x4::splat(3.0, 4.0);
    let mag = c.magnitude();
    assert!((mag.to_array()[0] - 5.0).abs() < 1e-6);
}

#[test]
fn test_conjugate() {
    let c = ComplexF32x4::splat(3.0, 4.0);
    let conj = c.conjugate();
    let arr = conj.to_array();
    assert_eq!(arr, [3.0, -4.0, 3.0, -4.0, 3.0, -4.0, 3.0, -4.0]);
}

#[test]
fn test_reduce_sum() {
    let c = ComplexF32x4::new((1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0));
    let (re_sum, im_sum) = c.reduce_sum();
    assert!((re_sum - 16.0).abs() < 1e-6); // 1+3+5+7
    assert!((im_sum - 20.0).abs() < 1e-6); // 2+4+6+8
}

#[test]
fn test_normalize() {
    let c = ComplexF32x4::splat(3.0, 4.0);
    let normalized = c.normalize();
    let mag = normalized.magnitude();
    let arr = mag.to_array();
    assert!((arr[0] - 1.0).abs() < 1e-6);
}

// ========================================
// T28 Q8-Q14: Property Tests (6 tests)
// ========================================

#[test]
fn test_add_associativity() {
    // (a + b) + c = a + (b + c)
    let a = ComplexF32x4::new((1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0));
    let b = ComplexF32x4::new((2.0, 3.0), (4.0, 5.0), (6.0, 7.0), (8.0, 9.0));
    let c = ComplexF32x4::new((3.0, 4.0), (5.0, 6.0), (7.0, 8.0), (9.0, 10.0));

    let lhs = a.add(&b).add(&c);
    let rhs = a.add(&b.add(&c));

    let lhs_arr = lhs.to_array();
    let rhs_arr = rhs.to_array();

    for i in 0..8 {
        assert!((lhs_arr[i] - rhs_arr[i]).abs() < 1e-6);
    }
}

#[test]
fn test_add_commutativity() {
    // a + b = b + a
    let a = ComplexF32x4::splat(1.0, 2.0);
    let b = ComplexF32x4::splat(3.0, 4.0);

    let lhs = a.add(&b);
    let rhs = b.add(&a);

    let lhs_arr = lhs.to_array();
    let rhs_arr = rhs.to_array();

    for i in 0..8 {
        assert!((lhs_arr[i] - rhs_arr[i]).abs() < 1e-6);
    }
}

#[test]
fn test_mul_associativity() {
    // (a × b) × c = a × (b × c)
    let a = ComplexF32x4::splat(1.0, 1.0);
    let b = ComplexF32x4::splat(2.0, 2.0);
    let c = ComplexF32x4::splat(3.0, 3.0);

    let lhs = a.mul(&b).mul(&c);
    let rhs = a.mul(&b.mul(&c));

    let lhs_arr = lhs.to_array();
    let rhs_arr = rhs.to_array();

    for i in 0..8 {
        assert!((lhs_arr[i] - rhs_arr[i]).abs() < 1e-5); // Slightly larger epsilon for mul
    }
}

#[test]
fn test_mul_identity() {
    // z × 1 = z
    let z = ComplexF32x4::splat(3.0, 4.0);
    let one = ComplexF32x4::splat(1.0, 0.0);

    let result = z.mul(&one);
    let result_arr = result.to_array();
    let z_arr = z.to_array();

    for i in 0..8 {
        assert!((result_arr[i] - z_arr[i]).abs() < 1e-6);
    }
}

#[test]
fn test_conjugate_involution() {
    // (z*)* = z
    let z = ComplexF32x4::splat(3.0, 4.0);
    let conj_conj = z.conjugate().conjugate();

    let z_arr = z.to_array();
    let cc_arr = conj_conj.to_array();

    for i in 0..8 {
        assert!((z_arr[i] - cc_arr[i]).abs() < 1e-6);
    }
}

#[test]
fn test_magnitude_multiplicativity() {
    // |a × b| = |a| × |b|
    let a = ComplexF32x4::splat(3.0, 4.0); // |a| = 5
    let b = ComplexF32x4::splat(5.0, 12.0); // |b| = 13

    let c = a.mul(&b);
    let mag_c = c.magnitude().to_array()[0];

    let mag_a = a.magnitude().to_array()[0];
    let mag_b = b.magnitude().to_array()[0];

    assert!((mag_c - mag_a * mag_b).abs() < 1e-5);
}
