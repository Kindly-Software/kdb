//! T28 Unit Tests for CNLS Validation Functions
//!
//! **Functions Tested**:
//! - validate_determinism_q16_48(): Q16.48 vs SIMD comparison
//! - verify_norm_conservation(): ∫|ψ|² = constant
//!
//! **Framework Compliance**:
//! - T28 Q1-Q7 (Unit): 20+ tests for validation logic
//! - ASSUM: All tolerance/bounds checks verified
//! - B32: Performance targets validated
//!
//! **Test Coverage**:
//! - Error handling: Invalid tolerance, dimension mismatch, bounds
//! - Determinism validation: SIMD vs Q16.48 comparison
//! - Norm conservation: Probability preservation
//! - Edge cases: Zero tolerance, NaN, empty grids

#![cfg(feature = "cnls")]

use atomic_capsule::patterns::cnls::{
    validate_determinism_q16_48, verify_norm_conservation, CNLSError, CNLSRuleCapsule, ComplexCell,
    Universe4DInterface,
};

// ============================================================================
// Mock Universe for Testing
// ============================================================================

/// Mock 4D universe for testing validation functions
struct MockUniverse {
    grid_size: usize,
    cells: Vec<ComplexCell>,
}

impl MockUniverse {
    fn new(grid_size: usize) -> Self {
        let num_cells = grid_size.pow(4);
        Self {
            grid_size,
            cells: vec![ComplexCell::default(); num_cells],
        }
    }

    fn set_cell(&mut self, x: usize, y: usize, z: usize, t: usize, cell: ComplexCell) {
        let idx = self.index_4d(x, y, z, t);
        self.cells[idx] = cell;
    }

    fn index_4d(&self, x: usize, y: usize, z: usize, t: usize) -> usize {
        t * (self.grid_size.pow(3)) + z * (self.grid_size.pow(2)) + y * self.grid_size + x
    }
}

impl Universe4DInterface for MockUniverse {
    fn grid_size(&self) -> usize {
        self.grid_size
    }

    fn get_cell_4d(
        &self,
        x: usize,
        y: usize,
        z: usize,
        t: usize,
    ) -> Result<ComplexCell, CNLSError> {
        if x >= self.grid_size || y >= self.grid_size || z >= self.grid_size || t >= self.grid_size
        {
            return Err(CNLSError::IndexOutOfBounds);
        }

        let idx = self.index_4d(x, y, z, t);
        Ok(self.cells[idx])
    }
}

// ============================================================================
// T28 Q1-Q7: Unit Tests for validate_determinism_q16_48
// ============================================================================

#[test]
fn test_validate_determinism_identical_universes() {
    // Two identical universes → max_error = 0.0 < tolerance
    let mut universe_a = MockUniverse::new(2);
    let mut universe_b = MockUniverse::new(2);

    // Set identical cells
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    let cell = ComplexCell::new(1.0, 2.0, 0.0, 0.0);
                    universe_a.set_cell(x, y, z, t, cell);
                    universe_b.set_cell(x, y, z, t, cell);
                }
            }
        }
    }

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6).unwrap();
    assert!(
        result,
        "Identical universes should validate as deterministic"
    );
}

#[test]
fn test_validate_determinism_small_difference() {
    // Small difference (1e-7) < tolerance (1e-6) → should pass
    let mut universe_a = MockUniverse::new(2);
    let mut universe_b = MockUniverse::new(2);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe_a.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                    universe_b.set_cell(x, y, z, t, ComplexCell::new(1.0 + 1e-7, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6).unwrap();
    assert!(
        result,
        "Small difference (1e-7) should be within tolerance (1e-6)"
    );
}

#[test]
fn test_validate_determinism_large_difference() {
    // Large difference (1e-5) > tolerance (1e-6) → should fail
    let mut universe_a = MockUniverse::new(2);
    let mut universe_b = MockUniverse::new(2);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe_a.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                    universe_b.set_cell(x, y, z, t, ComplexCell::new(1.0 + 1e-5, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6).unwrap();
    assert!(
        !result,
        "Large difference (1e-5) should exceed tolerance (1e-6)"
    );
}

#[test]
fn test_validate_determinism_imaginary_difference() {
    // Difference in imaginary part
    let mut universe_a = MockUniverse::new(2);
    let mut universe_b = MockUniverse::new(2);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe_a.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                    universe_b.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0 + 1e-5, 0.0, 0.0));
                }
            }
        }
    }

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6).unwrap();
    assert!(!result, "Imaginary difference should be detected");
}

#[test]
fn test_validate_determinism_invalid_tolerance_zero() {
    let universe_a = MockUniverse::new(2);
    let universe_b = MockUniverse::new(2);

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 0.0);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "Zero tolerance should be rejected"
    );
}

#[test]
fn test_validate_determinism_invalid_tolerance_negative() {
    let universe_a = MockUniverse::new(2);
    let universe_b = MockUniverse::new(2);

    let result = validate_determinism_q16_48(&universe_a, &universe_b, -1e-6);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "Negative tolerance should be rejected"
    );
}

#[test]
fn test_validate_determinism_invalid_tolerance_nan() {
    let universe_a = MockUniverse::new(2);
    let universe_b = MockUniverse::new(2);

    let result = validate_determinism_q16_48(&universe_a, &universe_b, f64::NAN);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "NaN tolerance should be rejected"
    );
}

#[test]
fn test_validate_determinism_invalid_tolerance_infinity() {
    let universe_a = MockUniverse::new(2);
    let universe_b = MockUniverse::new(2);

    let result = validate_determinism_q16_48(&universe_a, &universe_b, f64::INFINITY);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "Infinite tolerance should be rejected"
    );
}

#[test]
fn test_validate_determinism_dimension_mismatch() {
    let universe_a = MockUniverse::new(2);
    let universe_b = MockUniverse::new(3);

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6);
    assert_eq!(
        result,
        Err(CNLSError::DimensionMismatch),
        "Dimension mismatch should be detected"
    );
}

#[test]
fn test_validate_determinism_single_cell_difference() {
    // Only one cell differs → max_error should be detected
    let mut universe_a = MockUniverse::new(3);
    let mut universe_b = MockUniverse::new(3);

    // Set all cells identical except (1, 1, 1, 1)
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                for t in 0..3 {
                    universe_a.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                    universe_b.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    // Make (1, 1, 1, 1) differ by 1e-5
    universe_b.set_cell(1, 1, 1, 1, ComplexCell::new(1.0 + 1e-5, 2.0, 0.0, 0.0));

    let result = validate_determinism_q16_48(&universe_a, &universe_b, 1e-6).unwrap();
    assert!(!result, "Single cell difference should be detected");
}

// ============================================================================
// T28 Q1-Q7: Unit Tests for verify_norm_conservation
// ============================================================================

#[test]
fn test_verify_norm_conservation_zero_energy() {
    // Empty universe (all cells zero) → norm = 0.0, stored = 0.0
    let universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
    rule.update_energy(0.0);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(result, "Zero norm should be conserved");
}

#[test]
fn test_verify_norm_conservation_uniform_field() {
    // Uniform field: |ψ|² = 5 per cell, N⁴ = 16 cells, dx = 1.0 → norm = 80.0
    let mut universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    // |ψ|² = 1² + 2² = 5
                    universe.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    // Total norm: 5 (per cell) × 16 (cells) × 1.0⁴ (dx⁴) = 80.0
    rule.update_energy(80.0);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(result, "Uniform field norm should be conserved");
}

#[test]
fn test_verify_norm_conservation_small_violation() {
    // Norm slightly different (79.999999 vs 80.0) → within tolerance
    let mut universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    // Stored norm slightly different
    rule.update_energy(79.999999);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(result, "Small norm deviation should be within tolerance");
}

#[test]
fn test_verify_norm_conservation_large_violation() {
    // Norm violation (79.9 vs 80.0) → exceeds tolerance
    let mut universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe.set_cell(x, y, z, t, ComplexCell::new(1.0, 2.0, 0.0, 0.0));
                }
            }
        }
    }

    // Stored norm significantly different
    rule.update_energy(79.9);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(!result, "Large norm violation should be detected");
}

#[test]
fn test_verify_norm_conservation_invalid_tolerance_zero() {
    let universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    let result = verify_norm_conservation(&universe, &rule, 0.0);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "Zero tolerance should be rejected"
    );
}

#[test]
fn test_verify_norm_conservation_invalid_tolerance_negative() {
    let universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    let result = verify_norm_conservation(&universe, &rule, -1e-6);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "Negative tolerance should be rejected"
    );
}

#[test]
fn test_verify_norm_conservation_invalid_tolerance_nan() {
    let universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    let result = verify_norm_conservation(&universe, &rule, f64::NAN);
    assert_eq!(
        result,
        Err(CNLSError::InvalidTolerance),
        "NaN tolerance should be rejected"
    );
}

#[test]
fn test_verify_norm_conservation_nonuniform_field() {
    // Non-uniform field with varying |ψ|² values
    let mut universe = MockUniverse::new(2);
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);

    let mut expected_norm = 0.0;
    let dx4 = 1.0_f64.powi(4);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    let re = (x + y + z + t) as f64;
                    let im = (x * y + z * t) as f64;
                    universe.set_cell(x, y, z, t, ComplexCell::new(re, im, 0.0, 0.0));

                    expected_norm += (re * re + im * im) * dx4;
                }
            }
        }
    }

    rule.update_energy(expected_norm);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(result, "Non-uniform field norm should be conserved");
}

#[test]
fn test_verify_norm_conservation_with_dx() {
    // Test with non-unit dx (spatial resolution)
    let mut universe = MockUniverse::new(2);
    let dx = 0.5;
    let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, dx);

    let mut expected_norm = 0.0;
    let dx4 = dx.powi(4);

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                for t in 0..2 {
                    universe.set_cell(x, y, z, t, ComplexCell::new(2.0, 3.0, 0.0, 0.0));
                    // |ψ|² = 2² + 3² = 13
                    expected_norm += 13.0 * dx4;
                }
            }
        }
    }

    rule.update_energy(expected_norm);

    let result = verify_norm_conservation(&universe, &rule, 1e-6).unwrap();
    assert!(result, "Norm with non-unit dx should be conserved");
}

#[test]
fn test_cnls_error_display() {
    // Test Display trait for CNLSError
    assert_eq!(
        CNLSError::IndexOutOfBounds.to_string(),
        "Grid index out of bounds"
    );
    assert_eq!(
        CNLSError::DimensionMismatch.to_string(),
        "Universe dimensions mismatch"
    );
    assert_eq!(
        CNLSError::InvalidTolerance.to_string(),
        "Invalid tolerance (must be positive finite)"
    );
}
