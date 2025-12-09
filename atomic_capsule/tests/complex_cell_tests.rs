//! T28 Unit Tests for ComplexCell (Q16.48 Fixed-Point Complex Numbers)
//!
//! **Source**: Extracted from planck-universe/src/physics/cnls_rule.rs (lines 448-589)
//! **Total**: 13 tests (unit tests for ComplexCell methods)
//!
//! **Framework Compliance**:
//! - T28 Q1-Q7 (Unit): Core behaviors, Q16.48 accuracy, arithmetic operations
//!
//! **Test Coverage**:
//! - Construction: new, default
//! - Accessors: real(), imag(), potential(), phase()
//! - Magnitude: magnitude(), probability()
//! - Arithmetic: add(), mul_scalar(), mul_complex()
//! - Q16.48 conversion: Precision validation

#![cfg(feature = "complex-fixed")]

use atomic_capsule::ComplexCell;

// ========================================
// T28 Q1-Q7: Unit Tests (13 tests)
// ========================================

#[test]
fn test_complex_cell_new() {
    let cell = ComplexCell::new(1.0, 2.0, 0.5, 0.0);
    assert!((cell.real() - 1.0).abs() < 1e-10);
    assert!((cell.imag() - 2.0).abs() < 1e-10);
    assert!((cell.potential() - 0.5).abs() < 1e-10);
}

#[test]
fn test_complex_cell_magnitude() {
    let cell = ComplexCell::new(3.0, 4.0, 0.0, 0.0);
    assert!((cell.magnitude() - 5.0).abs() < 1e-6);
}

#[test]
fn test_complex_cell_probability() {
    let cell = ComplexCell::new(3.0, 4.0, 0.0, 0.0);
    assert!((cell.probability() - 25.0).abs() < 1e-6);
}

#[test]
fn test_complex_cell_addition() {
    let a = ComplexCell::new(1.0, 2.0, 0.0, 0.0);
    let b = ComplexCell::new(3.0, 4.0, 0.0, 0.0);
    let c = a.add(&b);
    assert!((c.real() - 4.0).abs() < 1e-6);
    assert!((c.imag() - 6.0).abs() < 1e-6);
}

#[test]
fn test_complex_cell_scalar_multiplication() {
    let cell = ComplexCell::new(2.0, 3.0, 0.0, 0.0);
    let scaled = cell.mul_scalar(2.5);
    assert!((scaled.real() - 5.0).abs() < 1e-6);
    assert!((scaled.imag() - 7.5).abs() < 1e-6);
}

#[test]
fn test_complex_cell_complex_multiplication() {
    // (1+2i)(3+4i) = 3+4i+6i+8i² = 3+10i-8 = -5+10i
    let a = ComplexCell::new(1.0, 2.0, 0.0, 0.0);
    let b = ComplexCell::new(3.0, 4.0, 0.0, 0.0);
    let c = a.mul_complex(&b);
    assert!((c.real() - (-5.0)).abs() < 1e-6);
    assert!((c.imag() - 10.0).abs() < 1e-6);
}

#[test]
fn test_q16_48_conversion_accuracy() {
    let values = [0.0, 0.5, 1.0, -1.5, 3.14159, 100.0, -256.0];

    for &v in &values {
        let cell = ComplexCell::new(v, 0.0, 0.0, 0.0);
        let recovered = cell.real();
        assert!(
            (v - recovered).abs() < 1e-10,
            "Q16.48 conversion error: {} -> {}",
            v,
            recovered
        );
    }
}

#[test]
fn test_complex_cell_alignment() {
    assert_eq!(std::mem::align_of::<ComplexCell>(), 32);
    assert_eq!(std::mem::size_of::<ComplexCell>(), 32);
}

#[test]
fn test_complex_cell_phase_encoding() {
    use std::f64::consts::PI;

    let cell = ComplexCell::new(1.0, 0.0, 0.0, PI / 2.0);
    let phase = cell.phase();
    assert!((phase - PI / 2.0).abs() < 0.01); // u32 quantization ~0.001 radian precision
}

#[test]
fn test_complex_cell_default() {
    let cell = ComplexCell::default();
    assert_eq!(cell.real(), 0.0);
    assert_eq!(cell.imag(), 0.0);
    assert_eq!(cell.potential(), 0.0);
    assert_eq!(cell.magnitude(), 0.0);
}

#[test]
fn test_complex_cell_phase_full_range() {
    use std::f64::consts::PI;

    let phases = [0.0, PI / 4.0, PI / 2.0, PI, 3.0 * PI / 2.0, 2.0 * PI - 0.01];

    for &phase_in in &phases {
        let cell = ComplexCell::new(1.0, 0.0, 0.0, phase_in);
        let phase_out = cell.phase();
        assert!(
            (phase_out - phase_in).abs() < 0.01,
            "Phase encoding error: {} -> {}",
            phase_in,
            phase_out
        );
    }
}

#[test]
fn test_complex_cell_probability_normalization() {
    // |ψ|² = 1 for normalized wave
    let mag = 1.0;
    let angle = std::f64::consts::PI / 3.0;
    let re = mag * angle.cos();
    let im = mag * angle.sin();

    let cell = ComplexCell::new(re, im, 0.0, angle);
    assert!((cell.probability() - 1.0).abs() < 1e-6);
}

#[test]
fn test_complex_cell_scalar_mul_associativity() {
    let cell = ComplexCell::new(3.0, 4.0, 0.0, 0.0);

    let result1 = cell.mul_scalar(2.0).mul_scalar(3.0);
    let result2 = cell.mul_scalar(6.0);

    assert!((result1.real() - result2.real()).abs() < 1e-6);
    assert!((result1.imag() - result2.imag()).abs() < 1e-6);
}
