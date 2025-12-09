//! T2 SIMD baseline: Replace SIMD with scalar loops
//!
//! Fair baseline strategy:
//! - Use scalar loops with same algorithm
//! - LLVM-optimized scalar code (not naive)
//! - Equivalent operations, different vectorization

/// Example: Vector addition (SIMD vs Scalar)
///
/// # SIMD version (T2)
/// ```rust,ignore
/// use std::simd::f32x8;
///
/// fn add_simd(a: &[f32], b: &[f32], result: &mut [f32]) {
///     for i in (0..a.len()).step_by(8) {
///         let va = f32x8::from_slice(&a[i..]);
///         let vb = f32x8::from_slice(&b[i..]);
///         let vr = va + vb;
///         vr.copy_to_slice(&mut result[i..]);
///     }
/// }
/// ```
///
/// # Scalar baseline
/// ```rust,ignore
/// fn add_scalar(a: &[f32], b: &[f32], result: &mut [f32]) {
///     for i in 0..a.len() {
///         result[i] = a[i] + b[i];
///     }
/// }
/// ```

/// SIMD-style array addition (simulated without portable_simd)
pub fn add_arrays_simd_style(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    // In real SIMD implementation, this would use f32x8
    // For now, simulate the same operation
    for i in 0..a.len() {
        result[i] = a[i] + b[i];
    }
}

/// Fair scalar baseline for array addition
pub fn add_arrays_scalar(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    // Same algorithm, no SIMD vectorization
    for i in 0..a.len() {
        result[i] = a[i] + b[i];
    }
}

/// Hebbian learning update (scalar baseline)
///
/// # Formula
/// Δw = η × input × output
/// w_new = w_old + Δw
///
/// where:
/// - η (eta) = learning rate
/// - input = pre-synaptic activity
/// - output = post-synaptic activity
pub fn hebbian_update_scalar(
    weights: &mut [f32],
    inputs: &[f32],
    outputs: &[f32],
    learning_rate: f32,
) {
    assert_eq!(weights.len(), inputs.len());
    assert_eq!(weights.len(), outputs.len());

    for i in 0..weights.len() {
        let delta = learning_rate * inputs[i] * outputs[i];
        weights[i] += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_arrays() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let mut result = [0.0; 4];

        add_arrays_scalar(&a, &b, &mut result);

        assert_eq!(result, [6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_hebbian_update() {
        let mut weights = [0.5, 0.5, 0.5, 0.5];
        let inputs = [1.0, 0.8, 0.6, 0.4];
        let outputs = [0.9, 0.7, 0.5, 0.3];
        let learning_rate = 0.1;

        hebbian_update_scalar(&mut weights, &inputs, &outputs, learning_rate);

        // weights[0] = 0.5 + 0.1 * 1.0 * 0.9 = 0.59
        assert!((weights[0] - 0.59).abs() < 0.001);
    }
}
